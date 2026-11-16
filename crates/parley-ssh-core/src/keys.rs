//! What a PC key sends to the host, as xterm sends it (R-SSH-7; xterm ctlseqs "PC-Style Function Keys"), and
//! how pasted text is sent (R-SSH-9).

use crate::term::{KeyMode, Modes};

/// A key the terminal turns into bytes. Text keys come as [`Key::Char`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Char(char),
    Enter,
    Backspace,
    Tab,
    Escape,
    Up,
    Down,
    Right,
    Left,
    Home,
    End,
    Insert,
    Delete,
    PageUp,
    PageDown,
    /// F1 to F12.
    F(u8),
}

/// The modifier keys held with a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Mods {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
}

impl Mods {
    /// xterm's modifier parameter: 1 plus Shift 1, Alt 2, Ctrl 4.
    fn parameter(self) -> u8 {
        1 + u8::from(self.shift) + 2 * u8::from(self.alt) + 4 * u8::from(self.ctrl)
    }
}

/// The bytes `key` with `mods` sends in the terminal's current `modes`; empty when it sends nothing.
pub fn encode(key: Key, mods: Mods, terminal: &Modes) -> Vec<u8> {
    let m = mods.parameter();
    let mut bytes = match key {
        Key::Char(c) => return char_bytes(c, mods),
        Key::Enter => b"\r".to_vec(),
        Key::Backspace => vec![if mods.ctrl { 0x08 } else { 0x7F }],
        Key::Tab if mods.shift => return b"\x1b[Z".to_vec(),
        Key::Tab => b"\t".to_vec(),
        Key::Escape => b"\x1b".to_vec(),
        Key::Up => return cursor(b'A', m, terminal.cursor_keys),
        Key::Down => return cursor(b'B', m, terminal.cursor_keys),
        Key::Right => return cursor(b'C', m, terminal.cursor_keys),
        Key::Left => return cursor(b'D', m, terminal.cursor_keys),
        Key::Home => return cursor(b'H', m, terminal.cursor_keys),
        Key::End => return cursor(b'F', m, terminal.cursor_keys),
        Key::Insert => return tilde(2, m),
        Key::Delete => return tilde(3, m),
        Key::PageUp => return tilde(5, m),
        Key::PageDown => return tilde(6, m),
        Key::F(n) => return function(n, m),
    };
    if mods.alt {
        bytes.insert(0, 0x1B);
    }
    bytes
}

/// A typed character: Ctrl+letter and the other Ctrl characters as C0 controls, Alt as an `ESC` first.
fn char_bytes(c: char, mods: Mods) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(5);
    if mods.alt {
        bytes.push(0x1B);
    }
    match (mods.ctrl, control(c)) {
        (true, Some(byte)) => bytes.push(byte),
        _ => bytes.extend_from_slice(c.encode_utf8(&mut [0; 4]).as_bytes()),
    }
    bytes
}

/// The C0 control Ctrl+`c` sends, as on a VT220 keyboard and in xterm.
fn control(c: char) -> Option<u8> {
    let byte = u8::try_from(c).ok()?;
    match byte {
        b'a'..=b'z' => Some(byte - b'a' + 1),
        b'A'..=b'Z' => Some(byte - b'A' + 1),
        b'@' | b' ' | b'2' => Some(0x00),
        b'[' | b'3' => Some(0x1B),
        b'\\' | b'4' => Some(0x1C),
        b']' | b'5' => Some(0x1D),
        b'^' | b'6' => Some(0x1E),
        b'_' | b'-' | b'7' => Some(0x1F),
        b'?' | b'8' => Some(0x7F),
        _ => None,
    }
}

/// Arrows, Home and End: `CSI x` or `SS3 x` in application mode; `CSI 1;m x` with modifiers.
fn cursor(final_byte: u8, m: u8, mode: KeyMode) -> Vec<u8> {
    if m > 1 {
        return format!("\x1b[1;{m}{}", char::from(final_byte)).into_bytes();
    }
    let introducer: &[u8] = match mode {
        KeyMode::Normal => b"\x1b[",
        KeyMode::Application => b"\x1bO",
    };
    [introducer, &[final_byte]].concat()
}

/// `CSI n ~`, with `;m` for modifiers.
fn tilde(n: u8, m: u8) -> Vec<u8> {
    if m > 1 {
        format!("\x1b[{n};{m}~")
    } else {
        format!("\x1b[{n}~")
    }
    .into_bytes()
}

/// F1–F4 as `SS3 P`…`S` (`CSI 1;m P` with modifiers); F5–F12 as `CSI 15~`…`CSI 24~`.
fn function(n: u8, m: u8) -> Vec<u8> {
    let code = match n {
        1..=4 => {
            let letter = char::from(b'P' + n - 1);
            return if m > 1 {
                format!("\x1b[1;{m}{letter}")
            } else {
                format!("\x1bO{letter}")
            }
            .into_bytes();
        }
        5 => 15,
        6..=10 => n + 11,
        11 | 12 => n + 12,
        _ => return Vec::new(),
    };
    tilde(code, m)
}

/// Pasted text as it goes to the host (R-SSH-9): line ends as `CR`; in bracketed paste mode wrapped in
/// `CSI 200~` and `CSI 201~`, with any `ESC` in the text dropped so it cannot end the paste early.
pub fn paste(text: &str, modes: &Modes) -> Vec<u8> {
    let text = text.replace("\r\n", "\r").replace('\n', "\r");
    if !modes.bracketed_paste {
        return text.into_bytes();
    }
    let mut bytes = b"\x1b[200~".to_vec();
    bytes.extend(text.bytes().filter(|&b| b != 0x1B));
    bytes.extend_from_slice(b"\x1b[201~");
    bytes
}

/// Does pasting `text` need a question first: more than one line, outside bracketed paste (R-SSH-9)?
pub fn paste_needs_confirmation(text: &str, modes: &Modes) -> bool {
    !modes.bracketed_paste && text.trim_end_matches(['\r', '\n']).contains(['\r', '\n'])
}

/// Focus in (`CSI I`) or out (`CSI O`) when the host asked for focus events (mode 1004).
pub fn focus(gained: bool, modes: &Modes) -> Vec<u8> {
    match (modes.focus_events, gained) {
        (false, _) => Vec::new(),
        (true, true) => b"\x1b[I".to_vec(),
        (true, false) => b"\x1b[O".to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(key: Key) -> Vec<u8> {
        encode(key, Mods::default(), &Modes::default())
    }

    fn with(key: Key, mods: Mods) -> Vec<u8> {
        encode(key, mods, &Modes::default())
    }

    const CTRL: Mods = Mods {
        shift: false,
        ctrl: true,
        alt: false,
    };
    const ALT: Mods = Mods {
        shift: false,
        ctrl: false,
        alt: true,
    };
    const SHIFT: Mods = Mods {
        shift: true,
        ctrl: false,
        alt: false,
    };

    #[test]
    fn text_is_sent_as_utf8_r_ssh_7() {
        assert_eq!(plain(Key::Char('a')), b"a");
        assert_eq!(plain(Key::Char('æ')), "æ".as_bytes());
        assert_eq!(plain(Key::Char('€')), "€".as_bytes());
    }

    #[test]
    fn ctrl_letters_are_c0_controls_r_ssh_7() {
        assert_eq!(with(Key::Char('c'), CTRL), [0x03]);
        assert_eq!(with(Key::Char('A'), CTRL), [0x01]);
        assert_eq!(with(Key::Char(' '), CTRL), [0x00]);
        assert_eq!(with(Key::Char('['), CTRL), [0x1B]);
        assert_eq!(with(Key::Char('_'), CTRL), [0x1F]);
        // Ctrl with a character that has no control sends the character.
        assert_eq!(with(Key::Char('ø'), CTRL), "ø".as_bytes());
    }

    #[test]
    fn alt_puts_an_escape_first_r_ssh_7() {
        assert_eq!(with(Key::Char('x'), ALT), b"\x1bx");
        assert_eq!(with(Key::Backspace, ALT), b"\x1b\x7f");
        assert_eq!(with(Key::Enter, ALT), b"\x1b\r");
    }

    #[test]
    fn editing_keys_send_what_xterm_sends_r_ssh_7() {
        assert_eq!(plain(Key::Enter), b"\r");
        assert_eq!(plain(Key::Backspace), [0x7F]);
        assert_eq!(with(Key::Backspace, CTRL), [0x08]);
        assert_eq!(plain(Key::Tab), b"\t");
        assert_eq!(with(Key::Tab, SHIFT), b"\x1b[Z");
        assert_eq!(plain(Key::Escape), b"\x1b");
        assert_eq!(plain(Key::Insert), b"\x1b[2~");
        assert_eq!(plain(Key::Delete), b"\x1b[3~");
        assert_eq!(plain(Key::PageUp), b"\x1b[5~");
        assert_eq!(plain(Key::PageDown), b"\x1b[6~");
        assert_eq!(with(Key::Delete, CTRL), b"\x1b[3;5~");
    }

    #[test]
    fn arrows_follow_the_cursor_key_mode_r_ssh_7() {
        assert_eq!(plain(Key::Up), b"\x1b[A");
        assert_eq!(plain(Key::Home), b"\x1b[H");
        let application = Modes {
            cursor_keys: KeyMode::Application,
            ..Modes::default()
        };
        assert_eq!(encode(Key::Up, Mods::default(), &application), b"\x1bOA");
        assert_eq!(encode(Key::End, Mods::default(), &application), b"\x1bOF");
        // With a modifier the CSI form is used in either mode.
        assert_eq!(encode(Key::Left, CTRL, &application), b"\x1b[1;5D");
        assert_eq!(with(Key::Right, SHIFT), b"\x1b[1;2C");
    }

    #[test]
    fn function_keys_send_what_xterm_sends_r_ssh_7() {
        let sent: Vec<Vec<u8>> = (1..=12).map(|n| plain(Key::F(n))).collect();
        let expected: [&[u8]; 12] = [
            b"\x1bOP",
            b"\x1bOQ",
            b"\x1bOR",
            b"\x1bOS",
            b"\x1b[15~",
            b"\x1b[17~",
            b"\x1b[18~",
            b"\x1b[19~",
            b"\x1b[20~",
            b"\x1b[21~",
            b"\x1b[23~",
            b"\x1b[24~",
        ];
        assert_eq!(sent, expected.map(<[u8]>::to_vec));
        assert_eq!(with(Key::F(1), SHIFT), b"\x1b[1;2P");
        assert_eq!(with(Key::F(5), CTRL), b"\x1b[15;5~");
        assert_eq!(plain(Key::F(13)), b"");
    }

    #[test]
    fn a_paste_sends_line_ends_as_cr_r_ssh_9() {
        assert_eq!(paste("ls\nwho\r\n", &Modes::default()), b"ls\rwho\r");
    }

    #[test]
    fn a_bracketed_paste_is_wrapped_and_cannot_end_itself_r_ssh_9() {
        let modes = Modes {
            bracketed_paste: true,
            ..Modes::default()
        };
        assert_eq!(
            paste("a\x1b[201~rm -rf\nb", &modes),
            b"\x1b[200~a[201~rm -rf\rb\x1b[201~"
        );
    }

    #[test]
    fn only_several_lines_outside_bracketed_paste_need_a_question_r_ssh_9() {
        let plain = Modes::default();
        assert!(!paste_needs_confirmation("ls -l", &plain));
        assert!(!paste_needs_confirmation("ls -l\n", &plain));
        assert!(paste_needs_confirmation("ls\nrm x", &plain));
        let bracketed = Modes {
            bracketed_paste: true,
            ..Modes::default()
        };
        assert!(!paste_needs_confirmation("ls\nrm x", &bracketed));
    }

    #[test]
    fn focus_events_are_sent_only_when_asked_for() {
        assert_eq!(focus(true, &Modes::default()), b"");
        let modes = Modes {
            focus_events: true,
            ..Modes::default()
        };
        assert_eq!(focus(true, &modes), b"\x1b[I");
        assert_eq!(focus(false, &modes), b"\x1b[O");
    }
}
