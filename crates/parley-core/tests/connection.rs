//! The network layer against a fake host on a local TCP socket (R-NET-1).

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::{Duration, Instant};

use parley_core::net::{Connection, LinkState};
use parley_core::record::{self, Opcode};
use parley_core::telnet::Telnet;
use parley_core::{Aid, CodePage, Key, SessionConfig};

const IAC: u8 = 255;
const EOR: u8 = 239;
const TIMEOUT: Duration = Duration::from_secs(10);

/// Reads from the socket until `done` says the bytes so far are enough.
fn read_until(socket: &mut TcpStream, done: impl Fn(&[u8]) -> bool) -> Vec<u8> {
    let mut received = Vec::new();
    let mut buf = [0; 1024];
    while !done(&received) {
        let n = socket.read(&mut buf).expect("the client stays connected");
        assert!(n > 0, "the client closed the connection; got {received:?}");
        received.extend(&buf[..n]);
    }
    received
}

fn wait_for(what: &str, condition: impl Fn() -> bool) {
    let start = Instant::now();
    while !condition() {
        assert!(start.elapsed() < TIMEOUT, "timed out waiting for {what}");
        thread::sleep(Duration::from_millis(10));
    }
}

fn config(port: u16) -> SessionConfig {
    SessionConfig {
        host: "127.0.0.1".into(),
        port,
        ..SessionConfig::default()
    }
}

#[test]
fn a_session_negotiates_shows_the_host_screen_and_sends_enter() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a free local port");
    let port = listener.local_addr().expect("a local address").port();
    let host = thread::spawn(move || {
        let (mut socket, _) = listener.accept().expect("the client connects");
        socket
            .set_read_timeout(Some(TIMEOUT))
            .expect("a read timeout");
        // DO TERMINAL-TYPE, then SB TERMINAL-TYPE SEND.
        socket
            .write_all(&[IAC, 253, 24, IAC, 250, 24, 1, IAC, 240])
            .expect("send");
        read_until(&mut socket, |r| r.windows(10).any(|w| w == b"IBM-3179-2"));

        let page = CodePage::Ccsid37;
        let mut data = vec![0x04, 0x40, 0x04, 0x11, 0x00, 0x00, 0x11, 1, 1];
        data.extend("HELLO".chars().filter_map(|c| page.to_byte(c)));
        data.extend([0x04, 0x52, 0x00, 0x00]);
        socket
            .write_all(&Telnet::frame(&record::build(0, Opcode::PutGet, &data)))
            .expect("send");
        read_until(&mut socket, |r| r.ends_with(&[IAC, EOR]))
    });

    let connection = Connection::open(config(port), || {});
    wait_for("the screen", || {
        let session = connection.session();
        session
            .screen()
            .text(session.code_page())
            .starts_with("HELLO")
    });
    assert_eq!(connection.state(), LinkState::Connected);
    connection.session().key(Key::Aid(Aid::ENTER));

    let reply = host.join().expect("the fake host finishes");
    let record_start = reply.len() - 2 - 13;
    let record = record::parse(&reply[record_start..reply.len() - 2]).expect("a 5250 record");
    assert_eq!(
        (record.opcode, record.data),
        (Opcode::PutGet, &[1, 1, 0xF1][..])
    );

    connection.close();
    wait_for("the connection to close", || {
        matches!(connection.state(), LinkState::Closed(_))
    });
}

#[test]
fn a_refused_connection_says_where_it_tried() {
    // Bind and drop a listener, so the port is very likely closed.
    let port = TcpListener::bind("127.0.0.1:0")
        .and_then(|l| l.local_addr())
        .expect("a free local port")
        .port();
    let connection = Connection::open(config(port), || {});
    wait_for("the failure", || {
        matches!(connection.state(), LinkState::Closed(_))
    });
    let LinkState::Closed(reason) = connection.state() else {
        unreachable!()
    };
    assert!(reason.contains(&format!("127.0.0.1:{port}")), "{reason}");
}
