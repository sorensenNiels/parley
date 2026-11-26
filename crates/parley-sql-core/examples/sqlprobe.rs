//! Which ways to run SQL does an IBM i offer (R-NET-8)? Asks each candidate service who it is,
//! **without signing on**: no user and no password is ever sent. For deciding how Parley's SQL
//! window (IDEA-13) talks to the host.
//!
//! ```sh
//! cargo run -p parley-core --example sqlprobe -- HOST
//! cargo run -p parley-core --example sqlprobe -- HOST --sign-on USER [--tls] [--ccsid 277]
//! ```
//!
//! `--tools USER` signs on once and runs the SQL window's own queries (R-SQL-30..32, R-SQL-78..80): a CL
//! command that fails harmlessly, Explain for its message and an SQL one, the job log and the
//! library list; it prints counts, message texts and the current schema, never rows of data.
//!
//! The password is asked for, or read from the file `PARLEY_PASSWORD_FILE` names: that is for
//! the test profile the owner made for Claude to probe with (D22), never for PUB400.
//!
//! `--snippets USER` signs on once and runs each starter snippet (R-SQL-27), saying only whether
//! it ran and how many rows and columns came back, never the rows.
//!
//! `--sql USER [--query "SQL"]` instead runs a few read-only statements (or only the one given)
//! through the SQL helper (R-SQL-11, D20;
//! build it first with `make helper`): one sign-on through `JTOpen`, never retried.
//!
//! `--sign-on USER` instead signs on once to the sign-on server (R-SQL-2, R-NET-9): it asks for
//! the password without showing it, makes exactly one attempt and never retries, and writes
//! what passed both ways to `target/sqlprobe-signon-HOST.txt` with the password substitute and
//! the seeds zeroed. Only the owner runs it, on hosts where they have a user; never Claude.
//!
//! What it asks:
//! - **Port mapper** (449): where each host server listens (`as-database`, `as-signon`, …).
//! - **Sign-on server** (8476, 9476 with TLS): the exchange of attributes that comes before a
//!   sign-on; the reply holds the IBM i release and the password level.
//! - **Database host server** (8471, 9471 with TLS): that it takes a connection, and its TLS.
//! - **DRDA** (446, 447 with TLS): EXCSAT, the exchange of server attributes (Open Group DRDA, DDM
//!   reference), which names the server class and its release.
//! - **Mapepire** (8076): a WebSocket upgrade without credentials, which a Mapepire server
//!   refuses with an HTTP status that shows it is there.
//! - **SSH** (22, 2222): the server's banner, for running SQL through `db2util`.
//!
//! Each reply is also printed as hex, since the host-server formats are not published: a
//! wrong reading shows. This is an investigation tool; the chosen route gets its protocol code
//! in `parley-core` with recorded fixtures.

#![expect(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "a command-line tool"
)]

use std::fmt::Write as _;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::time::Duration;

use parley_core::CodePage;
use parley_sql_core::hostserver::{self, Param, Record, ServerId, portmap};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{ClientConfig, ClientConnection, DigitallySignedStruct, SignatureScheme, StreamOwned};

const TIMEOUT: Duration = Duration::from_secs(6);

/// Exchange attributes request and its reply, on the sign-on server.
const EXCHANGE_ATTRIBUTES: u16 = 0x7003;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(host) = args.first().cloned() else {
        eprintln!("parley: usage: sqlprobe HOST [--sign-on USER [--tls] [--ccsid 277]]");
        std::process::exit(2);
    };
    let flag = |name: &str| args.iter().any(|a| a == name);
    let value = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    if let Some(user) = value("--tools") {
        std::process::exit(tools(&host, &user, flag("--tls")));
    }
    if let Some(user) = value("--snippets") {
        std::process::exit(snippets(&host, &user, flag("--tls")));
    }
    if let Some(user) = value("--sql") {
        let query = value("--query");
        std::process::exit(sql(&host, &user, flag("--tls"), query.as_deref()));
    }
    if let Some(user) = value("--sign-on") {
        let ccsid = value("--ccsid").map_or(Some(CodePage::Ccsid37), |n| {
            CodePage::ALL
                .into_iter()
                .find(|c| c.ccsid().to_string() == n)
        });
        let Some(code_page) = ccsid else {
            eprintln!("parley: --ccsid takes 37, 277, 1140 or 1142");
            std::process::exit(2);
        };
        std::process::exit(sign_on(&host, &user, flag("--tls"), code_page));
    }
    println!("SQL routes on {host} (nothing is signed on, no credentials are sent)\n");
    port_mapper(&host);
    signon(&host, 8476, false);
    signon(&host, 9476, true);
    database(&host, 8471, false);
    database(&host, 9471, true);
    drda(&host, 446, false);
    drda(&host, 447, true);
    mapepire(&host, 8076);
    for port in [22, 2222] {
        ssh(&host, port);
    }
}

// ---------------------------------------------------------------- connections

fn tcp(host: &str, port: u16) -> Result<TcpStream, String> {
    let addresses = (host, port)
        .to_socket_addrs()
        .map_err(|e| format!("cannot resolve: {e}"))?;
    let mut last = "no address".to_owned();
    for address in addresses {
        match TcpStream::connect_timeout(&address, TIMEOUT) {
            Ok(stream) => {
                stream
                    .set_read_timeout(Some(TIMEOUT))
                    .map_err(|e| e.to_string())?;
                stream
                    .set_write_timeout(Some(TIMEOUT))
                    .map_err(|e| e.to_string())?;
                return Ok(stream);
            }
            Err(e) => last = e.to_string(),
        }
    }
    Err(format!("no connection: {last}"))
}

type Tls = StreamOwned<ClientConnection, TcpStream>;

/// A TLS connection, the certificate checked as Parley checks it (the system's trusted
/// certificates, D6). When that fails, the reason, and a second connection that does not check
/// the certificate, only so the service can still be identified: nothing secret is sent on it.
fn tls(host: &str, port: u16) -> Result<(Tls, String), String> {
    match tls_with(host, port, checked_config()?) {
        Ok(stream) => {
            let note = format!("certificate trusted, {}", tls_details(&stream));
            Ok((stream, note))
        }
        Err(checked) => {
            let stream = tls_with(host, port, unchecked_config())?;
            let note = format!(
                "certificate NOT trusted ({checked}); identified over an unchecked connection, {}",
                tls_details(&stream)
            );
            Ok((stream, note))
        }
    }
}

fn tls_with(host: &str, port: u16, config: ClientConfig) -> Result<Tls, String> {
    let mut tcp = tcp(host, port)?;
    let name = ServerName::try_from(host.to_owned()).map_err(|e| e.to_string())?;
    let mut connection =
        ClientConnection::new(Arc::new(config), name).map_err(|e| e.to_string())?;
    while connection.is_handshaking() {
        connection
            .complete_io(&mut tcp)
            .map_err(|e| e.to_string())?;
    }
    Ok(StreamOwned::new(connection, tcp))
}

fn tls_details(stream: &Tls) -> String {
    let version = stream
        .conn
        .protocol_version()
        .map_or_else(|| "?".to_owned(), |v| format!("{v:?}"));
    let suite = stream
        .conn
        .negotiated_cipher_suite()
        .map_or_else(|| "?".to_owned(), |s| format!("{:?}", s.suite()));
    let chain = stream.conn.peer_certificates().map_or(0, <[_]>::len);
    format!("{version}, {suite}, {chain} certificate(s) in the chain")
}

fn provider() -> Arc<rustls::crypto::CryptoProvider> {
    Arc::new(rustls::crypto::ring::default_provider())
}

fn checked_config() -> Result<ClientConfig, String> {
    let provider = provider();
    let verifier = rustls_platform_verifier::Verifier::new(Arc::clone(&provider))
        .map_err(|e| e.to_string())?;
    Ok(ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|e| e.to_string())?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(verifier))
        .with_no_client_auth())
}

fn unchecked_config() -> ClientConfig {
    let provider = provider();
    let schemes = provider
        .signature_verification_algorithms
        .supported_schemes();
    ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .expect("ring supports the default protocol versions")
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(Unchecked(schemes)))
        .with_no_client_auth()
}

/// Takes any certificate. Only for identifying a service whose certificate the system does
/// not trust; never in Parley itself (D6).
#[derive(Debug)]
struct Unchecked(Vec<SignatureScheme>);

impl rustls::client::danger::ServerCertVerifier for Unchecked {
    fn verify_server_cert(
        &self,
        _: &CertificateDer<'_>,
        _: &[CertificateDer<'_>],
        _: &ServerName<'_>,
        _: &[u8],
        _: UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _: &[u8],
        _: &CertificateDer<'_>,
        _: &DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _: &[u8],
        _: &CertificateDer<'_>,
        _: &DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.0.clone()
    }
}

/// Reads until `done` says the bytes so far are complete, the peer closes, or time runs out.
fn read_reply(stream: &mut impl Read, done: impl Fn(&[u8]) -> bool) -> Result<Vec<u8>, String> {
    let mut reply = Vec::new();
    let mut buffer = [0u8; 4096];
    while !done(&reply) {
        match stream.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => reply.extend_from_slice(buffer.get(..n).unwrap_or_default()),
            Err(e) if reply.is_empty() => return Err(format!("no reply: {e}")),
            Err(_) => break,
        }
    }
    Ok(reply)
}

fn heading(title: &str) {
    println!("== {title}");
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::new();
    for (i, chunk) in bytes.chunks(16).enumerate() {
        let _ = write!(out, "     {:04x} ", i * 16); // writing to a String cannot fail
        for b in chunk {
            let _ = write!(out, " {b:02x}");
        }
        out.push('\n');
    }
    out
}

fn u16_at(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes(bytes.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

fn ebcdic(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|&b| CodePage::Ccsid37.to_char(b))
        .collect::<String>()
        .trim_end()
        .to_owned()
}

fn to_ebcdic(text: &str) -> Vec<u8> {
    text.chars()
        .filter_map(|c| CodePage::Ccsid37.to_byte(c))
        .collect()
}

// ---------------------------------------------------------------- port mapper

/// The port mapper takes a service name in ASCII and answers `+` and the port (4 bytes), or
/// `-` when it does not know the service.
fn port_mapper(host: &str) {
    heading("Port mapper (449)");
    let services = [
        "as-signon",
        "as-signon-s",
        "as-database",
        "as-database-s",
        "as-central",
        "as-svrmap",
        "drda",
        "ddm",
    ];
    for service in services {
        let answer = tcp(host, 449).and_then(|mut stream| {
            stream
                .write_all(service.as_bytes())
                .map_err(|e| e.to_string())?;
            read_reply(&mut stream, portmap::reply_complete)
        });
        let text = match answer.map(|reply| portmap::parse_reply(&reply)) {
            Ok(Ok(Some(port))) => format!("port {port}"),
            Ok(Ok(None)) => "not known".to_owned(),
            Ok(Err(e)) => e.to_string(),
            Err(e) => e,
        };
        println!("   {service:<14} {text}");
    }
    println!();
}

// ---------------------------------------------------------------- sign-on server

/// The host-server header (20 bytes) and the exchange-attributes request: client version,
/// data stream level and a client seed. No user, no password.
fn exchange_attributes_request() -> Vec<u8> {
    let params = vec![
        Param::new(0x1101, 1u32.to_be_bytes()), // client version
        Param::new(0x1102, 2u16.to_be_bytes()), // client data stream level
        Param::new(0x1103, *b"PARLEY\x00\x01"), // client seed
    ];
    Record::request(ServerId::SignOn, EXCHANGE_ATTRIBUTES, params)
        .encode()
        .expect("a short request")
}

/// Complete when the length in the first 4 bytes has arrived.
fn host_server_reply_complete(reply: &[u8]) -> bool {
    matches!(hostserver::record_length(reply), Ok(Some(length)) if reply.len() >= length)
}

fn signon(host: &str, port: u16, secure: bool) {
    heading(&format!(
        "Sign-on server ({port}{})",
        if secure { ", TLS" } else { "" }
    ));
    let request = exchange_attributes_request();
    let reply = if secure {
        tls(host, port).and_then(|(mut stream, note)| {
            println!("   TLS: {note}");
            stream.write_all(&request).map_err(|e| e.to_string())?;
            read_reply(&mut stream, host_server_reply_complete)
        })
    } else {
        tcp(host, port).and_then(|mut stream| {
            stream.write_all(&request).map_err(|e| e.to_string())?;
            read_reply(&mut stream, host_server_reply_complete)
        })
    };
    match reply {
        Ok(reply) => describe_signon_reply(&reply),
        Err(e) => println!("   {e}"),
    }
    println!();
}

fn describe_signon_reply(reply: &[u8]) {
    let (Some(server), Some(template), Some(id)) =
        (u16_at(reply, 6), u16_at(reply, 16), u16_at(reply, 18))
    else {
        println!("   short reply ({} bytes):\n{}", reply.len(), hex(reply));
        return;
    };
    println!(
        "   reply {id:#06x} from server {server:#06x}, {} bytes",
        reply.len()
    );
    if let Some(rc) = u32_at(reply, 20).filter(|_| template >= 4) {
        println!("   return code {rc}");
    }
    let mut at = 20 + usize::from(template);
    while let (Some(length), Some(code_point)) = (u32_at(reply, at), u16_at(reply, at + 4)) {
        let length = length as usize;
        let Some(data) = reply.get(at + 6..at + length.max(6)) else {
            break;
        };
        let meaning = match code_point {
            0x1101 => u32_at(data, 0).map(|v| {
                format!(
                    "server version V{}R{}M{}",
                    (v >> 16) & 0xFF,
                    (v >> 8) & 0xFF,
                    v & 0xFF
                )
            }),
            0x1102 => u16_at(data, 0).map(|l| format!("server data stream level {l}")),
            0x1103 => Some("server seed (8 bytes)".to_owned()),
            0x1119 => data
                .first()
                .map(|l| format!("password level {l} (QPWDLVL)")),
            0x111F => data.get(4..).map(|name| format!("job {}", ebcdic(name))),
            _ => None,
        };
        println!(
            "   CP {code_point:#06x}: {}",
            meaning.unwrap_or_else(|| format!("{data:02x?}"))
        );
        if length < 6 {
            break;
        }
        at += length;
    }
    print!("   raw:\n{}", hex(reply));
}

// ---------------------------------------------------------------- database host server

fn database(host: &str, port: u16, secure: bool) {
    heading(&format!(
        "Database host server ({port}{})",
        if secure { ", TLS" } else { "" }
    ));
    let result = if secure {
        tls(host, port).map(|(_, note)| format!("TLS: {note}"))
    } else {
        tcp(host, port).map(|_| "takes a connection".to_owned())
    };
    // Its first request is a sign-on (exchange random seeds with a user): not sent here.
    println!("   {}\n", result.unwrap_or_else(|e| e));
}

// ---------------------------------------------------------------- DRDA

/// EXCSAT (DDM code point 0x1041) in a request DSS: who we are and the manager levels we would
/// use. The reply, EXCSATRD (0x1443), names the server.
fn excsat_request() -> Vec<u8> {
    let mut params = Vec::new();
    let mut param = |code_point: u16, data: &[u8]| {
        let length = u16::try_from(4 + data.len()).expect("a short parameter");
        params.extend_from_slice(&length.to_be_bytes());
        params.extend_from_slice(&code_point.to_be_bytes());
        params.extend_from_slice(data);
    };
    param(0x115E, &to_ebcdic("PARLEY-SQLPROBE")); // EXTNAM
    let mut levels = Vec::new();
    for (manager, level) in [
        (0x1403u16, 7u16), // AGENT
        (0x2407, 7),       // SQLAM
        (0x240F, 7),       // RDB
        (0x1440, 7),       // SECMGR
        (0x1474, 5),       // CMNTCPIP
    ] {
        levels.extend_from_slice(&manager.to_be_bytes());
        levels.extend_from_slice(&level.to_be_bytes());
    }
    param(0x1404, &levels); // MGRLVLLS
    param(0x1147, &to_ebcdic("PARLEY")); // SRVCLSNM
    param(0x116D, &to_ebcdic("PARLEY")); // SRVNAM
    param(0x115A, &to_ebcdic("PRL00100")); // SRVRLSLV
    let object_length = u16::try_from(4 + params.len()).expect("a short object");
    let mut request = Vec::new();
    request.extend_from_slice(&(object_length + 6).to_be_bytes()); // DSS length
    request.extend_from_slice(&[0xD0, 0x01]); // DSS magic, request DSS
    request.extend_from_slice(&1u16.to_be_bytes()); // correlation
    request.extend_from_slice(&object_length.to_be_bytes());
    request.extend_from_slice(&0x1041u16.to_be_bytes()); // EXCSAT
    request.extend_from_slice(&params);
    request
}

fn drda(host: &str, port: u16, secure: bool) {
    heading(&format!(
        "DRDA ({port}{})",
        if secure { ", TLS" } else { "" }
    ));
    let complete = |r: &[u8]| u16_at(r, 0).is_some_and(|length| r.len() >= usize::from(length));
    let reply = if secure {
        tls(host, port).and_then(|(mut stream, note)| {
            println!("   TLS: {note}");
            stream
                .write_all(&excsat_request())
                .map_err(|e| e.to_string())?;
            read_reply(&mut stream, complete)
        })
    } else {
        tcp(host, port).and_then(|mut stream| {
            stream
                .write_all(&excsat_request())
                .map_err(|e| e.to_string())?;
            read_reply(&mut stream, complete)
        })
    };
    match reply {
        Ok(reply) => describe_excsatrd(&reply),
        Err(e) => println!("   {e}"),
    }
    println!();
}

fn describe_excsatrd(reply: &[u8]) {
    let Some(code_point) = u16_at(reply, 8) else {
        println!("   short reply:\n{}", hex(reply));
        return;
    };
    println!("   reply object {code_point:#06x} ({} bytes)", reply.len());
    let mut at = 10;
    while let (Some(length), Some(cp)) = (u16_at(reply, at), u16_at(reply, at + 2)) {
        let length = usize::from(length);
        let Some(data) = reply.get(at + 4..at + length.max(4)) else {
            break;
        };
        let meaning = match cp {
            0x115E => format!("external name {}", ebcdic(data)),
            0x1147 => format!("server class {}", ebcdic(data)),
            0x116D => format!("server name {}", ebcdic(data)),
            0x115A => format!("server release {}", ebcdic(data)),
            0x1404 => {
                let pairs: Vec<String> = data
                    .chunks(4)
                    .filter_map(|p| Some(format!("{:#06x}={}", u16_at(p, 0)?, u16_at(p, 2)?)))
                    .collect();
                format!("manager levels {}", pairs.join(" "))
            }
            _ => format!("{data:02x?}"),
        };
        println!("   CP {cp:#06x}: {meaning}");
        if length < 4 {
            break;
        }
        at += length;
    }
    print!("   raw:\n{}", hex(reply));
}

// ---------------------------------------------------------------- Mapepire

/// A WebSocket upgrade with no credentials: Mapepire answers 401 (or similar), which shows that
/// it listens there.
fn mapepire(host: &str, port: u16) {
    heading(&format!("Mapepire ({port}, TLS)"));
    let request = format!(
        "GET / HTTP/1.1\r\nHost: {host}:{port}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\
         Sec-WebSocket-Key: cGFybGV5LXNxbHByb2Jl\r\nSec-WebSocket-Version: 13\r\n\r\n"
    );
    let ends_headers = |r: &[u8]| r.windows(4).any(|w| w == b"\r\n\r\n");
    let reply = tls(host, port).and_then(|(mut stream, note)| {
        println!("   TLS: {note}");
        stream
            .write_all(request.as_bytes())
            .map_err(|e| e.to_string())?;
        read_reply(&mut stream, ends_headers)
    });
    match reply {
        Ok(reply) => {
            let text = String::from_utf8_lossy(&reply);
            for line in text.lines().take_while(|l| !l.is_empty()).take(12) {
                println!("   < {line}");
            }
        }
        Err(e) => {
            println!("   {e}");
            // Without TLS, in case it runs plain.
            let plain = tcp(host, port).and_then(|mut stream| {
                stream
                    .write_all(request.as_bytes())
                    .map_err(|e| e.to_string())?;
                read_reply(&mut stream, ends_headers)
            });
            match plain {
                Ok(reply) => {
                    let text = String::from_utf8_lossy(&reply);
                    for line in text.lines().take_while(|l| !l.is_empty()).take(12) {
                        println!("   < (plain) {line}");
                    }
                }
                Err(e) => println!("   plain: {e}"),
            }
        }
    }
    println!();
}

// ---------------------------------------------------------------- SSH

fn ssh(host: &str, port: u16) {
    heading(&format!("SSH ({port})"));
    let banner =
        tcp(host, port).and_then(|mut stream| read_reply(&mut stream, |r| r.contains(&b'\n')));
    match banner {
        Ok(banner) => println!("   {}", String::from_utf8_lossy(&banner).trim()),
        Err(e) => println!("   {e}"),
    }
    println!();
}

// ---------------------------------------------------------------- sign-on (R-NET-9)

/// Signs on once and records what passed; the exit code: 0 signed on, 1 refused or failed.
fn sign_on(host: &str, user: &str, tls: bool, code_page: CodePage) -> i32 {
    use parley_core::SessionConfig;
    use parley_core::password::Password;
    use parley_sql_core::net::{self as net, Direction};

    println!(
        "Sign-on to {host} as {user}, {}",
        if tls {
            "over TLS"
        } else {
            "WITHOUT TLS: the session is not encrypted"
        }
    );
    println!("One attempt only: a refusal is not retried.\n");
    let prompt = format!("Password for {user} on {host} (not shown): ");
    let password = match rpassword::prompt_password(prompt) {
        Ok(text) => Password::new(text),
        Err(e) => {
            eprintln!("parley: cannot read the password: {e}");
            return 1;
        }
    };
    let config = SessionConfig {
        host: host.to_owned(),
        tls,
        ..SessionConfig::default()
    };
    let mut transcript = net::Transcript::new();
    let result = net::sign_on(&config, user, &password, code_page, Some(&mut transcript));
    drop(password);

    let file = format!("target/sqlprobe-signon-{host}.txt");
    let mut text = format!(
        "# Sign-on to {host}, {}; password substitute and seeds zeroed (R-NET-9)\n",
        if tls { "TLS" } else { "no TLS" }
    );
    for (direction, bytes) in &transcript {
        let arrow = match direction {
            Direction::Sent => "> sent",
            Direction::Received => "< received",
        };
        let _ = writeln!(text, "{arrow} {} bytes\n{}", bytes.len(), hex(bytes)); // to a String
    }
    match std::fs::write(&file, &text) {
        Ok(()) => println!("Recorded: {file}"),
        Err(e) => eprintln!("parley: cannot write {file}: {e}"),
    }

    match result {
        Ok(done) => {
            let a = &done.attributes;
            println!(
                "Signed on: {} at port {}, password level {}, sign-on job {}",
                a.release_name(),
                done.port,
                a.password_level,
                a.job.as_deref().unwrap_or("?")
            );
            if let Some(ccsid) = done.signed_on.server_ccsid {
                println!("Host CCSID: {ccsid}");
            }
            0
        }
        Err(e) => {
            println!("Not signed on: {e}");
            println!(
                "Not retried. Check the user and password before running again: each failed sign-on counts towards QMAXSIGN."
            );
            1
        }
    }
}

// ---------------------------------------------------------------- SQL through the helper

/// Statements that between them show each kind of value, a fetch in blocks and an error.
const STATEMENTS: [(&str, u32); 7] = [
    ("VALUES CURRENT SERVER", 10),
    (
        "SELECT CURRENT USER, CURRENT DATE, CURRENT TIME, CURRENT TIMESTAMP FROM SYSIBM.SYSDUMMY1",
        10,
    ),
    (
        "VALUES (CAST(12345678901234567890.123456 AS DECIMAL(31, 6)), CAST(-0.5 AS DECIMAL(5, 2)), \
         CAST(-12 AS NUMERIC(5, 0)), CAST(9223372036854775807 AS BIGINT), CAST(1.5E0 AS DOUBLE), \
         CAST('æøå ÆØÅ' AS VARCHAR(20)), CAST('' AS VARCHAR(5)), CAST(NULL AS INTEGER), \
         CAST(X'00C17F' AS VARBINARY(3)))",
        10,
    ),
    (
        "SELECT TABLE_SCHEMA, TABLE_NAME, TABLE_TYPE FROM QSYS2.SYSTABLES \
         WHERE TABLE_SCHEMA = 'QSYS2' FETCH FIRST 12 ROWS ONLY",
        5,
    ),
    // R-SQL-29: what a column says about itself (label, field, file, heading, text), also
    // under an alias.
    (
        "SELECT TABLE_NAME AS T, TABLE_SCHEMA, ROW_LENGTH FROM QSYS2.SYSTABLES \
         WHERE TABLE_SCHEMA = 'QSYS2' FETCH FIRST 2 ROWS ONLY",
        10,
    ),
    ("SELECT * FROM PARLEY_NO_SUCH_TABLE_42704", 10),
    ("VALUES CURRENT SERVER", 10),
];

/// Signs on once through the helper and runs [`STATEMENTS`]; the exit code: 0 all ran as
/// expected, 1 not.
fn sql(host: &str, user: &str, tls: bool, query: Option<&str>) -> i32 {
    let Some(mut helper) = connect(host, user, tls) else {
        return 1;
    };
    let mut as_expected = true;
    // `--query`: that statement alone, instead of the usual ones.
    let own = query.map(|q| [(q, 20)]);
    let statements: &[(&str, u32)] = match &own {
        Some(one) => one,
        None => &STATEMENTS,
    };
    for &(statement, max_rows) in statements {
        println!(
            "> {}",
            statement.split_whitespace().collect::<Vec<_>>().join(" ")
        );
        let ran = run_statement(&mut helper, statement, max_rows);
        if !ran && !statement.contains("NO_SUCH_TABLE") {
            as_expected = false;
        }
        println!();
    }
    helper.close();
    if as_expected {
        println!("All statements ran as expected.");
        0
    } else {
        println!("Some statements did not run as expected.");
        1
    }
}

/// Sends `request` and waits for its answer, read as `T`.
fn call<T: for<'de> serde::Deserialize<'de>>(
    helper: &mut parley_sql_core::helper::Helper,
    request: &parley_sql_core::protocol::Request,
) -> parley_sql_core::Result<T> {
    helper
        .call(request)?
        .ok_or_else(|| parley_sql_core::Error::SqlHelper("no answer".to_owned()))?
        .into_result()
}

/// The password: from the file `PARLEY_PASSWORD_FILE` names, for a test profile the owner set up
/// for Claude to probe with (the file is the owner's and never printed); else asked for.
fn password(user: &str, host: &str) -> Option<String> {
    if let Some(file) = std::env::var_os("PARLEY_PASSWORD_FILE") {
        return match std::fs::read_to_string(&file) {
            Ok(text) => Some(text.trim_end_matches(['\r', '\n']).to_owned()),
            Err(e) => {
                eprintln!("parley: cannot read PARLEY_PASSWORD_FILE: {e}");
                None
            }
        };
    }
    let prompt = format!("Password for {user} on {host} (not shown): ");
    rpassword::prompt_password(prompt)
        .map_err(|e| eprintln!("parley: cannot read the password: {e}"))
        .ok()
}

/// Starts the helper and connects once; `None` after saying why not.
fn connect(host: &str, user: &str, tls: bool) -> Option<parley_sql_core::helper::Helper> {
    use parley_core::password::Password;
    use parley_sql_core::helper::{Helper, HelperFiles};
    use parley_sql_core::protocol::{Connect, Connected, Hello, Request};

    let java = parley_sql_core::java::find(None)
        .map_err(|e| eprintln!("parley: {e}"))
        .ok()?;
    let files = HelperFiles::in_folder(std::path::Path::new("target/helpers"));
    let mut helper = Helper::start(&java.path, &files)
        .map_err(|e| eprintln!("parley: {e} (run make helper first)"))
        .ok()?;
    let hello: Hello = call(&mut helper, &Request::Hello)
        .map_err(|e| eprintln!("parley: {e}"))
        .ok()?;
    println!(
        "Helper: protocol {}, Java {}, {}",
        hello.protocol, hello.java, hello.jtopen
    );
    let how = if tls {
        "over TLS"
    } else {
        "WITHOUT TLS: the session is not encrypted"
    };
    println!("Connecting to {host} as {user}, {how}");
    println!("One attempt only: a refusal is not retried.\n");
    let password = password(user, host)?;
    let request = Request::Connect(Connect {
        host: host.to_owned(),
        tls,
        user: user.to_owned(),
        password: Password::new(password),
        ca_file: String::new(),
        options: parley_sql_core::options::ConnectOptions::default(),
    });
    match call::<Connected>(&mut helper, &request) {
        Ok(c) => {
            println!(
                "Connected: job {}, release {}, user {}\n",
                c.job, c.release, c.user
            );
            Some(helper)
        }
        Err(e) => {
            println!("Not connected: {e}");
            println!(
                "Not retried. Check the user and password before running again: each failed \
                 sign-on counts towards QMAXSIGN."
            );
            None
        }
    }
}

/// Runs one statement, fetching all its rows in blocks of `max_rows`, and prints what came;
/// false when it failed.
fn run_statement(
    helper: &mut parley_sql_core::helper::Helper,
    statement: &str,
    max_rows: u32,
) -> bool {
    use parley_sql_core::protocol::{MoreRows, Outcome, Request};

    let run = Request::Run {
        sql: statement.to_owned(),
        max_rows,
        timeout: 0,
        params: Vec::new(),
    };
    let (cursor, columns, mut rows, mut more) = match call::<Outcome>(helper, &run) {
        Ok(Outcome::Count { count }) => {
            println!("  {count} rows changed");
            return true;
        }
        Ok(Outcome::Rows {
            cursor,
            columns,
            rows,
            more,
        }) => (cursor, columns, rows, more),
        Err(e) => {
            println!("  error: {e}");
            return false;
        }
    };
    let heading: Vec<String> = columns
        .iter()
        .map(|c| {
            let align = if c.numeric { " right" } else { "" };
            format!(
                "{} {}({},{}){align}",
                c.name, c.type_name, c.length, c.scale
            )
        })
        .collect();
    println!("  columns: {}", heading.join(" | "));
    for c in &columns {
        println!(
            "    {}: field {:?}, file {:?}.{:?}, heading {:?}, text {:?}",
            c.name, c.field, c.schema, c.table, c.heading, c.text
        );
    }
    let mut blocks = 1;
    while more {
        match call::<MoreRows>(helper, &Request::Fetch { cursor, max_rows }) {
            Ok(next) => {
                rows.extend(next.rows);
                more = next.more;
                blocks += 1;
            }
            Err(e) => {
                println!("  fetch failed: {e}");
                return false;
            }
        }
    }
    for row in &rows {
        let cells: Vec<String> = row
            .iter()
            .map(|v| {
                v.as_ref()
                    .map_or_else(|| "NULL".to_owned(), |t| format!("{t:?}"))
            })
            .collect();
        println!("  {}", cells.join(" | "));
    }
    println!("  {} rows in {blocks} block(s)", rows.len());
    true
}

// ---------------------------------------------------------------- starter snippets (R-SQL-27)

/// Runs every starter snippet, its placeholders filled with defaults or harmless test values;
/// prints ok or the error, and row and column counts only.
fn snippets(host: &str, user: &str, tls: bool) -> i32 {
    use parley_sql_core::protocol::{Outcome, Request};
    use parley_sql_core::snippet::{STARTERS, Snippet, fill};

    let Some(mut helper) = connect(host, user, tls) else {
        return 1;
    };
    let test_value = |name: &str| match name {
        "library" => Some("QSYS".to_owned()),
        "object" => Some("QSYSOPR".to_owned()),
        _ => None,
    };
    let mut failed = 0;
    for (name, text) in STARTERS {
        let snippet = Snippet::parse(text, name);
        let sql = fill(&snippet.sql, test_value);
        let run = Request::Run {
            sql,
            max_rows: 5,
            timeout: 60,
            params: Vec::new(),
        };
        // Said first, so a snippet the host takes long over is named while it runs.
        println!("...     {}", snippet.title);
        match call::<Outcome>(&mut helper, &run) {
            Ok(Outcome::Rows {
                columns,
                rows,
                more,
                ..
            }) => {
                let more = if more { "+" } else { "" };
                println!(
                    "ok      {} ({}): {}{more} rows, {} columns",
                    snippet.title,
                    snippet.needs,
                    rows.len(),
                    columns.len()
                );
            }
            Ok(Outcome::Count { count }) => {
                println!("ok      {}: {count} rows changed", snippet.title);
            }
            Err(e) => {
                failed += 1;
                let first = e
                    .to_string()
                    .split(". ")
                    .next()
                    .unwrap_or_default()
                    .to_owned();
                println!("FAILED  {} ({}): {first}", snippet.title, snippet.needs);
            }
        }
    }
    helper.close();
    println!(
        "\n{} of {} starter snippets ran.",
        STARTERS.len() - failed,
        STARTERS.len()
    );
    i32::from(failed > 0)
}

/// The System node's views (R-SQL-78..80): the signed-on user's spooled files, message queue
/// and jobs, this job's job log, and the newest spooled file's text. Read-only; counts only.
fn services(
    run: &mut impl FnMut(&str, String) -> Result<parley_sql_core::protocol::Outcome, String>,
) -> bool {
    use parley_sql_core::protocol::Outcome;
    use parley_sql_core::services::{self, View};

    let mut good = true;
    let mut newest = None;
    for view in View::ALL {
        match run(view.title(), view.sql()) {
            Ok(Outcome::Rows { rows, columns, .. }) => {
                good &= columns.len() == view.columns().len();
                if view == View::SpooledFiles {
                    newest = rows.first().cloned();
                }
            }
            Ok(Outcome::Count { .. }) => {}
            Err(_) => good = false,
        }
    }
    good &= run("Job log of this job", services::job_log("*")).is_ok();
    if let Some(row) = newest {
        let cell = |n: usize| row.get(n).cloned().flatten().unwrap_or_default();
        match services::spooled_file_text(
            &cell(services::spooled::JOB),
            &cell(services::spooled::NAME),
            &cell(services::spooled::NUMBER),
        ) {
            Ok(sql) => good &= run("The newest spooled file's text", sql).is_ok(),
            Err(why) => {
                println!("FAILED  the newest spooled file: {why}");
                good = false;
            }
        }
    }
    good
}

/// `--tools`: the SQL window's IBM i tools against a real host (R-SQL-30..32).
fn tools(host: &str, user: &str, tls: bool) -> i32 {
    use parley_sql_core::protocol::{Outcome, Request};
    use parley_sql_core::{catalog, message, text};

    let Some(mut helper) = connect(host, user, tls) else {
        return 1;
    };
    let mut run = |label: &str, sql: String| -> Result<Outcome, String> {
        let request = Request::Run {
            sql,
            max_rows: catalog::MAX_OBJECTS + 1,
            timeout: 60,
            params: Vec::new(),
        };
        let outcome = call::<Outcome>(&mut helper, &request).map_err(|e| e.to_string());
        match &outcome {
            Ok(Outcome::Rows { rows, columns, .. }) => {
                println!(
                    "ok      {label}: {} rows, {} columns",
                    rows.len(),
                    columns.len()
                );
            }
            Ok(Outcome::Count { count }) => println!("ok      {label}: count {count}"),
            Err(e) => println!("error   {label}: {e}"),
        }
        outcome
    };
    let mut good = true;
    // R-SQL-30: QTEMP is always in the library list, so this fails with CPF2103 and changes nothing.
    let cl = run(
        "CL: ADDLIBLE QTEMP (expect CPF2103)",
        text::cl_call("ADDLIBLE QTEMP"),
    );
    let cl_id = cl
        .as_ref()
        .err()
        .and_then(|e| message::message_id(e).map(str::to_owned));
    println!("        message ID read from it: {cl_id:?}");
    let sql_error = run(
        "SELECT FROM a missing table (expect SQL0204)",
        "SELECT * FROM QTEMP.PARLEY_NO_SUCH_TABLE".to_owned(),
    );
    let sql_id = sql_error
        .as_ref()
        .err()
        .and_then(|e| message::message_id(e).map(str::to_owned));
    println!("        message ID read from it: {sql_id:?}");
    // R-SQL-31: Explain, from the job log and from the message file.
    for id in [cl_id, sql_id].into_iter().flatten() {
        for (label, sql) in [
            ("job log", message::from_job_log(&id)),
            ("message file", message::from_message_file(&id)),
        ] {
            match run(&format!("Explain {id} from the {label}"), sql) {
                Ok(Outcome::Rows { rows, .. }) => {
                    if let Some(row) = rows.first() {
                        let text = |n: usize| row.get(n).cloned().flatten().unwrap_or_default();
                        println!("        {}", text(0));
                        for line in message::second_level(&text(1)).lines().take(6) {
                            println!("        | {line}");
                        }
                    }
                }
                Ok(Outcome::Count { .. }) => {}
                Err(_) => good = false,
            }
        }
    }
    if run("Job log", message::JOB_LOG.to_owned()).is_err() {
        good = false;
    }
    // R-SQL-32
    match run("Library list and current schema", catalog::JOB.to_owned()) {
        Ok(Outcome::Rows { rows, .. }) => {
            let schema = rows
                .first()
                .and_then(|r| r.first().cloned().flatten())
                .unwrap_or_default();
            println!("        current schema {schema}");
        }
        _ => good = false,
    }
    good &= browser(&mut run, user);
    good &= services(&mut run);
    good &= parameters(&mut helper);
    helper.close();
    i32::from(!good)
}

/// R-SQL-40: a prepared statement's values as text and null, converted by IBM i; constants only.
fn parameters(helper: &mut parley_sql_core::helper::Helper) -> bool {
    use parley_sql_core::params;
    use parley_sql_core::protocol::{Outcome, Request};

    let statement = "SELECT CAST(:text AS VARCHAR(10)) AS T, CAST(? AS INTEGER) + 1 AS N, \
        CAST(:text AS VARCHAR(10)) AS AGAIN, CAST(? AS DATE) AS D FROM SYSIBM.SYSDUMMY1";
    let Some(markers) = params::markers(statement) else {
        return false;
    };
    let fields = [Some("æøå".to_owned()), Some("41".to_owned()), None];
    let run = Request::Run {
        sql: markers.sql.clone(),
        max_rows: 5,
        timeout: 60,
        params: markers.values(&fields),
    };
    match call::<Outcome>(helper, &run) {
        Ok(Outcome::Rows { rows, .. }) => {
            let row = rows.first().cloned().unwrap_or_default();
            let expected = vec![
                Some("æøå".to_owned()),
                Some("42".to_owned()),
                Some("æøå".to_owned()),
                None,
            ];
            let good = row == expected;
            println!(
                "{}  Parameters (text, number, the same name twice, null): {row:?}",
                if good { "ok    " } else { "WRONG " }
            );
            good
        }
        Ok(Outcome::Count { .. }) => false,
        Err(e) => {
            println!("error   Parameters: {e}");
            false
        }
    }
}

/// The schema browser's queries (R-SQL-34): counts per kind only, no object names.
fn browser(
    run: &mut impl FnMut(&str, String) -> Result<parley_sql_core::protocol::Outcome, String>,
    user: &str,
) -> bool {
    use parley_sql_core::catalog;
    use parley_sql_core::protocol::Outcome;

    let mut good = true;
    // The profile's own library too: NODEJT400's is the one Claude may look into (D22).
    let own = parley_sql_core::text::system_name(user);
    for library in ["QGPL", "QSYS2"].into_iter().chain(own.as_deref()) {
        match run(
            &format!("Objects in {library}"),
            catalog::objects(library, catalog::MAX_OBJECTS),
        ) {
            Ok(Outcome::Rows { rows, .. }) => {
                let objects = catalog::objects_from(&rows);
                for kind in catalog::Kind::ALL {
                    let n = objects.iter().filter(|o| o.kind == kind).count();
                    println!("        {}: {n}", kind.plural());
                }
            }
            _ => good = false,
        }
    }
    match run(
        "Columns of QSYS2.SYSTABLES",
        catalog::columns("QSYS2", "SYSTABLES"),
    ) {
        Ok(Outcome::Rows { rows, .. }) => {
            for column in catalog::columns_from(&rows).iter().take(3) {
                println!(
                    "        {} {} \"{}\"",
                    column.name, column.data_type, column.heading
                );
            }
        }
        _ => good = false,
    }
    // R-SQL-35: Show SQL; the number of lines only.
    match run(
        "Show SQL of QSYS2.SYSTABLES",
        catalog::ddl("QSYS2", "SYSTABLES", catalog::Kind::View),
    ) {
        Ok(Outcome::Rows { rows, .. }) => {
            println!("        {} lines", catalog::ddl_from(&rows).lines().count());
        }
        _ => good = false,
    }
    good
}
