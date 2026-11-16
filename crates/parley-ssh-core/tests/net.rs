//! A whole connection against a small SSH server in this process (russh's own): the host key question
//! (R-SSH-3), signing on with a password and its retries (R-SSH-4), the shell with its pseudo-terminal, output into
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
use parley_ssh_core::net::{Command, Connection, Ending, Event, HostKeyAnswer, Target, lock};
use parley_ssh_core::term::Terminal;
use russh::keys::PrivateKey;
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
}

struct Server {
    seen: Arc<Mutex<Seen>>,
    /// russh closes a session channel whose handle is dropped.
    channels: Vec<Channel<Msg>>,
}

impl server::Handler for Server {
    type Error = russh::Error;

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
        Ok(())
    }

    async fn data(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
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
                methods: password_only(),
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
    let target = Target {
        host: address.ip().to_string(),
        port: address.port(),
        user: "tester".into(),
    };
    let terminal = Arc::new(Mutex::new(Terminal::new(80, 24)));
    let connection = Connection::open(target, known.clone(), Arc::clone(&terminal), || {});
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
