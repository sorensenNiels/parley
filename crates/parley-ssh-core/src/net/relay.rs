//! Copying through Parley SSH (R-SSH-54, the engine part of R-SSH-31): a server that cannot reach another is
//! asked to listen on a loopback port (`tcpip-forward`, RFC 4254 §7.1), and each connection to it is carried
//! through the other server's open connection to that server's own SSH port (`direct-tcpip`, §7.2), so a jump host
//! of its works too.

use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex, PoisonError};

use russh::client as ssh;

use super::jobs::JobId;
use super::{Event, Link, client};

/// Where a forwarded port listens on the server that runs rsync.
const LOOPBACK: &str = "127.0.0.1";

/// A way into a server through its open connection: what [`super::Command::OpenDoor`] hands out, for another
/// connection's [`super::Command::Forward`].
#[derive(Clone)]
pub struct Door {
    id: JobId,
    handle: Arc<ssh::Handle<client::Client>>,
    /// The server's SSH port, as reached from the server itself.
    port: u16,
}

impl Door {
    pub(super) fn new(id: JobId, handle: Arc<ssh::Handle<client::Client>>, port: u16) -> Self {
        Self { id, handle, port }
    }
}

impl fmt::Debug for Door {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Door")
            .field("id", &self.id)
            .field("port", &self.port)
            .finish_non_exhaustive()
    }
}

/// Doors are told apart by the job they were opened for.
impl PartialEq for Door {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && self.port == other.port
    }
}

impl Eq for Door {}

/// The ports this connection's server forwards, each to its door; shared with the handler, which carries the
/// connections that come in.
#[derive(Clone, Default)]
pub(super) struct Doors(Arc<Mutex<HashMap<u32, (JobId, Door)>>>);

impl Doors {
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<u32, (JobId, Door)>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The door a connection to `port` goes through.
    pub(super) fn get(&self, port: u32) -> Option<Door> {
        self.lock().get(&port).map(|(_, door)| door.clone())
    }

    /// Asks the server to listen on a free loopback port and carry its connections through `door`; says the port.
    pub(super) async fn forward(
        &self,
        handle: &ssh::Handle<client::Client>,
        id: JobId,
        door: Door,
        link: &Link,
    ) {
        let result = match handle.tcpip_forward(LOOPBACK, 0).await {
            Ok(port) => match u16::try_from(port) {
                Ok(shown) if shown != 0 => {
                    self.lock().insert(port, (id, door));
                    link.log(format!("The server forwards {LOOPBACK}:{port} through Parley"));
                    Ok(shown)
                }
                _ => Err(format!("The server chose a port that cannot be: {port}")),
            },
            Err(russh::Error::RequestDenied) => Err(
                "The source server does not allow forwarding (its sshd_config has AllowTcpForwarding no)"
                    .to_owned(),
            ),
            Err(e) => Err(format!("The source server could not forward a port: {e}")),
        };
        link.send(Event::Forwarded { id, port: result });
    }

    /// Stops the forwarding opened for `id`; one already stopped is not there, which is harmless.
    pub(super) async fn unforward(&self, handle: &ssh::Handle<client::Client>, id: JobId) {
        let port = {
            let mut doors = self.lock();
            let port = doors.iter().find(|(_, (of, _))| *of == id).map(|(p, _)| *p);
            if let Some(port) = port {
                doors.remove(&port);
            }
            port
        };
        if let Some(port) = port {
            // The connection may be ending; then the forwarding ends with it.
            let _ = handle.cancel_tcpip_forward(LOOPBACK, port).await;
        }
    }
}

/// Carries one connection that came in on a forwarded port through `door`, until either side closes.
pub(super) fn carry(channel: russh::Channel<ssh::Msg>, door: Door, link: Link) {
    tokio::spawn(async move {
        match door
            .handle
            .channel_open_direct_tcpip(LOOPBACK, u32::from(door.port), LOOPBACK, 0)
            .await
        {
            Ok(through) => {
                let (mut from, mut to) = (channel.into_stream(), through.into_stream());
                // Either side closing ends the relay; how is of no interest.
                let _ = tokio::io::copy_bidirectional(&mut from, &mut to).await;
            }
            Err(e) => {
                link.log(format!(
                    "The target server would not carry the connection to its own SSH port \
                     (AllowTcpForwarding?): {e}"
                ));
                // Closing tells the source's ssh at once.
                let _ = channel.close().await;
            }
        }
    });
}
