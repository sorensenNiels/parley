//! A whole connection against a small SSH server in this process (russh's own): the host key question
//! (R-SSH-3), signing on with a password and its retries (R-SSH-4), with key files (R-SSH-12) and an agent and its
//! forwarding (R-SSH-13), jump hosts (R-SSH-43), the shell with its pseudo-terminal, output into
//! the terminal and the terminal's replies back, typing, resizing and the exit status (R-SSH-5). No real server.

#![expect(
    clippy::unused_async_trait_impl,
    reason = "the test server implements russh's async Handler trait"
)]

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use parley_ssh_core::Secret;
use parley_ssh_core::known_hosts::{HostKeyStatus, KnownHosts};
use parley_ssh_core::net::agent::AgentAddress;
use parley_ssh_core::net::jump::Jump;
use parley_ssh_core::net::{
    Command, Connection, Ending, Event, HostKeyAnswer, Options, Target, lock,
};
use parley_ssh_core::term::Terminal;
use russh::keys::agent::client::AgentClient;
use russh::keys::{PrivateKey, PublicKey};
use russh::server::{self, Auth, Msg, Session};
use russh::{Channel, ChannelId, MethodKind, MethodSet, Pty};

/// A throwaway Ed25519 host key made for this test.
const HOST_KEY: &str = "-----BEGIN OPENSSH PRIVATE KEY-----
b3BlbnNzaC1rZXktdjEAAAAABG5vbmUAAAAEbm9uZQAAAAAAAAABAAAAMwAAAAtzc2gtZW
QyNTUxOQAAACCe9egPtJOPenTzXkIV0OAdM2FVTU+OqQGHj0gfxBn1CgAAAIgHdvOOB3bz
jgAAAAtzc2gtZWQyNTUxOQAAACCe9egPtJOPenTzXkIV0OAdM2FVTU+OqQGHj0gfxBn1Cg
AAAECmA9WCWvckYOtIGSl5u3YqPWuoVnNHw+nwFNHXVMCM/5716A+0k496dPNeQhXQ4B0z
YVVNT46pAYePSB/EGfUKAAAAAAECAwQF
-----END OPENSSH PRIVATE KEY-----
";
const OTHER_KEY: &str =
    "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIAV5CINKVHRsJdOO4vr5nwVr0ASkuG8DODBufVj2pCQ9";
const PASSWORD: &str = "æøå-hemmelig";

/// A throwaway client key, not encrypted.
const CLIENT_KEY: &str = "-----BEGIN OPENSSH PRIVATE KEY-----
b3BlbnNzaC1rZXktdjEAAAAABG5vbmUAAAAEbm9uZQAAAAAAAAABAAAAMwAAAAtzc2gtZW
QyNTUxOQAAACAFeQiDSlR0bCXTjuL6+Z8Fa9AEpLhvAzgwbn1Y9qQkPQAAAIh6ftWfen7V
nwAAAAtzc2gtZWQyNTUxOQAAACAFeQiDSlR0bCXTjuL6+Z8Fa9AEpLhvAzgwbn1Y9qQkPQ
AAAECXCI4l4HDPIwcUS0V+zYG5CsJo842jllOriKgHsbVdSgV5CINKVHRsJdOO4vr5nwVr
0ASkuG8DODBufVj2pCQ9AAAAAAECAwQF
-----END OPENSSH PRIVATE KEY-----
";
/// A throwaway client key encrypted with [`PASSPHRASE`].
const ENCRYPTED_KEY: &str = "-----BEGIN OPENSSH PRIVATE KEY-----
b3BlbnNzaC1rZXktdjEAAAAACmFlczI1Ni1jdHIAAAAGYmNyeXB0AAAAGAAAABDgvps3CL
3vsJBTt8FVQPyqAAAAGAAAAAEAAAAzAAAAC3NzaC1lZDI1NTE5AAAAID4h3fHUfGd/+K7r
KTBQ1aDn4eeuDmwKgTzydjnS3MeeAAAAkOZlOjnJWzsGiivWyJz5ZQaWaP31Vh2Ht3UWwq
oHgZGzsxXsSh3uHiB6M4xe8UO8MTxO5R5kQwwCA7qczI1xmyb4PcsOcBLbr9ixQgwTSJzz
3DiJTwL32EtllwyLZyEIic3q6yX7Zy7xMVXOTm5F0Bqf7t7vpxJfqQZYFRxfz0nVpaVTsY
rhs2UO8rFas9LTdQ==
-----END OPENSSH PRIVATE KEY-----
";
const PASSPHRASE: &str = "æøå pass";
/// [`CLIENT_KEY`] in `PuTTY`'s format, version 3, not encrypted.
const PPK_KEY: &str = "PuTTY-User-Key-File-3: ssh-ed25519
Encryption: none
Comment: parley test
Public-Lines: 2
AAAAC3NzaC1lZDI1NTE5AAAAIAV5CINKVHRsJdOO4vr5nwVr0ASkuG8DODBufVj2
pCQ9
Private-Lines: 1
AAAAIJcIjiXgcM8jBxRLRX7NgbkKwmjzjaOWU6uIqAextV1K
Private-MAC: 5ae5626107004366d633f9948f22ffb48398eab366e8b35fdc259a8f2ad91f66
";
/// The public keys the server lets in.
const ALLOWED: [&str; 2] = [
    "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIAV5CINKVHRsJdOO4vr5nwVr0ASkuG8DODBufVj2pCQ9",
    "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAID4h3fHUfGd/+K7rKTBQ1aDn4eeuDmwKgTzydjnS3Mee",
];

/// The pseudo-terminal the client asked for.
#[derive(Debug, Clone)]
struct PtyRequest {
    term: String,
    columns: u32,
    rows: u32,
    modes: Vec<(Pty, u32)>,
}

/// What the server saw, for the test to check.
#[derive(Debug, Default)]
struct Seen {
    pty: Option<PtyRequest>,
    sizes: Vec<(u32, u32)>,
    input: Vec<u8>,
    /// The client asked for agent forwarding.
    agent_requested: bool,
    /// Where `direct-tcpip` channels were asked to go (R-SSH-43).
    forwarded: Vec<(String, u32)>,
}

struct Server {
    seen: Arc<Mutex<Seen>>,
    /// russh closes a session channel whose handle is dropped.
    channels: Vec<Channel<Msg>>,
}

impl server::Handler for Server {
    type Error = russh::Error;

    async fn auth_publickey(
        &mut self,
        _user: &str,
        key: server::VerifiedPublicKey<'_>,
    ) -> Result<Auth, Self::Error> {
        let allowed = ALLOWED.iter().any(|k| {
            PublicKey::from_openssh(k).is_ok_and(|a| a.key_data() == key.public_key.key_data())
        });
        let again = Auth::Reject {
            proceed_with_methods: Some(methods()),
            partial_success: false,
        };
        Ok(if allowed { Auth::Accept } else { again })
    }

    async fn agent_request(
        &mut self,
        _channel: ChannelId,
        _session: &mut Session,
    ) -> Result<bool, Self::Error> {
        self.seen.lock().expect("not poisoned").agent_requested = true;
        Ok(true)
    }

    async fn auth_password(&mut self, _user: &str, password: &str) -> Result<Auth, Self::Error> {
        // As OpenSSH: a refusal names the methods that can go on, so the client may try again.
        let again = Auth::Reject {
            proceed_with_methods: Some(password_only()),
            partial_success: false,
        };
        Ok(if password == PASSWORD {
            Auth::Accept
        } else {
            again
        })
    }

    async fn channel_open_session(
        &mut self,
        channel: Channel<Msg>,
        reply: server::ChannelOpenHandle,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        self.channels.push(channel);
        reply.accept().await;
        Ok(())
    }

    /// As OpenSSH's `direct-tcpip`: connects to the address asked for and carries the bytes (R-SSH-43).
    async fn channel_open_direct_tcpip(
        &mut self,
        channel: Channel<Msg>,
        host_to_connect: &str,
        port_to_connect: u32,
        _originator_address: &str,
        _originator_port: u32,
        reply: server::ChannelOpenHandle,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        self.seen
            .lock()
            .expect("not poisoned")
            .forwarded
            .push((host_to_connect.to_owned(), port_to_connect));
        let address = format!("{host_to_connect}:{port_to_connect}");
        match tokio::net::TcpStream::connect(address).await {
            Ok(mut socket) => {
                reply.accept().await;
                tokio::spawn(async move {
                    let mut channel = channel.into_stream();
                    // Either side closing ends the relay.
                    let _ = tokio::io::copy_bidirectional(&mut channel, &mut socket).await;
                });
            }
            Err(_) => {
                reply.reject(russh::ChannelOpenFailure::ConnectFailed).await;
            }
        }
        Ok(())
    }

    async fn pty_request(
        &mut self,
        _channel: ChannelId,
        term: &str,
        columns: u32,
        rows: u32,
        _pix_width: u32,
        _pix_height: u32,
        modes: &[(Pty, u32)],
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        let request = PtyRequest {
            term: term.to_owned(),
            columns,
            rows,
            modes: modes.to_vec(),
        };
        self.seen.lock().expect("not poisoned").pty = Some(request);
        Ok(())
    }

    async fn shell_request(
        &mut self,
        channel: ChannelId,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        session.channel_success(channel)?;
        // A greeting, and a primary device attributes query the terminal must answer.
        session.data(channel, "Velkommen æøå\r\n$ \x1b[c".as_bytes().to_vec())?;
        if self.seen.lock().expect("not poisoned").agent_requested {
            // As a program on the server would: ask the forwarded agent for its keys, and say how many.
            let handle = session.handle();
            tokio::spawn(async move {
                let count = match handle.channel_open_agent().await {
                    Ok(agent) => AgentClient::connect(agent.into_stream())
                        .request_identities()
                        .await
                        .map_or(0, |ids| ids.len()),
                    Err(_) => 0,
                };
                // The test reads the answer from the screen; a closed session has no screen to read.
                let _ = handle
                    .data(channel, format!("agent keys: {count}\r\n").into_bytes())
                    .await;
            });
        }
        Ok(())
    }

    async fn data(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        // russh also hands a `direct-tcpip` channel's data here: that is the relay's, not the shell's.
        if !self.channels.iter().any(|c| c.id() == channel) {
            return Ok(());
        }
        let input = {
            let mut seen = self.seen.lock().expect("not poisoned");
            seen.input.extend_from_slice(data);
            seen.input.clone()
        };
        if input.ends_with(b"exit\r") {
            session.data(channel, b"bye\r\n".to_vec())?;
            session.exit_status_request(channel, 3)?;
            session.eof(channel)?;
            session.close(channel)?;
        } else if !data.starts_with(b"\x1b[?") {
            session.data(channel, data.to_vec())?;
        }
        Ok(())
    }

    async fn window_change_request(
        &mut self,
        _channel: ChannelId,
        columns: u32,
        rows: u32,
        _pix_width: u32,
        _pix_height: u32,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        self.seen
            .lock()
            .expect("not poisoned")
            .sizes
            .push((columns, rows));
        Ok(())
    }
}

fn password_only() -> MethodSet {
    MethodSet::from(&[MethodKind::Password][..])
}

/// What the server offers: public keys and the password, as most do.
fn methods() -> MethodSet {
    MethodSet::from(&[MethodKind::PublicKey, MethodKind::Password][..])
}

/// Starts the server on a free local port; it serves every connection until the test ends.
fn start_server(seen: Arc<Mutex<Seen>>) -> SocketAddr {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        runtime.block_on(async move {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
                .await
                .expect("bind");
            tx.send(listener.local_addr().expect("address"))
                .expect("test waits");
            let config = Arc::new(server::Config {
                keys: vec![PrivateKey::from_openssh(HOST_KEY).expect("test key")],
                methods: methods(),
                auth_rejection_time: Duration::from_millis(10),
                auth_rejection_time_initial: Some(Duration::ZERO),
                ..server::Config::default()
            });
            loop {
                let Ok((socket, _)) = listener.accept().await else {
                    return;
                };
                let handler = Server {
                    seen: Arc::clone(&seen),
                    channels: Vec::new(),
                };
                let config = Arc::clone(&config);
                tokio::spawn(async move {
                    if let Ok(running) = server::run_stream(config, socket, handler).await {
                        // The session's end, good or bad, is the client's to report.
                        let _ = running.await;
                    }
                });
            }
        });
    });
    rx.recv().expect("server started")
}

fn known_hosts(name: &str) -> KnownHosts {
    let folder = std::env::temp_dir().join(format!("parley-ssh-net-{name}-{}", std::process::id()));
    // A previous run may have left it.
    let _ = std::fs::remove_dir_all(&folder);
    KnownHosts::new(folder.join("known_hosts"), None)
}

fn open(address: SocketAddr, known: &KnownHosts) -> (Connection, Arc<Mutex<Terminal>>) {
    open_with(address, known, Options::default())
}

fn open_with(
    address: SocketAddr,
    known: &KnownHosts,
    options: Options,
) -> (Connection, Arc<Mutex<Terminal>>) {
    let target = Target {
        host: address.ip().to_string(),
        port: address.port(),
        user: "tester".into(),
    };
    let terminal = Arc::new(Mutex::new(Terminal::new(80, 24)));
    let connection = Connection::open(target, options, known.clone(), Arc::clone(&terminal), || {});
    (connection, terminal)
}

/// The next event, waiting up to ten seconds.
fn next(connection: &Connection) -> Event {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(event) = connection.events().find(|e| !matches!(e, Event::Log(_))) {
            return event;
        }
        assert!(Instant::now() < deadline, "no event within ten seconds");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn wait_until(what: &str, mut done: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !done() {
        assert!(
            Instant::now() < deadline,
            "{what} did not happen within ten seconds"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn screen_text(terminal: &Mutex<Terminal>) -> String {
    lock(terminal).all_text()
}

#[test]
fn a_whole_session_from_host_key_to_exit_status_r_ssh_3_r_ssh_4_r_ssh_5() {
    let seen = Arc::new(Mutex::new(Seen::default()));
    let address = start_server(Arc::clone(&seen));
    let known = known_hosts("whole");
    let (connection, terminal) = open(address, &known);

    // R-SSH-3: a host never seen is asked about, with its SHA-256 fingerprint.
    let Event::HostKey(question) = next(&connection) else {
        panic!("host key question first")
    };
    assert_eq!(question.status, HostKeyStatus::Unknown);
    assert_eq!(question.key_type, "ssh-ed25519");
    assert!(
        question.fingerprint.starts_with("SHA256:"),
        "{}",
        question.fingerprint
    );
    connection.send(Command::HostKey(HostKeyAnswer::AcceptAndSave));
    assert!(matches!(next(&connection), Event::Secured { .. }));
    let saved = std::fs::read_to_string(known.own_file()).expect("saved");
    assert!(
        saved.starts_with(&format!("[127.0.0.1]:{} ssh-ed25519 ", address.port())),
        "{saved}"
    );

    // R-SSH-4: a wrong password is asked again; the right one signs on.
    let Event::Prompt(prompt) = next(&connection) else {
        panic!("password prompt")
    };
    assert_eq!(prompt.fields.len(), 1);
    assert!(!prompt.fields[0].echo, "a password is hidden");
    connection.send(Command::Answers(Some(vec![Secret::new("wrong".into())])));
    let after_wrong = next(&connection);
    assert!(
        matches!(after_wrong, Event::Prompt(_)),
        "asked again after a wrong password: {after_wrong:?}"
    );
    connection.send(Command::Answers(Some(vec![Secret::new(PASSWORD.into())])));
    assert_eq!(next(&connection), Event::Started);

    // R-SSH-5: a pseudo-terminal of the window's size, xterm-256color, UTF-8.
    let pty = seen
        .lock()
        .expect("not poisoned")
        .pty
        .clone()
        .expect("pty requested");
    assert_eq!(
        (pty.term.as_str(), pty.columns, pty.rows),
        ("xterm-256color", 80, 24)
    );
    assert!(pty.modes.contains(&(Pty::IUTF8, 1)));

    // Output reaches the terminal, and its answer to the query reaches the host (R-SSH-6).
    wait_until("the greeting", || {
        screen_text(&terminal).contains("Velkommen æøå")
    });
    wait_until("the reply", || {
        seen.lock()
            .expect("not poisoned")
            .input
            .starts_with(b"\x1b[?62;22c")
    });

    connection.send(Command::Resize {
        columns: 100,
        rows: 30,
    });
    wait_until("the new size", || {
        seen.lock()
            .expect("not poisoned")
            .sizes
            .contains(&(100, 30))
    });

    connection.send(Command::Input(b"ls\r".to_vec()));
    wait_until("the echo", || screen_text(&terminal).contains("$ ls"));
    connection.send(Command::Input(b"exit\r".to_vec()));
    assert_eq!(next(&connection), Event::Ended(Ending::Exited(3)));
    assert!(screen_text(&terminal).contains("bye"));

    // The saved key is known next time: no question.
    let (again, _) = open(address, &known);
    assert!(
        matches!(next(&again), Event::Secured { .. }),
        "no host key question for a saved key"
    );
    again.send(Command::Disconnect);
}

#[test]
fn a_changed_host_key_is_refused_unless_the_user_goes_on_r_ssh_3() {
    let address = start_server(Arc::new(Mutex::new(Seen::default())));
    let known = known_hosts("changed");
    std::fs::create_dir_all(known.own_file().parent().expect("folder")).expect("folder");
    std::fs::write(
        known.own_file(),
        format!("[127.0.0.1]:{} {OTHER_KEY}\n", address.port()),
    )
    .expect("write");
    let (connection, _) = open(address, &known);
    let Event::HostKey(question) = next(&connection) else {
        panic!("host key question")
    };
    assert!(
        matches!(question.status, HostKeyStatus::Changed { .. }),
        "{:?}",
        question.status
    );
    connection.send(Command::HostKey(HostKeyAnswer::Cancel));
    let Event::Ended(Ending::Failed(why)) = next(&connection) else {
        panic!("refused")
    };
    assert!(why.contains("host key was not accepted"), "{why}");
}

#[test]
fn cancelling_the_password_ends_the_connection_quietly_r_ssh_4() {
    let address = start_server(Arc::new(Mutex::new(Seen::default())));
    let known = known_hosts("cancel");
    let (connection, _) = open(address, &known);
    let Event::HostKey(_) = next(&connection) else {
        panic!("host key question")
    };
    connection.send(Command::HostKey(HostKeyAnswer::ConnectOnce));
    assert!(matches!(next(&connection), Event::Secured { .. }));
    assert!(!known.own_file().exists(), "Connect once saves nothing");
    let Event::Prompt(_) = next(&connection) else {
        panic!("password prompt")
    };
    connection.send(Command::Answers(None));
    assert_eq!(next(&connection), Event::Ended(Ending::Disconnected));
}

#[test]
fn a_port_nobody_listens_on_is_said_in_plain_words_r_ssh_2() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let address = listener.local_addr().expect("address");
    drop(listener);
    let (connection, _) = open(address, &known_hosts("refused"));
    let Event::Ended(Ending::Failed(why)) = next(&connection) else {
        panic!("failed")
    };
    assert!(
        why.contains("refused") || why.contains("could not"),
        "{why}"
    );
}

/// A file in a fresh temporary folder for `name`.
fn temp_file(name: &str, file: &str, text: &str) -> std::path::PathBuf {
    // Not `known_hosts`'s folder, which is emptied when the connection starts.
    let folder =
        std::env::temp_dir().join(format!("parley-ssh-file-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&folder).expect("folder");
    let path = folder.join(file);
    std::fs::write(&path, text).expect("written");
    path
}

/// Connects with `options`, trusting the host once, up to the first event after the key exchange.
fn signing_on(
    name: &str,
    options: Options,
) -> (Connection, Arc<Mutex<Terminal>>, Arc<Mutex<Seen>>) {
    let seen = Arc::new(Mutex::new(Seen::default()));
    let address = start_server(Arc::clone(&seen));
    let (connection, terminal) = open_with(address, &known_hosts(name), options);
    let Event::HostKey(_) = next(&connection) else {
        panic!("host key question")
    };
    connection.send(Command::HostKey(HostKeyAnswer::ConnectOnce));
    assert!(matches!(next(&connection), Event::Secured { .. }));
    (connection, terminal, seen)
}

#[test]
fn a_key_file_signs_on_without_a_password_r_ssh_12() {
    let key = temp_file("key", "id_ed25519", CLIENT_KEY);
    let options = Options {
        key_files: vec![key],
        ..Options::default()
    };
    let (connection, _, _) = signing_on("key", options);
    assert_eq!(next(&connection), Event::Started, "no prompt");
}

#[test]
fn a_putty_key_file_signs_on_too_r_ssh_12() {
    let key = temp_file("ppk", "key.ppk", PPK_KEY);
    let options = Options {
        key_files: vec![key],
        ..Options::default()
    };
    let (connection, _, _) = signing_on("ppk", options);
    assert_eq!(next(&connection), Event::Started, "no prompt");
}

#[test]
fn an_encrypted_key_asks_its_passphrase_again_when_wrong_r_ssh_12() {
    let key = temp_file("encrypted", "id_ed25519", ENCRYPTED_KEY);
    let options = Options {
        key_files: vec![key],
        ..Options::default()
    };
    let (connection, _, _) = signing_on("encrypted", options);
    let Event::Prompt(prompt) = next(&connection) else {
        panic!("passphrase prompt")
    };
    assert_eq!(prompt.name, "Key passphrase");
    assert!(!prompt.fields[0].echo);
    connection.send(Command::Answers(Some(vec![Secret::new("wrong".into())])));
    let Event::Prompt(again) = next(&connection) else {
        panic!("asked again")
    };
    assert!(
        again.instructions.contains("did not open"),
        "{}",
        again.instructions
    );
    connection.send(Command::Answers(Some(vec![Secret::new(PASSPHRASE.into())])));
    assert_eq!(next(&connection), Event::Started);
}

#[test]
fn a_key_file_that_cannot_be_read_is_said_and_the_password_still_works_r_ssh_12() {
    let missing = std::env::temp_dir().join("parley-ssh-no-such-key-æøå");
    let options = Options {
        key_files: vec![missing],
        ..Options::default()
    };
    let (connection, _, _) = signing_on("unreadable", options);
    let Event::Problem(problem) = next(&connection) else {
        panic!("said")
    };
    assert!(problem.contains("could not be read"), "{problem}");
    let Event::Prompt(_) = next(&connection) else {
        panic!("password prompt")
    };
    connection.send(Command::Answers(Some(vec![Secret::new(PASSWORD.into())])));
    assert_eq!(next(&connection), Event::Started);
}

/// Starts russh's own agent on a Unix socket, holding the client key.
#[cfg(unix)]
fn start_agent(name: &str) -> std::path::PathBuf {
    let folder =
        std::env::temp_dir().join(format!("parley-ssh-agent-{name}-{}", std::process::id()));
    // A previous run may have left it.
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).expect("folder");
    let socket = folder.join("agent.sock");
    let (tx, rx) = std::sync::mpsc::channel();
    let path = socket.clone();
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        runtime.block_on(async move {
            let listener = tokio::net::UnixListener::bind(&path).expect("bind");
            let incoming = futures::stream::unfold(listener, |l| async move {
                Some((l.accept().await.map(|(s, _)| s), l))
            });
            tokio::spawn(russh::keys::agent::server::serve(Box::pin(incoming), ()));
            let mut client = AgentClient::connect_uds(&path).await.expect("agent");
            let key = PrivateKey::from_openssh(CLIENT_KEY).expect("test key");
            client.add_identity(&key, &[]).await.expect("added");
            tx.send(()).expect("test waits");
            std::future::pending::<()>().await;
        });
    });
    rx.recv().expect("agent started");
    socket
}

#[cfg(unix)]
#[test]
fn the_agent_signs_on_and_is_forwarded_when_asked_r_ssh_13() {
    let socket = start_agent("forward");
    let options = Options {
        agent: Some(AgentAddress::Socket(socket)),
        agent_forwarding: true,
        ..Options::default()
    };
    let (connection, terminal, seen) = signing_on("agent", options);
    assert_eq!(
        next(&connection),
        Event::Started,
        "the agent's key, no prompt"
    );
    assert!(seen.lock().expect("not poisoned").agent_requested);
    wait_until("the server to use the agent", || {
        screen_text(&terminal).contains("agent keys: 1")
    });
}

#[cfg(unix)]
#[test]
fn an_agent_key_not_named_is_not_used_and_forwarding_stays_off_r_ssh_13() {
    let socket = start_agent("named");
    let options = Options {
        agent: Some(AgentAddress::Socket(socket)),
        agent_key: Some("SHA256:not-this-one".into()),
        ..Options::default()
    };
    let (connection, _, seen) = signing_on("named", options);
    let Event::Prompt(_) = next(&connection) else {
        panic!("password prompt: the agent's key was not used")
    };
    connection.send(Command::Answers(Some(vec![Secret::new(PASSWORD.into())])));
    assert_eq!(next(&connection), Event::Started);
    assert!(!seen.lock().expect("not poisoned").agent_requested);
}

/// A `Target` for a test server, signing on as `tester`.
fn target_of(address: SocketAddr) -> Target {
    Target {
        host: address.ip().to_string(),
        port: address.port(),
        user: "tester".into(),
    }
}

/// Answers the host key once and the password, checking both are for `address`.
fn sign_on_to(connection: &Connection, address: SocketAddr) {
    let event = next(connection);
    let other = event.clone();
    let Event::HostKey(question) = event else {
        panic!("host key question for {address}: {other:?}")
    };
    assert_eq!(
        question.port,
        address.port(),
        "the hop's own host key is asked about"
    );
    connection.send(Command::HostKey(HostKeyAnswer::ConnectOnce));
    let Event::Prompt(prompt) = next(connection) else {
        panic!("password prompt for {address}")
    };
    assert_eq!(
        prompt.host,
        target_of(address).label(),
        "the prompt names its hop"
    );
    connection.send(Command::Answers(Some(vec![Secret::new(PASSWORD.into())])));
}

#[test]
fn a_session_through_two_jump_hosts_checks_and_signs_on_to_each_r_ssh_43() {
    let first_seen = Arc::new(Mutex::new(Seen::default()));
    let second_seen = Arc::new(Mutex::new(Seen::default()));
    let first = start_server(Arc::clone(&first_seen));
    let second = start_server(Arc::clone(&second_seen));
    let target = start_server(Arc::new(Mutex::new(Seen::default())));
    let options = Options {
        jumps: [first, second]
            .map(|a| Jump {
                target: target_of(a),
                key_files: Vec::new(),
            })
            .to_vec(),
        ..Options::default()
    };
    let terminal = Arc::new(Mutex::new(Terminal::new(80, 24)));
    let connection = Connection::open(
        target_of(target),
        options,
        known_hosts("jumps"),
        Arc::clone(&terminal),
        || {},
    );
    sign_on_to(&connection, first);
    sign_on_to(&connection, second);
    let Event::HostKey(question) = next(&connection) else {
        panic!("the target's host key")
    };
    assert_eq!(question.port, target.port());
    connection.send(Command::HostKey(HostKeyAnswer::ConnectOnce));
    assert!(
        matches!(next(&connection), Event::Secured { .. }),
        "only the target's algorithms go to the status bar"
    );
    let Event::Prompt(prompt) = next(&connection) else {
        panic!("the target's password")
    };
    assert_eq!(prompt.host, target_of(target).label());
    connection.send(Command::Answers(Some(vec![Secret::new(PASSWORD.into())])));
    assert_eq!(next(&connection), Event::Started);
    wait_until("the greeting", || {
        screen_text(&terminal).contains("Velkommen æøå")
    });
    let to = |a: SocketAddr| vec![(a.ip().to_string(), u32::from(a.port()))];
    assert_eq!(
        first_seen.lock().expect("not poisoned").forwarded,
        to(second)
    );
    assert_eq!(
        second_seen.lock().expect("not poisoned").forwarded,
        to(target)
    );
}

#[test]
fn a_jump_host_that_cannot_reach_the_target_says_so_r_ssh_43() {
    let jump = start_server(Arc::new(Mutex::new(Seen::default())));
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let nobody = listener.local_addr().expect("address");
    drop(listener);
    let options = Options {
        jumps: vec![Jump {
            target: target_of(jump),
            key_files: Vec::new(),
        }],
        ..Options::default()
    };
    let terminal = Arc::new(Mutex::new(Terminal::new(80, 24)));
    let connection = Connection::open(
        target_of(nobody),
        options,
        known_hosts("jump-refused"),
        terminal,
        || {},
    );
    sign_on_to(&connection, jump);
    let Event::Ended(Ending::Failed(why)) = next(&connection) else {
        panic!("failed")
    };
    assert!(
        why.contains(&format!(
            "the jump host {} could not reach",
            target_of(jump).label()
        )),
        "{why}"
    );
}

#[test]
fn a_connection_that_never_started_ends_with_its_problem_r_ssh_43() {
    let terminal = Arc::new(Mutex::new(Terminal::new(80, 24)));
    let connection = Connection::failed("bad jump".into(), terminal);
    assert_eq!(
        next(&connection),
        Event::Ended(Ending::Failed("bad jump".into()))
    );
}
