//! Telnet for TN5250: option negotiation (RFC 854, RFC 1205), the terminal type, the
//! NEW-ENVIRON variables (RFC 1572, RFC 4777), and splitting the byte stream into 5250
//! records, each ended by IAC EOR.

use crate::signon::Credentials;

const IAC: u8 = 255;
const DONT: u8 = 254;
const DO: u8 = 253;
const WONT: u8 = 252;
const WILL: u8 = 251;
const SB: u8 = 250;
const SE: u8 = 240;
const EOR: u8 = 239;
const NOP: u8 = 241;

const BINARY: u8 = 0;
const TIMING_MARK: u8 = 6;
const TERMINAL_TYPE: u8 = 24;
const END_OF_RECORD: u8 = 25;
const NEW_ENVIRON: u8 = 39;

// Sub-negotiation codes (RFC 1091, RFC 1572).
const IS: u8 = 0;
const SEND: u8 = 1;
const VAR: u8 = 0;
const VALUE: u8 = 1;
const ESC: u8 = 2;
const USERVAR: u8 = 3;

/// Options Parley turns on for its own side when the host asks (DO): RFC 2877 section 2.
const LOCAL_OPTIONS: [u8; 4] = [BINARY, TERMINAL_TYPE, END_OF_RECORD, NEW_ENVIRON];
/// Options Parley accepts for the host's side (WILL).
const REMOTE_OPTIONS: [u8; 2] = [BINARY, END_OF_RECORD];

/// Negotiation and record framing for one connection.
#[derive(Debug)]
pub struct Telnet {
    terminal_type: String,
    environment: Vec<(String, String)>,
    /// For bypassing the sign-on screen, when the host sends its seed (RFC 4777 section 5).
    credentials: Option<Credentials>,
    state: State,
    local: [bool; 256],
    remote: [bool; 256],
    record: Vec<u8>,
    sub: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Data,
    Iac,
    /// After IAC WILL, WONT, DO or DONT: the option byte comes next.
    Option(u8),
    Sub,
    SubIac,
}

/// What a call to [`Telnet::receive`] produced.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Received {
    /// Complete 5250 records, without the IAC EOR that ended them.
    pub records: Vec<Vec<u8>>,
    /// Negotiation replies to send to the host.
    pub reply: Vec<u8>,
}

impl Telnet {
    /// `environment` holds the user variables (`USERVAR`) sent when the host asks, such as
    /// `DEVNAME` and `CODEPAGE` (RFC 4777). With `credentials`, the user and a password
    /// substitute go with them when the host offers a seed.
    pub fn new(
        terminal_type: &str,
        environment: Vec<(String, String)>,
        credentials: Option<Credentials>,
    ) -> Self {
        Self {
            terminal_type: terminal_type.to_owned(),
            environment,
            credentials,
            state: State::Data,
            local: [false; 256],
            remote: [false; 256],
            record: Vec::new(),
            sub: Vec::new(),
        }
    }

    pub fn receive(&mut self, bytes: &[u8], out: &mut Received) {
        for &b in bytes {
            self.state = match (self.state, b) {
                (State::Data, IAC) => State::Iac,
                (State::Data, _) => {
                    self.record.push(b);
                    State::Data
                }
                (State::Iac, IAC) => {
                    self.record.push(IAC);
                    State::Data
                }
                (State::Iac, EOR) => {
                    out.records.push(std::mem::take(&mut self.record));
                    State::Data
                }
                (State::Iac, WILL | WONT | DO | DONT) => State::Option(b),
                (State::Iac, SB) => {
                    self.sub.clear();
                    State::Sub
                }
                (State::Option(verb), option) => {
                    self.negotiate(verb, option, &mut out.reply);
                    State::Data
                }
                (State::Sub, IAC) => State::SubIac,
                (State::Sub, _) => {
                    self.sub.push(b);
                    State::Sub
                }
                (State::SubIac, IAC) => {
                    self.sub.push(IAC);
                    State::Sub
                }
                (State::SubIac, SE) => {
                    self.subnegotiation(&mut out.reply);
                    State::Data
                }
                // NOP, GA and the other commands mean nothing to a 5250 session; anything else
                // after IAC inside a sub-negotiation is not valid and is dropped.
                (State::Iac | State::SubIac, _) => State::Data,
            };
        }
    }

    /// Telnet No Operation (RFC 854), sent now and then to keep idle connections open
    /// through firewalls that drop them (R-NET-5).
    pub const KEEP_ALIVE: [u8; 2] = [IAC, NOP];

    /// A 5250 record as it goes on the wire: IAC bytes doubled, IAC EOR at the end.
    pub fn frame(record: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(record.len() + 4);
        for &b in record {
            out.push(b);
            if b == IAC {
                out.push(IAC);
            }
        }
        out.extend([IAC, EOR]);
        out
    }

    /// Replies only when an option changes state, so the two sides cannot loop (RFC 854).
    fn negotiate(&mut self, verb: u8, option: u8, reply: &mut Vec<u8>) {
        let i = usize::from(option);
        match verb {
            // The IBM i Telnet server checks the client is alive with timing marks; each one
            // needs an answer, so it has no state.
            DO if option == TIMING_MARK => reply.extend([IAC, WILL, TIMING_MARK]),
            DO if !LOCAL_OPTIONS.contains(&option) => reply.extend([IAC, WONT, option]),
            DO if !self.local[i] => {
                self.local[i] = true;
                reply.extend([IAC, WILL, option]);
            }
            DONT if self.local[i] => {
                self.local[i] = false;
                reply.extend([IAC, WONT, option]);
            }
            WILL if !REMOTE_OPTIONS.contains(&option) => reply.extend([IAC, DONT, option]),
            WILL if !self.remote[i] => {
                self.remote[i] = true;
                reply.extend([IAC, DO, option]);
            }
            WONT if self.remote[i] => {
                self.remote[i] = false;
                reply.extend([IAC, DONT, option]);
            }
            _ => {}
        }
    }

    fn subnegotiation(&self, reply: &mut Vec<u8>) {
        match self.sub.as_slice() {
            [TERMINAL_TYPE, SEND, ..] => {
                reply.extend([IAC, SB, TERMINAL_TYPE, IS]);
                reply.extend(self.terminal_type.bytes());
                reply.extend([IAC, SE]);
            }
            // The host may name the variables it wants; sending all of ours is allowed (RFC 1572).
            [NEW_ENVIRON, SEND, request @ ..] => {
                reply.extend([IAC, SB, NEW_ENVIRON, IS]);
                self.sign_on_variables(request, reply);
                for (name, value) in &self.environment {
                    reply.push(USERVAR);
                    escape_into(name.as_bytes(), reply);
                    reply.push(VALUE);
                    escape_into(value.as_bytes(), reply);
                }
                reply.extend([IAC, SE]);
            }
            _ => {}
        }
    }
}

impl Telnet {
    /// RFC 4777 section 5: the user, Parley's seed and the password substitute, when the host
    /// sent its own seed. Without one, nothing: the password is never sent in clear text.
    fn sign_on_variables(&self, request: &[u8], reply: &mut Vec<u8>) {
        let Some(credentials) = &self.credentials else {
            return;
        };
        let Some(substitute) = server_seed(request).and_then(|seed| credentials.substitute(seed))
        else {
            return;
        };
        let mut variable = |kind: u8, name: &[u8], value: &[u8]| {
            reply.push(kind);
            escape_into(name, reply);
            reply.push(VALUE);
            escape_into(value, reply);
        };
        variable(VAR, b"USER", credentials.user_name().as_bytes());
        variable(USERVAR, b"IBMRSEED", &credentials.client_seed);
        variable(USERVAR, b"IBMSUBSPW", &substitute);
    }
}

/// The 8-byte seed after `IBMRSEED` in the host's NEW-ENVIRON SEND (RFC 4777 section 5), its
/// escapes removed; `None` if there is none or it is short.
fn server_seed(request: &[u8]) -> Option<[u8; 8]> {
    const NAME: &[u8] = b"IBMRSEED";
    let at = request.windows(NAME.len()).position(|w| w == NAME)?;
    let mut bytes = request.get(at + NAME.len()..)?.iter();
    let mut seed = [0; 8];
    for byte in &mut seed {
        let mut b = *bytes.next()?;
        if b == ESC {
            b = *bytes.next()?;
        }
        *byte = b;
    }
    Some(seed)
}

/// NEW-ENVIRON text: the codes VAR, VALUE, ESC and USERVAR are escaped with ESC, IAC is doubled.
fn escape_into(text: &[u8], out: &mut Vec<u8>) {
    for &b in text {
        match b {
            0..=3 => out.extend([ESC, b]),
            IAC => out.extend([IAC, IAC]),
            _ => out.push(b),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn telnet() -> Telnet {
        Telnet::new(
            "IBM-3179-2",
            vec![("DEVNAME".into(), "PARLEY01".into())],
            None,
        )
    }

    fn signing_on() -> Telnet {
        use crate::signon::{Password, PasswordLevel};
        let credentials = Credentials {
            user: "dummyusr".into(),
            password: Password::new("DUMMYPW".into()),
            level: PasswordLevel::Des,
            code_page: crate::CodePage::Ccsid37,
            client_seed: *b"NAB3NAB3",
        };
        Telnet::new("IBM-3179-2", Vec::new(), Some(credentials))
    }

    /// The host's request from RFC 4777 section 5, with its seed X'7D3E488F18080404'.
    const SEND_WITH_SEED: &[u8] = &[
        IAC,
        SB,
        NEW_ENVIRON,
        SEND,
        USERVAR,
        b'I',
        b'B',
        b'M',
        b'R',
        b'S',
        b'E',
        b'E',
        b'D',
        0x7D,
        0x3E,
        0x48,
        0x8F,
        0x18,
        0x08,
        0x04,
        0x04,
        USERVAR,
        b'I',
        b'B',
        b'M',
        b'S',
        b'U',
        b'B',
        b'S',
        b'P',
        b'W',
        USERVAR,
        VAR,
        IAC,
        SE,
    ];

    #[test]
    fn the_sign_on_reply_matches_the_rfc_4777_example() {
        // R-NET-4, RFC 4777 section 5: the client's IS, byte for byte.
        let out = receive(&mut signing_on(), SEND_WITH_SEED);
        let mut expected = vec![IAC, SB, NEW_ENVIRON, IS, VAR];
        expected.extend(b"USER");
        expected.push(VALUE);
        expected.extend(b"DUMMYUSR");
        expected.push(USERVAR);
        expected.extend(b"IBMRSEED");
        expected.push(VALUE);
        expected.extend(b"NAB3NAB3");
        expected.push(USERVAR);
        expected.extend(b"IBMSUBSPW");
        expected.push(VALUE);
        expected.extend([0xDF, 0xB0, 0x40, 0x2F, 0x22, 0xAB, 0xA3, 0xBA]);
        expected.extend([IAC, SE]);
        assert_eq!(out.reply, expected);
    }

    #[test]
    fn without_a_host_seed_no_credentials_are_sent() {
        // R-NET-4: never the password in clear text.
        let out = receive(&mut signing_on(), &[IAC, SB, NEW_ENVIRON, SEND, IAC, SE]);
        assert_eq!(out.reply, [IAC, SB, NEW_ENVIRON, IS, IAC, SE]);
    }

    #[test]
    fn a_truncated_or_escaped_seed_is_read_safely() {
        assert_eq!(server_seed(b"IBMRSEED\x01\x02"), None);
        assert_eq!(server_seed(b"IBMRSEED"), None);
        assert_eq!(server_seed(&[]), None);
        let mut escaped = b"IBMRSEED".to_vec();
        escaped.extend([ESC, VALUE, ESC, ESC, 4, 5, 6, 7, 8, 9]);
        assert_eq!(server_seed(&escaped), Some([1, 2, 4, 5, 6, 7, 8, 9]));
    }

    fn receive(t: &mut Telnet, bytes: &[u8]) -> Received {
        let mut out = Received::default();
        t.receive(bytes, &mut out);
        out
    }

    #[test]
    fn the_tn5250_options_are_accepted_once() {
        // R-NET-1
        let mut t = telnet();
        let out = receive(
            &mut t,
            &[
                IAC,
                DO,
                TERMINAL_TYPE,
                IAC,
                DO,
                END_OF_RECORD,
                IAC,
                WILL,
                BINARY,
            ],
        );
        assert_eq!(
            out.reply,
            [
                IAC,
                WILL,
                TERMINAL_TYPE,
                IAC,
                WILL,
                END_OF_RECORD,
                IAC,
                DO,
                BINARY
            ]
        );
        let again = receive(&mut t, &[IAC, DO, TERMINAL_TYPE, IAC, WILL, BINARY]);
        assert_eq!(again.reply, Vec::<u8>::new());
    }

    #[test]
    fn unknown_options_are_refused() {
        let mut t = telnet();
        let out = receive(&mut t, &[IAC, DO, 31, IAC, WILL, 1]);
        assert_eq!(out.reply, [IAC, WONT, 31, IAC, DONT, 1]);
    }

    #[test]
    fn every_timing_mark_is_answered() {
        let mut t = telnet();
        let out = receive(&mut t, &[IAC, DO, TIMING_MARK, IAC, DO, TIMING_MARK]);
        assert_eq!(out.reply, [IAC, WILL, TIMING_MARK, IAC, WILL, TIMING_MARK]);
    }

    #[test]
    fn the_terminal_type_is_sent_when_asked() {
        let mut t = telnet();
        let out = receive(&mut t, &[IAC, SB, TERMINAL_TYPE, SEND, IAC, SE]);
        let mut expected = vec![IAC, SB, TERMINAL_TYPE, IS];
        expected.extend(b"IBM-3179-2");
        expected.extend([IAC, SE]);
        assert_eq!(out.reply, expected);
    }

    #[test]
    fn user_variables_are_sent_when_the_host_asks() {
        // R-NET-3
        let mut t = telnet();
        let out = receive(&mut t, &[IAC, SB, NEW_ENVIRON, SEND, 0, USERVAR, IAC, SE]);
        let mut expected = vec![IAC, SB, NEW_ENVIRON, IS, USERVAR];
        expected.extend(b"DEVNAME");
        expected.push(VALUE);
        expected.extend(b"PARLEY01");
        expected.extend([IAC, SE]);
        assert_eq!(out.reply, expected);
    }

    #[test]
    fn records_end_at_iac_eor_and_doubled_iac_is_data() {
        let mut t = telnet();
        let out = receive(&mut t, &[1, 2, IAC, IAC, 3, IAC, EOR, 4]);
        assert_eq!(out.records, vec![vec![1, 2, IAC, 3]]);
        let out = receive(&mut t, &[5, IAC, EOR]);
        assert_eq!(out.records, vec![vec![4, 5]]);
    }

    #[test]
    fn a_record_split_inside_an_iac_pair_is_put_back_together() {
        let mut t = telnet();
        assert_eq!(receive(&mut t, &[7, IAC]).records, Vec::<Vec<u8>>::new());
        assert_eq!(receive(&mut t, &[EOR]).records, vec![vec![7]]);
    }

    #[test]
    fn framing_doubles_iac_and_appends_eor() {
        assert_eq!(Telnet::frame(&[1, IAC, 2]), [1, IAC, IAC, 2, IAC, EOR]);
    }

    #[test]
    fn variable_values_with_control_codes_are_escaped() {
        let mut out = Vec::new();
        escape_into(&[b'A', 1, IAC], &mut out);
        assert_eq!(out, [b'A', ESC, 1, IAC, IAC]);
    }
}
