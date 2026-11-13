//! The host servers over the network: finding a server's port (R-SQL-1; the protocol is in
//! [`crate::hostserver::portmap`], with a fall back to the usual port) and signing on (R-SQL-2;
//! the messages are in [`crate::hostserver::signon`]).

use std::io::{Read, Write};
use std::time::Duration;

use crate::CodePage;
use crate::error::{Error, Result};
use crate::hostserver::portmap::{self, Service};
use crate::hostserver::signon::{self, ServerAttributes, SignedOn};
use crate::hostserver::substitute::{Profile, random_seed};
use crate::hostserver::{self, Record};
use crate::password::Password;
use crate::session::SessionConfig;

use super::Stream;

/// How long the port mapper may take to answer.
const MAPPER_TIMEOUT: Duration = Duration::from_secs(5);

/// Where `service` listens on `host`: what the port mapper says, or the usual port when it
/// cannot be reached or does not know the service (a refused connection then says more than
/// a failed lookup would).
pub fn port(host: &str, service: Service, tls: bool) -> u16 {
    ask_port_mapper(host, portmap::PORT, service, tls)
        .ok()
        .flatten()
        .unwrap_or_else(|| service.usual_port(tls))
}

/// Asks the port mapper on `host`:`mapper_port` where `service` listens.
pub fn ask_port_mapper(
    host: &str,
    mapper_port: u16,
    service: Service,
    tls: bool,
) -> Result<Option<u16>> {
    let mut stream = super::connect_tcp(host, mapper_port)?;
    stream.set_read_timeout(Some(MAPPER_TIMEOUT))?;
    stream.set_write_timeout(Some(MAPPER_TIMEOUT))?;
    stream.write_all(portmap::request(service, tls))?;
    let mut reply = Vec::new();
    let mut buffer = [0u8; 8];
    while !portmap::reply_complete(&reply) {
        let n = stream.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        reply.extend_from_slice(buffer.get(..n).unwrap_or_default());
    }
    portmap::parse_reply(&reply)
}

/// How long a host server may take to answer a request.
const ANSWER_TIMEOUT: Duration = Duration::from_secs(30);

/// Which way a record went, in a [`Transcript`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Sent,
    Received,
}

/// The records of a sign-on, for test fixtures (R-NET-9): the password substitute and both
/// seeds are zeroed before they are kept.
pub type Transcript = Vec<(Direction, Vec<u8>)>;

/// What a sign-on found out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignOnResult {
    pub attributes: ServerAttributes,
    pub signed_on: SignedOn,
    pub port: u16,
}

/// Signs `user` on to the sign-on server of `config`'s host (R-SQL-2), over TLS when
/// `config.tls`, checked as the 5250 connection checks it (D6). Makes exactly one sign-on
/// attempt: a user or password the host's password level cannot carry is refused before
/// anything is sent, and a refusal is never retried. Ends the server's job afterwards.
pub fn sign_on(
    config: &SessionConfig,
    user: &str,
    password: &Password,
    code_page: CodePage,
    transcript: Option<&mut Transcript>,
) -> Result<SignOnResult> {
    let port = port(&config.host, Service::SignOn, config.tls);
    sign_on_at(config, port, user, password, code_page, transcript)
}

/// [`sign_on`] to the sign-on server on `port`.
fn sign_on_at(
    config: &SessionConfig,
    port: u16,
    user: &str,
    password: &Password,
    code_page: CodePage,
    mut transcript: Option<&mut Transcript>,
) -> Result<SignOnResult> {
    let profile = Profile::new(user)?;
    let tcp = super::connect_tcp(&config.host, port)?;
    let mut stream: Box<dyn Stream> = if config.tls {
        Box::new(super::tls::connect(config, tcp, ANSWER_TIMEOUT)?)
    } else {
        tcp.set_read_timeout(Some(ANSWER_TIMEOUT))?;
        Box::new(tcp)
    };
    let client_seed = random_seed();
    let mut log = |direction, record: &Record| {
        if let Some(transcript) = transcript.as_deref_mut() {
            transcript.push((direction, blanked(record)));
        }
    };

    let request = signon::exchange_attributes(client_seed);
    send(&mut stream, &request)?;
    log(Direction::Sent, &request);
    let reply = receive(&mut stream)?;
    log(Direction::Received, &reply);
    let attributes = signon::read_attributes(&reply)?;

    let request = signon::sign_on(&attributes, &profile, password, client_seed, code_page)?;
    send(&mut stream, &request)?;
    log(Direction::Sent, &request);
    let reply = receive(&mut stream)?;
    log(Direction::Received, &reply);
    let outcome = signon::read_sign_on(&reply);

    // Ends the job either way; the host may close first, which is no error.
    let end = signon::end();
    if send(&mut stream, &end).is_ok() {
        log(Direction::Sent, &end);
    }
    Ok(SignOnResult {
        attributes,
        signed_on: outcome?,
        port,
    })
}

fn send(stream: &mut dyn Stream, record: &Record) -> Result<()> {
    stream.write_all(&record.encode()?)?;
    stream.flush()?;
    Ok(())
}

/// Reads one whole record.
fn receive(stream: &mut dyn Stream) -> Result<Record> {
    let mut bytes = vec![0; 4];
    stream.read_exact(&mut bytes)?;
    let length = hostserver::record_length(&bytes)?
        .ok_or_else(|| Error::host_server("a record without a length".to_owned()))?;
    bytes.resize(length, 0);
    stream.read_exact(bytes.get_mut(4..).unwrap_or_default())?;
    Record::decode(&bytes)
}

/// `record` as sent or received, with the password substitute (0x1105) and the seeds (0x1103)
/// zeroed: a fixture must not let anyone work the password out.
fn blanked(record: &Record) -> Vec<u8> {
    let mut copy = record.clone();
    for param in &mut copy.params {
        if matches!(param.code_point, 0x1103 | 0x1105) {
            param.data.fill(0);
        }
    }
    copy.encode().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::net::TcpListener;
    use std::thread;

    use super::*;

    /// A port mapper on a local port that answers `answer` to the first name it gets; returns
    /// its port and, when joined, the name it was asked.
    fn fake_mapper(answer: Vec<u8>) -> (u16, thread::JoinHandle<Vec<u8>>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a local port");
        let port = listener.local_addr().expect("its address").port();
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("a client");
            let mut asked = [0u8; 32];
            let n = stream.read(&mut asked).expect("the name");
            stream.write_all(&answer).expect("the answer");
            asked.get(..n).unwrap_or_default().to_vec()
        });
        (port, handle)
    }

    #[test]
    fn the_port_mapper_is_asked_by_name_and_its_port_is_used_r_sql_1() {
        let (mapper, asked) = fake_mapper(vec![b'+', 0, 0, 0x24, 0xFF]);
        let port = ask_port_mapper("127.0.0.1", mapper, Service::Database, true);
        assert_eq!(port.ok(), Some(Some(9471)));
        assert_eq!(asked.join().expect("the mapper"), b"as-database-s");
    }

    #[test]
    fn an_unknown_service_is_none_r_sql_1() {
        let (mapper, _asked) = fake_mapper(b"-".to_vec());
        let port = ask_port_mapper("127.0.0.1", mapper, Service::SignOn, false);
        assert_eq!(port.ok(), Some(None));
    }

    #[test]
    fn no_port_mapper_is_an_error_the_caller_can_fall_back_from_r_sql_1() {
        let unused = TcpListener::bind("127.0.0.1:0").expect("a local port");
        let port = unused.local_addr().expect("its address").port();
        drop(unused);
        assert!(ask_port_mapper("127.0.0.1", port, Service::SignOn, false).is_err());
    }

    /// A sign-on server on a local port that answers the exchange of attributes with PUB400's
    /// recorded reply and every sign-on with `code`; joined, the request ids it received.
    fn fake_sign_on_server(code: u32) -> (u16, thread::JoinHandle<Vec<u16>>) {
        use crate::hostserver::fixtures::PUB400_EXCHANGE_ATTRIBUTES;
        use crate::hostserver::{Param, ServerId};
        let listener = TcpListener::bind("127.0.0.1:0").expect("a local port");
        let port = listener.local_addr().expect("its address").port();
        let handle = thread::spawn(move || {
            let (stream, _) = listener.accept().expect("a client");
            let mut stream: Box<dyn Stream> = Box::new(stream);
            let mut ids = Vec::new();
            while let Ok(request) = receive(&mut stream) {
                ids.push(request.id);
                match request.id {
                    0x7003 => {
                        let reply = Record::decode(&PUB400_EXCHANGE_ATTRIBUTES).expect("fixture");
                        send(&mut stream, &reply).expect("the attributes");
                    }
                    0x7004 => {
                        let reply = Record {
                            server: ServerId::SignOn,
                            cs_instance: 0,
                            correlation: 0,
                            id: 0xF004,
                            template: code.to_be_bytes().to_vec(),
                            params: vec![Param::new(0x1114, 37u32.to_be_bytes())],
                        };
                        send(&mut stream, &reply).expect("the answer");
                    }
                    _ => break,
                }
            }
            ids
        });
        (port, handle)
    }

    fn local() -> SessionConfig {
        SessionConfig {
            host: "127.0.0.1".to_owned(),
            ..SessionConfig::default()
        }
    }

    #[test]
    fn a_sign_on_exchanges_attributes_signs_on_once_and_ends_r_sql_2() {
        let (port, ids) = fake_sign_on_server(0);
        let password = Password::new("Kodeord æøå".to_owned());
        let mut transcript = Transcript::new();
        let result = sign_on_at(
            &local(),
            port,
            "usr",
            &password,
            CodePage::Ccsid277,
            Some(&mut transcript),
        )
        .expect("signed on");
        assert_eq!(result.attributes.release_name(), "V7R5M0");
        assert_eq!(result.signed_on.server_ccsid, Some(37));
        assert_eq!(ids.join().expect("the server"), [0x7003, 0x7004, 0x7006]);
        let directions: Vec<Direction> = transcript.iter().map(|(d, _)| *d).collect();
        assert_eq!(
            directions,
            [
                Direction::Sent,
                Direction::Received,
                Direction::Sent,
                Direction::Received,
                Direction::Sent
            ]
        );
    }

    #[test]
    fn a_wrong_password_is_refused_once_and_never_retried_r_sql_2() {
        let (port, ids) = fake_sign_on_server(0x0003_000C);
        let password = Password::new("wrong".to_owned());
        let error = sign_on_at(&local(), port, "usr", &password, CodePage::Ccsid37, None)
            .expect_err("refused");
        assert!(
            error
                .to_string()
                .contains("One more wrong password disables"),
            "{error}"
        );
        let ids = ids.join().expect("the server");
        assert_eq!(
            ids.iter().filter(|&&id| id == 0x7004).count(),
            1,
            "{ids:04X?}"
        );
    }

    #[test]
    fn the_transcript_keeps_no_substitute_and_no_seeds_r_net_9() {
        let (port, _ids) = fake_sign_on_server(0);
        let password = Password::new("secret".to_owned());
        let mut transcript = Transcript::new();
        sign_on_at(
            &local(),
            port,
            "usr",
            &password,
            CodePage::Ccsid37,
            Some(&mut transcript),
        )
        .expect("signed on");
        for (_, bytes) in &transcript {
            let record = Record::decode(bytes).expect("a whole record");
            for code_point in [0x1103, 0x1105] {
                if let Some(data) = record.param(code_point) {
                    assert!(data.iter().all(|&b| b == 0), "{code_point:#06X} kept");
                }
            }
        }
        let sign_on = Record::decode(&transcript[2].1).expect("the sign-on");
        assert_eq!(
            sign_on.param(0x1105).map(<[u8]>::len),
            Some(20),
            "blanked, not dropped"
        );
    }

    #[test]
    fn a_user_name_that_cannot_sign_on_sends_nothing_r_sql_2() {
        let unused = TcpListener::bind("127.0.0.1:0").expect("a local port");
        let port = unused.local_addr().expect("its address").port();
        drop(unused);
        let password = Password::new("pw".to_owned());
        let error = sign_on_at(&local(), port, "a b", &password, CodePage::Ccsid37, None)
            .expect_err("refused");
        assert!(
            matches!(error, Error::SignOn(_)),
            "refused before connecting: {error}"
        );
    }
}
