//! Desktop transfers' remote shell (R-SSH-55): rsync on the desktop runs `parley-ssh --relay PORT TOKEN host
//! command…` as its `-e` program; the relay connects to the running window on a loopback port, says its one-time
//! token and the command, and then carries rsync's stdin and stdout. The window runs the command on an exec channel
//! of the session's open connection ([`super::Command::Pipe`]), so nothing signs on again.
//!
//! The relay's first two lines: the token, then the command as ssh would send it (its words joined by spaces).

use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, Shutdown, TcpListener, TcpStream};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

/// The most a relay may say before its data: a long rsync command line fits many times over.
const HEADER: u64 = 64 * 1024;

/// How long a relay has to say its token and command.
const SAYING: Duration = Duration::from_secs(10);

/// A one-time token: 128 random bits as hex.
///
/// # Errors
/// The system has no randomness to give, said for the user.
pub fn token() -> Result<String, String> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes)
        .map_err(|e| format!("This computer could not make a random token: {e}"))?;
    Ok(data_encoding::HEXLOWER.encode(&bytes))
}

/// Whether `given` is `token`, taking as long whatever the first difference.
pub fn same_token(given: &str, token: &str) -> bool {
    given.len() == token.len()
        && given
            .bytes()
            .zip(token.bytes())
            .fold(0u8, |d, (a, b)| d | (a ^ b))
            == 0
}

/// A relay that has said its token and command; its stream carries rsync's data from here on.
#[derive(Debug)]
pub struct Piped {
    pub token: String,
    pub command: String,
    pub stream: TcpStream,
}

/// The window's end: a loopback port relays connect to.
#[derive(Debug)]
pub struct PipeServer {
    port: u16,
    piped: Receiver<Piped>,
}

impl PipeServer {
    /// Listens on a free loopback port, on a thread of its own; `wake` is called when a relay has spoken.
    ///
    /// # Errors
    /// No loopback port could be had.
    pub fn start(wake: impl Fn() + Send + Sync + 'static) -> io::Result<Self> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        let port = listener.local_addr()?.port();
        let (send, piped) = mpsc::channel();
        let wake = std::sync::Arc::new(wake);
        std::thread::Builder::new()
            .name("parley-ssh-pipes".into())
            .spawn(move || {
                for stream in listener.incoming() {
                    let Ok(stream) = stream else { continue };
                    let send = send.clone();
                    let wake = std::sync::Arc::clone(&wake);
                    // A relay that does not speak must not hold up the next.
                    std::thread::spawn(move || {
                        if let Ok(piped) = hear(stream)
                            && send.send(piped).is_ok()
                        {
                            wake();
                        }
                    });
                }
            })?;
        Ok(Self { port, piped })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    /// The relays that have spoken since the last call.
    pub fn take(&self) -> Vec<Piped> {
        self.piped.try_iter().collect()
    }
}

/// Reads a relay's token and command, a byte at a time: rsync's data follows at once (it says its protocol
/// version without waiting), and must stay in the stream.
fn hear(mut stream: TcpStream) -> io::Result<Piped> {
    stream.set_read_timeout(Some(SAYING))?;
    let mut said = 0u64;
    let mut line = |what: &str| -> io::Result<String> {
        let mut bytes = Vec::new();
        let mut byte = [0u8];
        loop {
            said += 1;
            if said > HEADER || stream.read(&mut byte)? == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("no {what}"),
                ));
            }
            if byte[0] == b'\n' {
                return String::from_utf8(bytes)
                    .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, format!("no {what}")));
            }
            bytes.push(byte[0]);
        }
    };
    let token = line("token")?;
    let command = line("command")?;
    stream.set_read_timeout(None)?;
    Ok(Piped {
        token,
        command,
        stream,
    })
}

/// The relay's side: connects to the window's `port`, says `token` and `command`, then carries `input` to the
/// window and what comes back to `output` until the window closes the connection.
///
/// # Errors
/// The window is not there, or the connection broke.
pub fn relay(
    port: u16,
    token: &str,
    command: &str,
    mut input: impl Read + Send + 'static,
    mut output: impl Write,
) -> io::Result<()> {
    let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, port))?;
    stream.set_nodelay(true)?;
    if token.contains('\n') || command.contains('\n') {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "a line end in the token or command",
        ));
    }
    stream.write_all(format!("{token}\n{command}\n").as_bytes())?;
    let mut sending = stream.try_clone()?;
    std::thread::spawn(move || {
        // rsync closing its end, or the window closing, ends the copy; either is how it ends, not a failure.
        let _ = io::copy(&mut input, &mut sending);
        let _ = sending.shutdown(Shutdown::Write);
    });
    // Each piece goes on at once: stdout is line-buffered, and rsync's data has no lines.
    let mut buffer = vec![0u8; 32 * 1024];
    loop {
        match stream.read(&mut buffer) {
            Ok(0) => return output.flush(),
            Ok(n) => {
                output.write_all(&buffer[..n])?;
                output.flush()?;
            }
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
}

/// The command rsync asks its remote shell to run, from the relay's arguments after the host: joined by spaces,
/// as ssh joins them (rsync quotes what needs it).
pub fn command_of(words: &[String]) -> String {
    words.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_random_hex_and_compared_whole_r_ssh_55() {
        let (a, b) = (token().expect("a token"), token().expect("a token"));
        assert_eq!(a.len(), 32);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b);
        assert!(same_token(&a, &a));
        assert!(!same_token(&a, &b));
        assert!(!same_token(&a[..31], &a));
        assert!(!same_token("", &a));
    }

    #[test]
    fn a_relay_says_its_token_and_command_then_carries_the_data_both_ways_r_ssh_55() {
        let server = PipeServer::start(|| {}).expect("a port");
        let port = server.port();
        let relay = std::thread::spawn(move || {
            let mut output = Vec::new();
            relay(
                port,
                "abc",
                "rsync --server -logDtprze.iLsfxCIvu . /tmp/æøå",
                io::Cursor::new(b"from rsync".to_vec()),
                &mut output,
            )
            .map(|()| output)
        });
        let piped = loop {
            if let Some(piped) = server.take().pop() {
                break piped;
            }
            std::thread::sleep(Duration::from_millis(5));
        };
        assert_eq!(piped.token, "abc");
        assert_eq!(
            piped.command,
            "rsync --server -logDtprze.iLsfxCIvu . /tmp/æøå"
        );
        let mut stream = piped.stream;
        let mut heard = Vec::new();
        stream.read_to_end(&mut heard).expect("read");
        assert_eq!(heard, b"from rsync", "rsync's input, ended when it ended");
        stream.write_all(b"from the server").expect("written");
        drop(stream);
        let output = relay.join().expect("relay thread").expect("relayed");
        assert_eq!(output, b"from the server");
    }

    #[test]
    fn data_right_after_the_header_stays_in_the_stream_r_ssh_55() {
        let server = PipeServer::start(|| {}).expect("a port");
        let mut relay =
            TcpStream::connect((Ipv4Addr::LOCALHOST, server.port())).expect("connected");
        // rsync says its protocol version at once, in the same packet as the header.
        relay
            .write_all(b"t\nrsync --server\n\x1f\x00\x00\x00")
            .expect("written");
        drop(relay);
        let piped = loop {
            if let Some(piped) = server.take().pop() {
                break piped;
            }
            std::thread::sleep(Duration::from_millis(5));
        };
        let mut rest = Vec::new();
        let mut stream = piped.stream;
        stream.read_to_end(&mut rest).expect("read");
        assert_eq!(rest, b"\x1f\x00\x00\x00");
    }

    #[test]
    fn a_relay_without_a_window_fails_and_a_silent_one_is_not_heard_r_ssh_55() {
        let free = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("a port");
        let closed = free.local_addr().expect("address").port();
        drop(free);
        assert!(relay(closed, "t", "c", io::empty(), Vec::new()).is_err());

        let server = PipeServer::start(|| {}).expect("a port");
        let mut silent =
            TcpStream::connect((Ipv4Addr::LOCALHOST, server.port())).expect("connected");
        silent.write_all(b"only a token\n").expect("written");
        drop(silent);
        std::thread::sleep(Duration::from_millis(200));
        assert!(server.take().is_empty());
        assert_eq!(
            command_of(&["rsync".into(), "--server".into()]),
            "rsync --server"
        );
    }
}
