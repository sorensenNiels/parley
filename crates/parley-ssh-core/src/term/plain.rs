//! Session logs (R-SSH-25): the host's output kept as it came ("raw"), or as plain text without escape sequences,
//! carriage returns and other controls. The filter keeps its state between calls, so a sequence cut in two by the
//! network is still left out.

use super::Terminal;

/// Where the filter is in the host's output.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum State {
    #[default]
    Text,
    /// After ESC.
    Escape,
    /// In a CSI sequence, up to its final byte.
    Csi,
    /// In an OSC, DCS, SOS, PM or APC string, up to BEL or ST.
    String,
    /// After ESC inside a string: `\` ends it.
    StringEscape,
}

/// Turns the host's output into plain text, line breaks as `\n`.
#[derive(Debug, Default)]
pub struct PlainText {
    state: State,
    /// A UTF-8 character cut in two at the end of the last call.
    pending: Vec<u8>,
}

impl PlainText {
    #[expect(
        clippy::match_same_arms,
        reason = "a state table reads best with one arm per state and byte, as ECMA-48 describes them"
    )]
    pub fn push(&mut self, bytes: &[u8]) -> String {
        let mut kept = std::mem::take(&mut self.pending);
        for &b in bytes {
            self.state = match (self.state, b) {
                (State::Text, 0x1b) => State::Escape,
                (State::Text, b'\n' | b'\t') => {
                    kept.push(b);
                    State::Text
                }
                (State::Text, b) if b < 0x20 || b == 0x7f => State::Text,
                (State::Text, b) => {
                    kept.push(b);
                    State::Text
                }
                (State::Escape, b'[') => State::Csi,
                (State::Escape, b']' | b'P' | b'X' | b'^' | b'_') => State::String,
                // ESC ( 0 and its kind take one more byte, which is not text either.
                (State::Escape, b'(' | b')' | b'*' | b'+' | b'#' | b' ' | b'%') => State::Csi,
                (State::Escape, _) => State::Text,
                (State::Csi, 0x40..=0x7e) => State::Text,
                (State::Csi, _) => State::Csi,
                (State::String, 0x07) => State::Text,
                (State::String, 0x1b) => State::StringEscape,
                (State::String, _) => State::String,
                (State::StringEscape, b'\\') => State::Text,
                (State::StringEscape, _) => State::String,
            };
        }
        // Hold back an incomplete character at the end.
        let cut = incomplete_tail(&kept);
        self.pending = kept.split_off(kept.len() - cut);
        String::from_utf8_lossy(&kept).into_owned()
    }
}

/// How many bytes at the end begin a UTF-8 character that is not complete yet.
fn incomplete_tail(bytes: &[u8]) -> usize {
    for back in 1..=bytes.len().min(3) {
        let b = bytes[bytes.len() - back];
        if b & 0b1100_0000 == 0b1000_0000 {
            continue;
        }
        let needed = match b {
            0xc0..=0xdf => 2,
            0xe0..=0xef => 3,
            0xf0..=0xf7 => 4,
            _ => 1,
        };
        return if needed > back { back } else { 0 };
    }
    0
}

impl Terminal {
    /// Keeps a copy of what the host sends from now on, for a session log (R-SSH-25); off again with `false`.
    pub fn record(&mut self, on: bool) {
        self.recording = on.then(Vec::new);
    }

    /// What the host sent since the last call, while recording.
    pub fn take_recorded(&mut self) -> Vec<u8> {
        self.recording
            .as_mut()
            .map(std::mem::take)
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_sequences_and_carriage_returns_are_left_out_r_ssh_25() {
        let mut plain = PlainText::default();
        let text = plain.push(b"\x1b[1;31mred\x1b[0m\r\n\x1b]0;title\x07ok\x1b]8;;http://x\x1b\\link\x1b(0q\x1b(B\tend\x07\n");
        assert_eq!(text, "red\noklink\tend\n");
    }

    #[test]
    fn a_sequence_or_character_cut_in_two_is_still_right_r_ssh_25() {
        let mut plain = PlainText::default();
        // ESC [ 3 1 m, then æ ø å of two bytes each: cut after ESC [ and inside ø.
        let all = "\x1b[31mæøå".as_bytes();
        assert_eq!(plain.push(&all[..2]), "");
        assert_eq!(plain.push(&all[2..8]), "æ");
        assert_eq!(plain.push(&all[8..]), "øå");
    }

    #[test]
    fn the_terminal_records_only_while_asked_r_ssh_25() {
        let mut t = Terminal::new(10, 2);
        t.feed(b"before");
        t.record(true);
        t.feed(b"\x1b[1mafter");
        assert_eq!(t.take_recorded(), b"\x1b[1mafter");
        assert_eq!(t.take_recorded(), b"");
        t.record(false);
        t.feed(b"more");
        assert_eq!(t.take_recorded(), b"");
    }
}
