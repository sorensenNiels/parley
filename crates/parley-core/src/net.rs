//! Runs a [`Session`] over TCP or TLS on a background thread (R-NET-1, R-NET-2).
//!
//! The thread owns the socket. It feeds what arrives into the session and sends what the
//! session queued; the UI locks the session to draw it and to pass it keys.

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
    /// The connection ended, or never got through; the text says why.
    Closed(String),
    /// The connection got through, then the network failed: a read, write or TLS error, not
    /// the host ending the session (R-NET-10). The text says why.
    Dropped(String),
}

/// How the link ended: `run`'s result, and whether it had got through first.
fn ending(result: Result<String>, got_through: bool) -> LinkState {
    match result {
        Ok(reason) => LinkState::Closed(reason),
        Err(e) if got_through => LinkState::Dropped(e.to_string()),
        Err(e) => LinkState::Closed(e.to_string()),
    }
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
            let result = link.run(&config, &thread_state);
            let mut state = lock(&thread_state);
            *state = ending(result, *state == LinkState::Connected);
            drop(state);
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

/// A connection to a host: TCP, or TLS over TCP.
pub trait Stream: Read + Write + Send {}
impl<T: Read + Write + Send> Stream for T {}

/// Connects to `port` on `config`'s host, over TLS when `config` says so (R-NET-2), with
/// `read_timeout` on reads. The terminal's link and the IBM i host servers (R-SQL-2) both
/// connect this way.
///
/// # Errors
/// When the host cannot be reached or the TLS handshake fails.
pub fn open_stream(
    config: &SessionConfig,
    port: u16,
    read_timeout: Duration,
) -> Result<Box<dyn Stream>> {
    let tcp = connect_tcp(&config.host, port)?;
    tcp.set_nodelay(true)?;
    Ok(if config.tls {
        Box::new(tls::connect(config, tcp, read_timeout)?)
    } else {
        tcp.set_read_timeout(Some(read_timeout))?;
        Box::new(tcp)
    })
}

struct Link {
    session: Arc<Mutex<Session>>,
    stop: Arc<AtomicBool>,
    wake: Box<dyn Fn() + Send>,
}

impl Link {
    /// The trace file named by [`TRACE_VARIABLE`], if it is set. One that cannot be opened is
    /// not a reason to refuse the connection: the session's notice says why there is no trace.
    fn open_trace(&self) -> Option<File> {
        let path = std::env::var_os(TRACE_VARIABLE)?;
        match OpenOptions::new().create(true).append(true).open(&path) {
            Ok(file) => Some(file),
            Err(e) => {
                lock(&self.session).set_notice(format!(
                    "cannot open the trace file {}: {e}",
                    path.to_string_lossy()
                ));
                None
            }
        }
    }

    /// Returns why the connection ended normally; errors are the abnormal ends.
    fn run(&self, config: &SessionConfig, state: &Mutex<LinkState>) -> Result<String> {
        let mut stream = open_stream(config, config.port, POLL)?;
        *lock(state) = LinkState::Connected;
        (self.wake)();

        let mut trace = self.open_trace();
        let mut buf = vec![0; 16 * 1024];
        let mut last_sent = Instant::now();
        loop {
            if self.stop.load(Ordering::Relaxed) {
                return Ok("Disconnected.".to_owned());
            }
            let (outbound, gave_up) = {
                let mut session = lock(&self.session);
                if config.keep_alive && last_sent.elapsed() >= KEEP_ALIVE {
                    session.keep_alive();
                }
                let gave_up = session.check_macro_time(Instant::now());
                (session.take_outbound(), gave_up)
            };
            // The UI locks the session to redraw: wake it only after the lock is let go.
            if gave_up {
                (self.wake)();
            }
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

/// A TCP connection to `host`:`port`, trying each of its addresses.
///
/// # Errors
/// When the name cannot be resolved or no address answers.
pub fn connect_tcp(host: &str, port: u16) -> Result<TcpStream> {
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
