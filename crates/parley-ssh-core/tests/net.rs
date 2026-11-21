//! A whole connection against a small SSH server in this process (russh's own): the host key question
//! (R-SSH-3), signing on with a password and its retries (R-SSH-4), with key files (R-SSH-12) and an agent and its
//! forwarding (R-SSH-13), jump hosts (R-SSH-43), the shell with its pseudo-terminal, output into
//! the terminal and the terminal's replies back, typing, resizing and the exit status (R-SSH-5), and commands on
//! side channels beside the shell (Phase 31, R-SSH-57, D58). No real server.

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
    AddressFamily, Command, Connection, Ending, Event, Exec, HostKeyAnswer, JobEnding, JobId,
    Options, Proxy, SignedOnWith, Stream, Target, lock,
};
use parley_ssh_core::term::Terminal;
use parley_ssh_core::transfer::{self, Check};
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
    /// The command asked for instead of a shell, and the environment set (R-SSH-28).
    command: Option<String>,
    env: Vec<(String, String)>,
    /// The user names passwords were tried for (R-SSH-62).
    users: Vec<String>,
    /// Refuse `tcpip-forward`, as `AllowTcpForwarding no` (R-SSH-54).
    refuse_forwarding: bool,
    /// The ports forwarded, and those whose forwarding was cancelled (R-SSH-54).
    forwards: Vec<u32>,
    cancelled: Vec<u32>,
}

struct Server {
    seen: Arc<Mutex<Seen>>,
    /// russh closes a session channel whose handle is dropped.
    channels: Vec<Channel<Msg>>,
    /// The listeners of forwarded ports, by port (R-SSH-54).
    listeners: std::collections::HashMap<u32, tokio::task::JoinHandle<()>>,
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

    async fn auth_password(&mut self, user: &str, password: &str) -> Result<Auth, Self::Error> {
        self.seen
            .lock()
            .expect("not poisoned")
            .users
            .push(user.to_owned());
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

    /// As OpenSSH's remote forwarding (RFC 4254 §7.1): listens on a free port of the address asked for and opens a
    /// `forwarded-tcpip` channel to the client for each connection to it (R-SSH-54).
    async fn tcpip_forward(
        &mut self,
        address: &str,
        port: &mut u32,
        session: &mut Session,
    ) -> Result<bool, Self::Error> {
        if self.seen.lock().expect("not poisoned").refuse_forwarding {
            return Ok(false);
        }
        let Ok(listener) = tokio::net::TcpListener::bind((address, 0)).await else {
            return Ok(false);
        };
        let bound = u32::from(listener.local_addr().map_err(russh::Error::from)?.port());
        *port = bound;
        self.seen.lock().expect("not poisoned").forwards.push(bound);
        let handle = session.handle();
        let address = address.to_owned();
        let listening = tokio::spawn(async move {
            while let Ok((mut socket, from)) = listener.accept().await {
                let Ok(channel) = handle
                    .channel_open_forwarded_tcpip(
                        address.clone(),
                        bound,
                        from.ip().to_string(),
                        u32::from(from.port()),
                    )
                    .await
                else {
                    continue;
                };
                tokio::spawn(async move {
                    let mut channel = channel.into_stream();
                    // Either side closing ends the relay.
                    let _ = tokio::io::copy_bidirectional(&mut channel, &mut socket).await;
                });
            }
        });
        self.listeners.insert(bound, listening);
        Ok(true)
    }

    async fn cancel_tcpip_forward(
        &mut self,
        _address: &str,
        port: u32,
        _session: &mut Session,
    ) -> Result<bool, Self::Error> {
        self.seen.lock().expect("not poisoned").cancelled.push(port);
        Ok(self.listeners.remove(&port).is_some_and(|l| {
            l.abort();
            true
        }))
    }

    /// Takes LANG and refuses everything else, as OpenSSH's default `AcceptEnv` (R-SSH-28).
    async fn env_request(
        &mut self,
        channel: ChannelId,
        name: &str,
        value: &str,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        if name == "LANG" {
            self.seen
                .lock()
                .expect("not poisoned")
                .env
                .push((name.to_owned(), value.to_owned()));
            session.channel_success(channel)?;
        } else {
            session.channel_failure(channel)?;
        }
        Ok(())
    }

    async fn exec_request(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        let command = String::from_utf8_lossy(data).into_owned();
        session.channel_success(channel)?;
        if side_job(&command, channel, session)? {
            return Ok(());
        }
        session.data(channel, format!("ran: {command}\r\n").into_bytes())?;
        self.seen.lock().expect("not poisoned").command = Some(command);
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

    /// The client has sent all a side channel's input: a program reading it to its end ends (R-SSH-55).
    async fn channel_eof(
        &mut self,
        channel: ChannelId,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        session.exit_status_request(channel, 0)?;
        session.eof(channel)?;
        session.close(channel)?;
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

/// The commands the side-channel tests run (Phase 31); `false` for any other command.
fn side_job(
    command: &str,
    channel: ChannelId,
    session: &mut Session,
) -> Result<bool, russh::Error> {
    if command == transfer::check_command() {
        session.data(
            channel,
            b"rsync  version 3.2.7  protocol version 31\nparley-rsync: /QOpenSys/pkgs/bin/rsync\n"
                .to_vec(),
        )?;
        session.extended_data(channel, 1, "advarsel: æøå\n".as_bytes().to_vec())?;
        session.exit_status_request(channel, 0)?;
        session.eof(channel)?;
        session.close(channel)?;
        return Ok(true);
    }
    match command {
        // Runs until the client ends it.
        "sleep forever" => Ok(true),
        // As rsync's server side, nothing said first; the input comes back (R-SSH-55).
        c if c.starts_with("rsync --server") => Ok(true),
        // As a program on the server would: ask the forwarded agent for its keys, and say how many.
        "agent keys" => {
            let handle = session.handle();
            tokio::spawn(async move {
                let count = match handle.channel_open_agent().await {
                    Ok(agent) => AgentClient::connect(agent.into_stream())
                        .request_identities()
                        .await
                        .map_or(0, |ids| ids.len()),
                    Err(_) => 0,
                };
                // The client may be gone; the test then fails on what it did not get.
                let _ = handle.data(channel, format!("{count}").into_bytes()).await;
                let _ = handle.exit_status_request(channel, 0).await;
                let _ = handle.eof(channel).await;
                let _ = handle.close(channel).await;
            });
            Ok(true)
        }
        _ => Ok(false),
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
                    listeners: std::collections::HashMap::new(),
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
    let Event::Secured { host_key, .. } = next(&connection) else {
        panic!("secured")
    };
    let public = PrivateKey::from_openssh(HOST_KEY)
        .expect("test key")
        .public_key()
        .to_openssh()
        .expect("encoded");
    assert_eq!(host_key, public, "the key a transfer trusts (R-SSH-53)");
    assert_eq!(host_key.split_whitespace().count(), 2, "{host_key}");
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
    assert_eq!(
        next(&connection),
        Event::Started {
            with: SignedOnWith::Password
        }
    );

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
    let Event::Ended(Ending::Refused(why)) = next(&connection) else {
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
fn a_session_without_a_user_asks_for_one_and_signs_on_as_it_r_ssh_62() {
    let seen = Arc::new(Mutex::new(Seen::default()));
    let address = start_server(Arc::clone(&seen));
    let known = known_hosts("no-user");
    let target = Target {
        user: String::new(),
        ..target_of(address)
    };
    let terminal = Arc::new(Mutex::new(Terminal::new(80, 24)));
    let connection = Connection::open(target, Options::default(), known, terminal, || {});
    let Event::HostKey(_) = next(&connection) else {
        panic!("host key question first")
    };
    connection.send(Command::HostKey(HostKeyAnswer::ConnectOnce));
    assert!(matches!(next(&connection), Event::Secured { .. }));

    let ask = |answer: &str| {
        let Event::Prompt(prompt) = next(&connection) else {
            panic!("user name prompt")
        };
        assert_eq!(prompt.fields.len(), 1);
        assert_eq!(prompt.fields[0].label, "User name");
        assert!(prompt.fields[0].echo, "a user name is shown as typed");
        assert_eq!(prompt.host, format!("127.0.0.1:{}", address.port()));
        connection.send(Command::Answers(Some(vec![Secret::new(answer.into())])));
    };
    ask("  ");
    ask(" æøå ");
    assert_eq!(next(&connection), Event::User("æøå".into()));

    let Event::Prompt(prompt) = next(&connection) else {
        panic!("password prompt")
    };
    assert!(
        prompt.fields[0].label.contains("æøå@127.0.0.1"),
        "{prompt:?}"
    );
    connection.send(Command::Answers(Some(vec![Secret::new(PASSWORD.into())])));
    assert_eq!(
        next(&connection),
        Event::Started {
            with: SignedOnWith::Password
        }
    );
    assert_eq!(seen.lock().expect("not poisoned").users, ["æøå"]);
    connection.send(Command::Disconnect);
}

#[test]
fn cancelling_the_user_name_ends_the_connection_quietly_r_ssh_62() {
    let address = start_server(Arc::new(Mutex::new(Seen::default())));
    let target = Target {
        user: String::new(),
        ..target_of(address)
    };
    let terminal = Arc::new(Mutex::new(Terminal::new(80, 24)));
    let connection = Connection::open(
        target,
        Options::default(),
        known_hosts("no-user-cancel"),
        terminal,
        || {},
    );
    let Event::HostKey(_) = next(&connection) else {
        panic!("host key question first")
    };
    connection.send(Command::HostKey(HostKeyAnswer::ConnectOnce));
    assert!(matches!(next(&connection), Event::Secured { .. }));
    let Event::Prompt(_) = next(&connection) else {
        panic!("user name prompt")
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
    assert_eq!(
        next(&connection),
        Event::Started {
            with: SignedOnWith::KeyFile
        },
        "no prompt"
    );
}

#[test]
fn a_putty_key_file_signs_on_too_r_ssh_12() {
    let key = temp_file("ppk", "key.ppk", PPK_KEY);
    let options = Options {
        key_files: vec![key],
        ..Options::default()
    };
    let (connection, _, _) = signing_on("ppk", options);
    assert_eq!(
        next(&connection),
        Event::Started {
            with: SignedOnWith::KeyFile
        },
        "no prompt"
    );
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
    assert_eq!(
        next(&connection),
        Event::Started {
            with: SignedOnWith::KeyFile
        }
    );
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
    assert_eq!(
        next(&connection),
        Event::Started {
            with: SignedOnWith::Password
        }
    );
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
        Event::Started {
            with: SignedOnWith::Agent
        },
        "the agent's key, no prompt"
    );
    assert!(seen.lock().expect("not poisoned").agent_requested);
    wait_until("the server to use the agent", || {
        screen_text(&terminal).contains("agent keys: 1")
    });
}

#[cfg(unix)]
#[test]
fn the_agent_s_keys_are_listed_on_request_r_ssh_67() {
    let socket = start_agent("listed");
    let options = Options {
        agent: Some(AgentAddress::Socket(socket)),
        ..Options::default()
    };
    let (connection, _, _) = signing_on("listed", options);
    assert_eq!(
        next(&connection),
        Event::Started {
            with: SignedOnWith::Agent
        }
    );
    connection.send(Command::AgentKeys);
    let Event::AgentKeys(Ok(keys)) = next(&connection) else {
        panic!("the agent's keys")
    };
    let client = PrivateKey::from_openssh(CLIENT_KEY).expect("test key");
    assert_eq!(keys.len(), 1);
    assert_eq!(
        keys[0].fingerprint,
        parley_ssh_core::known_hosts::fingerprint(client.public_key())
    );
}

#[test]
fn without_an_agent_listing_its_keys_says_so_r_ssh_67() {
    let (connection, _) = started("no-agent", Options::default());
    connection.send(Command::AgentKeys);
    assert_eq!(
        next(&connection),
        Event::AgentKeys(Err("No agent is running.".into()))
    );
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
    assert_eq!(
        next(&connection),
        Event::Started {
            with: SignedOnWith::Password
        }
    );
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
    assert_eq!(
        next(&connection),
        Event::Started {
            with: SignedOnWith::Password
        }
    );
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

/// Signs on with the password, trusting the host once, up to the shell started; the events in between are checked.
fn through_to_started(connection: &Connection) {
    loop {
        match next(connection) {
            Event::HostKey(_) => connection.send(Command::HostKey(HostKeyAnswer::ConnectOnce)),
            Event::Prompt(prompt) if prompt.name.is_empty() => {
                connection.send(Command::Answers(Some(vec![Secret::new(PASSWORD.into())])));
            }
            Event::Prompt(prompt) => panic!("unexpected prompt {prompt:?}"),
            Event::Started { .. } => return,
            Event::Ended(ending) => panic!("ended: {ending:?}"),
            Event::User(user) => panic!("asked for a user, got {user}"),
            Event::JobOutput { .. }
            | Event::JobEnded { .. }
            | Event::Door { .. }
            | Event::Forwarded { .. }
            | Event::AgentKeys(_) => panic!("a job before the shell"),
            Event::Secured { .. } | Event::Banner(_) | Event::Problem(_) | Event::Log(_) => {}
        }
    }
}

#[test]
fn a_command_runs_instead_of_the_shell_with_its_environment_and_terminal_type_r_ssh_28() {
    let seen = Arc::new(Mutex::new(Seen::default()));
    let address = start_server(Arc::clone(&seen));
    let options = Options {
        shell: parley_ssh_core::net::Shell {
            command: Some("tmux new -A -s main".into()),
            env: vec![
                ("LANG".into(), "da_DK.UTF-8".into()),
                ("SECRET".into(), "x".into()),
            ],
            terminal_type: "xterm".into(),
        },
        ..Options::default()
    };
    let (connection, terminal) = open_with(address, &known_hosts("command"), options);
    through_to_started(&connection);
    wait_until("the command's output", || {
        screen_text(&terminal).contains("ran: tmux new -A -s main")
    });
    let seen = seen.lock().expect("not poisoned");
    assert_eq!(seen.command.as_deref(), Some("tmux new -A -s main"));
    assert_eq!(
        seen.env,
        [("LANG".to_owned(), "da_DK.UTF-8".to_owned())],
        "SECRET refused"
    );
    assert_eq!(seen.pty.as_ref().map(|p| p.term.as_str()), Some("xterm"));
}

/// A proxy for one connection on a local port: `serve` speaks the proxy's side, then the bytes are relayed to the
/// address it read.
fn start_proxy<F, Fut>(serve: F) -> SocketAddr
where
    F: Fn(tokio::net::TcpStream) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = Option<(tokio::net::TcpStream, String)>> + Send,
{
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
            while let Ok((socket, _)) = listener.accept().await {
                if let Some((mut client, to)) = serve(socket).await
                    && let Ok(mut server) = tokio::net::TcpStream::connect(to).await
                {
                    tokio::spawn(async move {
                        // Either side closing ends the relay.
                        let _ = tokio::io::copy_bidirectional(&mut client, &mut server).await;
                    });
                }
            }
        });
    });
    rx.recv().expect("proxy started")
}

#[test]
fn a_socks5_proxy_with_a_password_carries_the_connection_r_ssh_29() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let address = start_server(Arc::new(Mutex::new(Seen::default())));
    let proxy = start_proxy(|mut s| async move {
        let mut greeting = [0; 4];
        s.read_exact(&mut greeting).await.ok()?;
        assert_eq!(
            greeting,
            [5, 2, 0, 2],
            "no authentication and user/password offered"
        );
        s.write_all(&[5, 2]).await.ok()?;
        let mut head = [0; 2];
        s.read_exact(&mut head).await.ok()?;
        let mut user = vec![0; usize::from(head[1])];
        s.read_exact(&mut user).await.ok()?;
        let mut len = [0; 1];
        s.read_exact(&mut len).await.ok()?;
        let mut password = vec![0; usize::from(len[0])];
        s.read_exact(&mut password).await.ok()?;
        assert_eq!(
            (user.as_slice(), password.as_slice()),
            (b"pxuser".as_slice(), "pxæøå".as_bytes())
        );
        s.write_all(&[1, 0]).await.ok()?;
        let mut request = [0; 5];
        s.read_exact(&mut request).await.ok()?;
        assert_eq!(
            request[..4],
            [5, 1, 0, 3],
            "CONNECT to a name the proxy resolves"
        );
        let mut host = vec![0; usize::from(request[4])];
        s.read_exact(&mut host).await.ok()?;
        let mut port = [0; 2];
        s.read_exact(&mut port).await.ok()?;
        s.write_all(&[5, 0, 0, 1, 127, 0, 0, 1, 0, 0]).await.ok()?;
        let to = format!(
            "{}:{}",
            String::from_utf8(host).ok()?,
            u16::from_be_bytes(port)
        );
        Some((s, to))
    });
    let options = Options {
        transport: parley_ssh_core::net::Transport {
            proxy: Proxy::Socks5 {
                host: "127.0.0.1".into(),
                port: proxy.port(),
                user: "pxuser".into(),
            },
            ..parley_ssh_core::net::Transport::default()
        },
        ..Options::default()
    };
    let (connection, _) = open_with(address, &known_hosts("socks"), options);
    let Event::Prompt(prompt) = next(&connection) else {
        panic!("the proxy's password")
    };
    assert_eq!(prompt.name, "Proxy password");
    assert!(
        prompt.host.starts_with("pxuser@127.0.0.1"),
        "{}",
        prompt.host
    );
    connection.send(Command::Answers(Some(vec![Secret::new("pxæøå".into())])));
    through_to_started(&connection);
}

#[test]
fn an_http_proxy_asks_for_its_password_when_it_answers_407_r_ssh_29() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let address = start_server(Arc::new(Mutex::new(Seen::default())));
    let proxy = start_proxy(|mut s| async move {
        let mut header = Vec::new();
        while !header.ends_with(b"\r\n\r\n") {
            let mut byte = [0; 1];
            s.read_exact(&mut byte).await.ok()?;
            header.push(byte[0]);
        }
        let text = String::from_utf8(header).ok()?;
        if !text.contains("Proxy-Authorization: Basic cHg6c2VjcmV0") {
            s.write_all(b"HTTP/1.1 407 Proxy Authentication Required\r\n\r\n")
                .await
                .ok()?;
            return None;
        }
        let to = text.split_whitespace().nth(1)?.to_owned();
        s.write_all(b"HTTP/1.1 200 Connection established\r\n\r\n")
            .await
            .ok()?;
        Some((s, to))
    });
    let options = Options {
        transport: parley_ssh_core::net::Transport {
            proxy: Proxy::Http {
                host: "127.0.0.1".into(),
                port: proxy.port(),
                user: "px".into(),
            },
            ..parley_ssh_core::net::Transport::default()
        },
        ..Options::default()
    };
    let (connection, _) = open_with(address, &known_hosts("http-proxy"), options);
    let Event::Prompt(prompt) = next(&connection) else {
        panic!("the proxy's password")
    };
    assert_eq!(prompt.name, "Proxy password");
    connection.send(Command::Answers(Some(vec![Secret::new("secret".into())])));
    through_to_started(&connection);
}

#[test]
fn an_http_proxy_without_a_user_says_what_is_missing_r_ssh_29() {
    use tokio::io::AsyncWriteExt;
    let proxy = start_proxy(|mut s| async move {
        let mut buffer = [0; 512];
        let _ = tokio::io::AsyncReadExt::read(&mut s, &mut buffer).await;
        s.write_all(b"HTTP/1.1 407 Proxy Authentication Required\r\n\r\n")
            .await
            .ok()?;
        None
    });
    let options = Options {
        transport: parley_ssh_core::net::Transport {
            proxy: Proxy::Http {
                host: "127.0.0.1".into(),
                port: proxy.port(),
                user: String::new(),
            },
            ..parley_ssh_core::net::Transport::default()
        },
        ..Options::default()
    };
    let target: SocketAddr = "127.0.0.1:9".parse().expect("address");
    let (connection, _) = open_with(target, &known_hosts("http-no-user"), options);
    let Event::Ended(Ending::Failed(why)) = next(&connection) else {
        panic!("failed")
    };
    assert!(why.contains("wants a user name and password"), "{why}");
}

#[cfg(unix)]
#[test]
fn a_proxy_command_carries_the_connection_r_ssh_29() {
    let address = start_server(Arc::new(Mutex::new(Seen::default())));
    // A relay, as `nc %h %p` would be; Python is on the machines that run these tests.
    let script = temp_file(
        "proxy-command",
        "relay.py",
        "import os, socket, sys, threading\n\
         s = socket.create_connection((sys.argv[1], int(sys.argv[2])))\n\
         def up():\n\
         \x20   while d := os.read(0, 65536):\n\
         \x20       s.sendall(d)\n\
         \x20   s.shutdown(socket.SHUT_WR)\n\
         threading.Thread(target=up, daemon=True).start()\n\
         while d := s.recv(65536):\n\
         \x20   os.write(1, d)\n",
    );
    let relay = format!("python3 -I {} %h %p", script.display());
    let options = Options {
        transport: parley_ssh_core::net::Transport {
            proxy: Proxy::Command(relay),
            ..parley_ssh_core::net::Transport::default()
        },
        ..Options::default()
    };
    let (connection, _) = open_with(address, &known_hosts("proxy-command"), options);
    through_to_started(&connection);
}

#[test]
fn ipv6_only_to_a_name_with_only_ipv4_says_so_r_ssh_26() {
    let options = Options {
        transport: parley_ssh_core::net::Transport {
            family: AddressFamily::V6,
            ..parley_ssh_core::net::Transport::default()
        },
        ..Options::default()
    };
    let target: SocketAddr = "127.0.0.1:9".parse().expect("address");
    let (connection, _) = open_with(target, &known_hosts("v6"), options);
    let Event::Ended(Ending::Failed(why)) = next(&connection) else {
        panic!("failed")
    };
    assert!(why.contains("no IPv6 address"), "{why}");
}

/// Signs on with the password up to the running shell.
fn started(name: &str, options: Options) -> (Connection, Arc<Mutex<Terminal>>) {
    let (connection, terminal, _) = started_seen(name, options);
    (connection, terminal)
}

fn started_seen(
    name: &str,
    options: Options,
) -> (Connection, Arc<Mutex<Terminal>>, Arc<Mutex<Seen>>) {
    let (connection, terminal, seen) = signing_on(name, options);
    if let Event::Prompt(_) = next(&connection) {
        connection.send(Command::Answers(Some(vec![Secret::new(PASSWORD.into())])));
        assert_eq!(
            next(&connection),
            Event::Started {
                with: SignedOnWith::Password
            }
        );
    }
    (connection, terminal, seen)
}

/// What job `id` wrote to stdout and stderr, and how it ended.
fn job(connection: &Connection, id: JobId) -> (String, String, JobEnding) {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    loop {
        match next(connection) {
            Event::JobOutput {
                id: of,
                stream,
                data,
            } if of == id => match stream {
                Stream::Out => out.extend(data),
                Stream::Err => err.extend(data),
            },
            Event::JobEnded { id: of, ending } if of == id => {
                return (
                    String::from_utf8_lossy(&out).into_owned(),
                    String::from_utf8_lossy(&err).into_owned(),
                    ending,
                );
            }
            _ => {}
        }
    }
}

fn exec(id: u64, command: &str, agent: bool) -> Command {
    Command::Exec(Exec {
        id: JobId(id),
        command: command.to_owned(),
        agent,
    })
}

#[test]
fn a_command_on_a_side_channel_reports_its_output_and_leaves_the_screen_alone_r_ssh_57() {
    let (connection, terminal) = started("side", Options::default());
    wait_until("the greeting", || {
        screen_text(&terminal).contains("Velkommen")
    });
    let before = screen_text(&terminal);
    connection.send(exec(1, &transfer::check_command(), false));
    let (out, err, ending) = job(&connection, JobId(1));
    assert_eq!(ending, JobEnding::Exited(0));
    assert_eq!(err, "advarsel: æøå\n");
    let Check::Usable { version, path } = transfer::check(Some(0), &out) else {
        panic!("a usable rsync: {out}")
    };
    assert_eq!(
        (version.to_string().as_str(), path.as_str()),
        ("3.2.7", "/QOpenSys/pkgs/bin/rsync")
    );
    assert_eq!(
        screen_text(&terminal),
        before,
        "nothing of it on the screen"
    );
    connection.send(Command::Input(b"ls\r".to_vec()));
    wait_until("the shell still answers", || {
        screen_text(&terminal).contains("ls")
    });
}

#[test]
fn ending_a_job_stops_it_and_says_so_r_ssh_56() {
    let (connection, _) = started("stop", Options::default());
    connection.send(exec(7, "sleep forever", false));
    connection.send(Command::EndJob(JobId(7)));
    let (_, _, ending) = job(&connection, JobId(7));
    assert_eq!(ending, JobEnding::Stopped);
    connection.send(Command::EndJob(JobId(7)));
    connection.send(exec(8, &transfer::check_command(), false));
    assert_eq!(
        job(&connection, JobId(8)).2,
        JobEnding::Exited(0),
        "ending it twice is harmless"
    );
}

#[test]
fn a_job_cannot_use_an_agent_that_is_not_running_d58() {
    let (connection, _) = started("no-agent", Options::default());
    connection.send(exec(2, "agent keys", true));
    let (_, _, ending) = job(&connection, JobId(2));
    assert!(
        matches!(ending, JobEnding::Refused(ref why) if why.contains("No agent")),
        "{ending:?}"
    );
}

#[cfg(unix)]
#[test]
fn the_agent_is_forwarded_only_while_a_job_that_asked_runs_d58() {
    let socket = start_agent("job");
    let options = Options {
        agent: Some(AgentAddress::Socket(socket)),
        ..Options::default()
    };
    let (connection, _) = started("job-agent", options);
    connection.send(exec(1, "agent keys", true));
    assert_eq!(job(&connection, JobId(1)).0, "1", "the job's agent answers");
    connection.send(exec(2, "agent keys", false));
    assert_eq!(
        job(&connection, JobId(2)).0,
        "0",
        "refused once no job asks for it"
    );
}

/// The answer to `Command::Forward` on `connection`.
fn forwarded(connection: &Connection, id: JobId) -> Result<u16, String> {
    loop {
        if let Event::Forwarded { id: of, port } = next(connection)
            && of == id
        {
            return port;
        }
    }
}

/// What a connection to `port` on this machine says first, up to its first line end.
fn first_line_at(port: u16) -> std::io::Result<String> {
    use std::io::Read;
    let mut socket = std::net::TcpStream::connect(("127.0.0.1", port))?;
    socket.set_read_timeout(Some(Duration::from_secs(10)))?;
    let mut said = Vec::new();
    let mut byte = [0u8];
    while said.last() != Some(&b'\n') && socket.read(&mut byte)? == 1 {
        said.push(byte[0]);
    }
    Ok(String::from_utf8_lossy(&said).into_owned())
}

#[test]
fn a_port_forwarded_on_one_server_reaches_the_other_servers_ssh_port_through_parley_r_ssh_54() {
    let (a, _, seen_a) = started_seen("relay-a", Options::default());
    let (b, _, seen_b) = started_seen("relay-b", Options::default());
    b.send(Command::OpenDoor(JobId(1)));
    let door = loop {
        if let Event::Door { id, door } = next(&b) {
            assert_eq!(id, JobId(1));
            break door;
        }
    };
    a.send(Command::Forward { id: JobId(2), door });
    let port = forwarded(&a, JobId(2)).expect("a port");
    assert_eq!(
        seen_a.lock().expect("not poisoned").forwards,
        [u32::from(port)]
    );

    // rsync's ssh on A connecting to the forwarded port meets B's SSH server, through Parley.
    let said = first_line_at(port).expect("connected");
    assert!(said.starts_with("SSH-2.0-"), "{said:?}");
    let b_port = seen_b.lock().expect("not poisoned").forwarded.clone();
    assert_eq!(b_port.len(), 1, "one direct-tcpip on B");
    assert_eq!(
        b_port[0].0, "127.0.0.1",
        "B's own SSH port, as B reaches it"
    );

    a.send(Command::Unforward(JobId(2)));
    wait_until("the forwarding is cancelled", || {
        seen_a.lock().expect("not poisoned").cancelled == [u32::from(port)]
    });
}

#[test]
fn a_server_that_does_not_forward_says_so_in_plain_words_r_ssh_54() {
    let (a, _, seen_a) = started_seen("relay-refused-a", Options::default());
    let (b, _, _) = started_seen("relay-refused-b", Options::default());
    seen_a.lock().expect("not poisoned").refuse_forwarding = true;
    b.send(Command::OpenDoor(JobId(1)));
    let door = loop {
        if let Event::Door { door, .. } = next(&b) {
            break door;
        }
    };
    a.send(Command::Forward { id: JobId(2), door });
    let refused = forwarded(&a, JobId(2)).expect_err("refused");
    assert!(refused.contains("AllowTcpForwarding"), "{refused}");
}

/// Both ends of a loopback connection: what the relay would hold, and what the window hands the connection.
fn socket_pair() -> (std::net::TcpStream, std::net::TcpStream) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let relay =
        std::net::TcpStream::connect(listener.local_addr().expect("address")).expect("connect");
    let (window, _) = listener.accept().expect("accept");
    (relay, window)
}

#[test]
fn a_pipe_carries_a_relays_data_through_an_exec_channel_and_back_r_ssh_55() {
    use std::io::{Read, Write};
    let (connection, terminal) = started("pipe", Options::default());
    let (mut relay, window) = socket_pair();
    connection.send(Command::Pipe {
        id: JobId(5),
        command: "rsync --server -logDtprze.iLsfxCIvu . /tmp".into(),
        stream: window,
    });
    relay.write_all("hej æøå".as_bytes()).expect("written");
    relay
        .shutdown(std::net::Shutdown::Write)
        .expect("half closed");
    relay
        .set_read_timeout(Some(Duration::from_secs(10)))
        .expect("timeout");
    let mut back = Vec::new();
    relay.read_to_end(&mut back).expect("read to the end");
    assert_eq!(
        String::from_utf8_lossy(&back),
        "hej æøå",
        "the test server echoes it"
    );
    let (out, _, ending) = job(&connection, JobId(5));
    assert_eq!(out, "", "the data went to the relay, not the UI");
    assert_eq!(ending, JobEnding::Exited(0));
    let screen = screen_text(&terminal);
    assert!(!screen.contains("hej"), "nor onto the screen: {screen}");
}

#[test]
fn ending_a_pipe_closes_the_relays_connection_r_ssh_55() {
    use std::io::Read;
    let (connection, _) = started("pipe-end", Options::default());
    let (mut relay, window) = socket_pair();
    connection.send(Command::Pipe {
        id: JobId(6),
        command: "sleep forever".into(),
        stream: window,
    });
    // The channel is open once the command runs: a short wait, then the end.
    std::thread::sleep(Duration::from_millis(200));
    connection.send(Command::EndJob(JobId(6)));
    assert_eq!(job(&connection, JobId(6)).2, JobEnding::Stopped);
    relay
        .set_read_timeout(Some(Duration::from_secs(10)))
        .expect("timeout");
    let mut rest = Vec::new();
    relay.read_to_end(&mut rest).expect("closed, not timed out");
    assert_eq!(rest, Vec::<u8>::new());
}
