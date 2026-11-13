//! Runs a [`Session`] over TCP or TLS on a background thread (R-NET-1, R-NET-2).
//!
//! The thread owns the socket. It feeds what arrives into the session and sends what the
//! session queued; the UI locks the session to draw it and to pass it keys.

pub mod hostserver;
mod tls;

use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use crate::error::{Error, Result};
use crate::session::{Session, SessionConfig};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// How long a read waits before the thread checks for keys to send: the worst-case delay
/// between a key and its bytes leaving.
const POLL: Duration = Duration::from_millis(15);
/// With keep-alive on, a No Operation goes out after this long without sending (R-NET-5).
const KEEP_ALIVE: Duration = Duration::from_secs(60);

/// Set `PARLEY_TRACE` to a file name to append the bytes the host sends, as hex, for building
/// test fixtures. Only inbound data is traced: outbound data holds what the operator typed,
/// passwords included.
const TRACE_VARIABLE: &str = "PARLEY_TRACE";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkState {
    Connecting,
    Connected,
    /// The connection ended; the text says why.
    Closed(String),
}

#[derive(Debug)]
pub struct Connection {
    session: Arc<Mutex<Session>>,
    state: Arc<Mutex<LinkState>>,
    stop: Arc<AtomicBool>,
}

impl Connection {
    /// Starts connecting. `wake` is called from the network thread whenever the screen or the
    /// state changed, so the UI can redraw.
    pub fn open(config: SessionConfig, wake: impl Fn() + Send + 'static) -> Self {
        let session = Arc::new(Mutex::new(Session::new(config.clone())));
        let state = Arc::new(Mutex::new(LinkState::Connecting));
        let stop = Arc::new(AtomicBool::new(false));
        let link = Link {
            session: Arc::clone(&session),
            stop: Arc::clone(&stop),
            wake: Box::new(wake),
        };
        let thread_state = Arc::clone(&state);
        thread::spawn(move || {
            let reason = match link.run(&config, &thread_state) {
                Ok(reason) => reason,
                Err(e) => e.to_string(),
            };
            *lock(&thread_state) = LinkState::Closed(reason);
            (link.wake)();
        });
        Self {
            session,
            state,
            stop,
        }
    }

    /// The session, locked. Keep the guard short: the network thread waits for it.
    pub fn session(&self) -> MutexGuard<'_, Session> {
        lock(&self.session)
    }

    pub fn state(&self) -> LinkState {
        lock(&self.state).clone()
    }

    /// Ends the connection; the network thread notices within a poll interval.
    pub fn close(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        self.close();
    }
}

/// A poisoned lock only means another thread panicked while holding it; the data is still
/// the best there is.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

trait Stream: Read + Write + Send {}
impl<T: Read + Write + Send> Stream for T {}

struct Link {
    session: Arc<Mutex<Session>>,
    stop: Arc<AtomicBool>,
    wake: Box<dyn Fn() + Send>,
}

impl Link {
    /// Returns why the connection ended normally; errors are the abnormal ends.
    fn run(&self, config: &SessionConfig, state: &Mutex<LinkState>) -> Result<String> {
        let tcp = connect_tcp(&config.host, config.port)?;
        tcp.set_nodelay(true)?;
        let mut stream: Box<dyn Stream> = if config.tls {
            Box::new(tls::connect(config, tcp, POLL)?)
        } else {
            tcp.set_read_timeout(Some(POLL))?;
            Box::new(tcp)
        };
        *lock(state) = LinkState::Connected;
        (self.wake)();

        let mut trace = std::env::var_os(TRACE_VARIABLE)
            .and_then(|path| OpenOptions::new().create(true).append(true).open(path).ok());
        let mut buf = vec![0; 16 * 1024];
        let mut last_sent = Instant::now();
        loop {
            if self.stop.load(Ordering::Relaxed) {
                return Ok("Disconnected.".to_owned());
            }
            let outbound = {
                let mut session = lock(&self.session);
                if config.keep_alive && last_sent.elapsed() >= KEEP_ALIVE {
                    session.keep_alive();
                }
                if session.check_macro_time(Instant::now()) {
                    (self.wake)();
                }
                session.take_outbound()
            };
            if !outbound.is_empty() {
                stream.write_all(&outbound)?;
                stream.flush()?;
                last_sent = Instant::now();
            }
            match stream.read(&mut buf) {
                Ok(0) => return Ok("The host closed the connection.".to_owned()),
                Ok(n) => {
                    if let Some(file) = trace.as_mut() {
                        write_trace(file, &buf[..n]);
                    }
                    lock(&self.session).receive(&buf[..n]);
                    (self.wake)();
                }
                Err(e) if is_timeout(&e) => {}
                Err(e) => return Err(e.into()),
            }
        }
    }
}

fn connect_tcp(host: &str, port: u16) -> Result<TcpStream> {
    let fail = |source| Error::Connect {
        host: host.to_owned(),
        port,
        source,
    };
    let mut last = io::Error::new(io::ErrorKind::NotFound, "the host name has no addresses");
    for address in (host, port).to_socket_addrs().map_err(fail)? {
        match TcpStream::connect_timeout(&address, CONNECT_TIMEOUT) {
            Ok(stream) => return Ok(stream),
            Err(e) => last = e,
        }
    }
    Err(fail(last))
}

/// A read that timed out (Unix reports `WouldBlock`, Windows `TimedOut`).
fn is_timeout(e: &io::Error) -> bool {
    matches!(
        e.kind(),
        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut | io::ErrorKind::Interrupted
    )
}

fn write_trace(file: &mut File, bytes: &[u8]) {
    let hex: Vec<String> = bytes.iter().map(|b| format!("{b:02x}")).collect();
    // Tracing is a debugging aid: a failed write must not end the session.
    let _ = writeln!(file, "{}", hex.join(" "));
}
