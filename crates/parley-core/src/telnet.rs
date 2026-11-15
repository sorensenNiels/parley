//! Telnet for TN5250: option negotiation (RFC 854, RFC 1205), the terminal type, the
//! NEW-ENVIRON variables (RFC 1572, RFC 4777: device name and code page; never a user or
//! password, R-NET-7), and splitting the byte stream into 5250
//! records, each ended by IAC EOR.

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
const VALUE: u8 = 1;
const ESC: u8 = 2;
const USERVAR: u8 = 3;

/// Options Parley turns on for its own side when the host asks (DO): RFC 2877 section 2.
const LOCAL_OPTIONS: [u8; 4] = [BINARY, TERMINAL_TYPE, END_OF_RECORD, NEW_ENVIRON];
/// Options Parley accepts for the host's side (WILL).
const REMOTE_OPTIONS: [u8; 2] = [BINARY, END_OF_RECORD];

/// The longest 5250 record: its length field is 16 bits (RFC 1205 section 3, "5250 Data
/// Stream Header"). A host that sends more without IAC EOR is not speaking 5250.
const MAX_RECORD: usize = 65_535;
/// The longest sub-negotiation kept: the host's are a terminal-type or NEW-ENVIRON request
/// of a few dozen bytes.
const MAX_SUB: usize = 4096;

/// Whether bytes are being kept in a buffer, or dropped because it outgrew its limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Keep {
    Keeping,
    /// Dropped until the end of the record (IAC EOR) or sub-negotiation (IAC SE): the next
    /// one starts afresh.
    Dropping,
}

/// Negotiation and record framing for one connection.
#[derive(Debug)]
pub struct Telnet {
    terminal_type: String,
    environment: Vec<(String, String)>,
    state: State,
    local: [bool; 256],
    remote: [bool; 256],
    record: Vec<u8>,
    record_keep: Keep,
    sub: Vec<u8>,
    sub_keep: Keep,
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
    /// How many records were dropped for being longer than a 5250 record can be.
    pub dropped: usize,
}

impl Telnet {
    /// `environment` holds the user variables (`USERVAR`) sent when the host asks, such as
    /// `DEVNAME` and `CODEPAGE` (RFC 4777).
    pub fn new(terminal_type: &str, environment: Vec<(String, String)>) -> Self {
        Self {
            terminal_type: terminal_type.to_owned(),
            environment,
            state: State::Data,
            local: [false; 256],
            remote: [false; 256],
            record: Vec::new(),
            record_keep: Keep::Keeping,
            sub: Vec::new(),
            sub_keep: Keep::Keeping,
        }
    }

    pub fn receive(&mut self, bytes: &[u8], out: &mut Received) {
        for &b in bytes {
            self.state = match (self.state, b) {
                (State::Data, IAC) => State::Iac,
                (State::Data, _) | (State::Iac, IAC) => {
                    self.push_record(b);
                    State::Data
                }
                (State::Iac, EOR) => {
                    let record = std::mem::take(&mut self.record);
                    match std::mem::replace(&mut self.record_keep, Keep::Keeping) {
                        Keep::Keeping => out.records.push(record),
                        Keep::Dropping => out.dropped += 1,
                    }
                    State::Data
                }
                (State::Iac, WILL | WONT | DO | DONT) => State::Option(b),
                (State::Iac, SB) => {
                    self.sub.clear();
                    self.sub_keep = Keep::Keeping;
                    State::Sub
                }
                (State::Option(verb), option) => {
                    self.negotiate(verb, option, &mut out.reply);
                    State::Data
                }
                (State::Sub, IAC) => State::SubIac,
                (State::Sub, _) | (State::SubIac, IAC) => {
                    self.push_sub(b);
                    State::Sub
                }
                (State::SubIac, SE) => {
                    match self.sub_keep {
                        Keep::Keeping => self.subnegotiation(&mut out.reply),
                        Keep::Dropping => self.sub_keep = Keep::Keeping,
                    }
                    self.sub.clear();
                    State::Data
                }
                // NOP, GA and the other commands mean nothing to a 5250 session; anything else
                // after IAC inside a sub-negotiation is not valid and is dropped.
                (State::Iac | State::SubIac, _) => State::Data,
            };
        }
    }

    /// Adds a byte to the record, unless it has grown past [`MAX_RECORD`]: then the record is
    /// dropped up to its IAC EOR, so the next one is read from its start.
    fn push_record(&mut self, b: u8) {
        match self.record_keep {
            Keep::Keeping if self.record.len() < MAX_RECORD => self.record.push(b),
            Keep::Keeping => {
                self.record = Vec::new();
                self.record_keep = Keep::Dropping;
            }
            Keep::Dropping => {}
        }
    }

    /// Adds a byte to the sub-negotiation, unless it has grown past [`MAX_SUB`]: then it is
    /// dropped up to its IAC SE and not answered.
    fn push_sub(&mut self, b: u8) {
        match self.sub_keep {
            Keep::Keeping if self.sub.len() < MAX_SUB => self.sub.push(b),
            Keep::Keeping => {
                self.sub = Vec::new();
                self.sub_keep = Keep::Dropping;
            }
            Keep::Dropping => {}
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
            [NEW_ENVIRON, SEND, ..] => {
                reply.extend([IAC, SB, NEW_ENVIRON, IS]);
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

/// NEW-ENVIRON text: the codes VAR (0), VALUE, ESC and USERVAR are escaped with ESC, IAC is doubled.
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
        Telnet::new("IBM-3179-2", vec![("DEVNAME".into(), "PARLEY01".into())])
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
        0, // VAR
        IAC,
        SE,
    ];

    #[test]
    fn no_user_or_password_goes_to_the_host_even_when_it_offers_a_seed_r_net_7() {
        // RFC 4777 section 5's request for USER and a password substitute: only DEVNAME goes.
        let out = receive(&mut telnet(), SEND_WITH_SEED);
        let mut expected = vec![IAC, SB, NEW_ENVIRON, IS, USERVAR];
        expected.extend(b"DEVNAME");
        expected.push(VALUE);
        expected.extend(b"PARLEY01");
        expected.extend([IAC, SE]);
        assert_eq!(out.reply, expected);
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
    fn a_record_longer_than_65535_bytes_is_dropped_and_the_next_one_still_arrives_r_net_1() {
        let mut t = telnet();
        let mut out = receive(&mut t, &vec![7; MAX_RECORD]);
        assert_eq!(
            t.record.len(),
            MAX_RECORD,
            "a record of the longest length is kept"
        );
        out.records.extend(receive(&mut t, &[8; 1000]).records);
        assert!(t.record.capacity() < MAX_RECORD, "the buffer stops growing");
        let end = receive(&mut t, &[IAC, EOR, 1, 2, IAC, EOR]);
        assert_eq!(out.records, Vec::<Vec<u8>>::new());
        assert_eq!(end.dropped, 1);
        assert_eq!(
            end.records,
            vec![vec![1, 2]],
            "the next record starts afresh"
        );
    }

    #[test]
    fn a_record_of_exactly_65535_bytes_arrives_r_net_1() {
        let mut t = telnet();
        let mut bytes = vec![7; MAX_RECORD];
        bytes.extend([IAC, EOR]);
        let out = receive(&mut t, &bytes);
        assert_eq!(out.records.len(), 1);
        assert_eq!(out.dropped, 0);
    }

    #[test]
    fn an_overlong_sub_negotiation_is_dropped_unanswered_and_the_next_is_answered_r_net_1() {
        let mut t = telnet();
        let mut bytes = vec![IAC, SB, TERMINAL_TYPE, SEND];
        bytes.extend(vec![0; MAX_SUB * 4]);
        assert_eq!(receive(&mut t, &bytes).reply, Vec::<u8>::new());
        assert!(t.sub.capacity() <= MAX_SUB, "the buffer stops growing");
        assert_eq!(
            receive(&mut t, &[IAC, SE]).reply,
            Vec::<u8>::new(),
            "not answered"
        );
        let out = receive(&mut t, &[IAC, SB, TERMINAL_TYPE, SEND, IAC, SE]);
        assert!(out.reply.starts_with(&[IAC, SB, TERMINAL_TYPE, IS]));
    }

    #[test]
    fn dont_and_wont_turn_an_option_off_once_r_net_1() {
        let mut t = telnet();
        receive(&mut t, &[IAC, DO, END_OF_RECORD, IAC, WILL, BINARY]);
        let out = receive(&mut t, &[IAC, DONT, END_OF_RECORD, IAC, WONT, BINARY]);
        assert_eq!(out.reply, [IAC, WONT, END_OF_RECORD, IAC, DONT, BINARY]);
        let again = receive(&mut t, &[IAC, DONT, END_OF_RECORD, IAC, WONT, BINARY]);
        assert_eq!(again.reply, Vec::<u8>::new(), "already off: no reply");
    }

    #[test]
    fn dont_and_wont_for_options_never_on_get_no_reply_r_net_1() {
        let out = receive(&mut telnet(), &[IAC, DONT, 31, IAC, WONT, 31]);
        assert_eq!(out.reply, Vec::<u8>::new());
    }

    #[test]
    fn a_sub_negotiation_split_inside_iac_se_is_answered_once_whole_r_net_1() {
        let mut t = telnet();
        let first = receive(&mut t, &[IAC, SB, TERMINAL_TYPE, SEND, IAC]);
        assert_eq!(first.reply, Vec::<u8>::new());
        let second = receive(&mut t, &[SE]);
        assert!(second.reply.starts_with(&[IAC, SB, TERMINAL_TYPE, IS]));
    }

    #[test]
    fn a_doubled_iac_inside_a_sub_negotiation_is_data_r_net_1() {
        let mut t = telnet();
        receive(&mut t, &[IAC, SB, 99, IAC, IAC, 1]);
        assert_eq!(t.sub, [99, IAC, 1]);
    }

    #[test]
    fn telnet_commands_between_records_are_ignored_r_net_1() {
        let out = receive(&mut telnet(), &[1, IAC, NOP, 2, IAC, EOR]);
        assert_eq!(out.records, vec![vec![1, 2]]);
        assert_eq!(out.reply, Vec::<u8>::new());
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
