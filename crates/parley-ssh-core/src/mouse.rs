//! What a mouse event sends to the host when it asked for them (R-SSH-18; xterm ctlseqs "Mouse Tracking"): which
//! events each tracking mode reports, and the default, UTF-8 (1005) and SGR (1006) encodings.

use crate::keys::Mods;
use crate::term::{Modes, MouseEncoding, MouseTracking};

/// A mouse button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    Left,
    Middle,
    Right,
}

impl Button {
    fn code(self) -> u32 {
        match self {
            Self::Left => 0,
            Self::Middle => 1,
            Self::Right => 2,
        }
    }
}

/// Something the mouse did over the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    Press(Button),
    Release(Button),
    /// Moved to another cell with the button held.
    Drag(Button),
    /// Moved to another cell with no button held.
    Move,
    WheelUp,
    WheelDown,
}

/// The bytes `event` at `row` and `column` (from 0) with the modifier keys `held` sends in the terminal's `modes`; empty when the
/// host did not ask for it, or the place cannot be written in the encoding in use.
pub fn encode(event: Event, row: usize, column: usize, held: Mods, modes: &Modes) -> Vec<u8> {
    let tracking = modes.mouse;
    let wanted = match (tracking, event) {
        (MouseTracking::Off, _) => false,
        (MouseTracking::X10, event) => matches!(event, Event::Press(_)),
        (MouseTracking::Normal, event) => !matches!(event, Event::Drag(_) | Event::Move),
        (MouseTracking::ButtonEvent, event) => event != Event::Move,
        (MouseTracking::AnyEvent, _) => true,
    };
    if !wanted {
        return Vec::new();
    }
    let sgr = modes.mouse_encoding == MouseEncoding::Sgr;
    let mut code = match event {
        Event::Press(button) => button.code(),
        // Only SGR says which button was released.
        Event::Release(button) if sgr => button.code(),
        Event::Release(_) => 3,
        Event::Drag(button) => 32 + button.code(),
        Event::Move => 32 + 3,
        Event::WheelUp => 64,
        Event::WheelDown => 65,
    };
    if tracking != MouseTracking::X10 {
        code += 4 * u32::from(held.shift) + 8 * u32::from(held.alt) + 16 * u32::from(held.ctrl);
    }
    let (Ok(x), Ok(y)) = (u32::try_from(column + 1), u32::try_from(row + 1)) else {
        return Vec::new();
    };
    match modes.mouse_encoding {
        MouseEncoding::Sgr => {
            let end = if matches!(event, Event::Release(_)) {
                'm'
            } else {
                'M'
            };
            format!("\x1b[<{code};{x};{y}{end}").into_bytes()
        }
        MouseEncoding::Default => {
            let mut bytes = b"\x1b[M".to_vec();
            for value in [code, x, y] {
                match u8::try_from(value + 32) {
                    Ok(byte) => bytes.push(byte),
                    Err(_) => return Vec::new(),
                }
            }
            bytes
        }
        MouseEncoding::Utf8 => {
            let mut bytes = b"\x1b[M".to_vec();
            for value in [code, x, y] {
                // Two UTF-8 bytes at most, as xterm: up to U+07FF.
                match char::from_u32(value + 32).filter(|c| u32::from(*c) < 0x800) {
                    Some(c) => bytes.extend_from_slice(c.encode_utf8(&mut [0; 4]).as_bytes()),
                    None => return Vec::new(),
                }
            }
            bytes
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::term::Terminal;

    fn modes(set: &[u8]) -> Modes {
        let mut t = Terminal::new(80, 24);
        t.feed(set);
        *t.modes()
    }

    const NOTHING: Vec<u8> = Vec::new();

    fn none() -> Mods {
        Mods::default()
    }

    #[test]
    fn nothing_is_sent_until_the_host_asks_r_ssh_18() {
        let m = Modes::default();
        assert_eq!(
            encode(Event::Press(Button::Left), 0, 0, none(), &m),
            NOTHING
        );
    }

    #[test]
    fn mode_1000_sends_presses_releases_and_the_wheel_in_the_default_encoding_r_ssh_18() {
        let m = modes(b"\x1b[?1000h");
        assert_eq!(
            encode(Event::Press(Button::Left), 1, 4, none(), &m),
            b"\x1b[M\x20\x25\x22"
        );
        assert_eq!(
            encode(Event::Release(Button::Left), 1, 4, none(), &m),
            b"\x1b[M\x23\x25\x22",
            "a release is button 3"
        );
        assert_eq!(
            encode(Event::WheelUp, 0, 0, none(), &m),
            b"\x1b[M\x60\x21\x21"
        );
        assert_eq!(encode(Event::Drag(Button::Left), 1, 5, none(), &m), NOTHING);
        assert_eq!(encode(Event::Move, 1, 5, none(), &m), NOTHING);
    }

    #[test]
    fn sgr_names_the_button_released_and_has_no_limit_r_ssh_18() {
        let m = modes(b"\x1b[?1002h\x1b[?1006h");
        assert_eq!(
            encode(Event::Press(Button::Right), 0, 300, none(), &m),
            b"\x1b[<2;301;1M"
        );
        assert_eq!(
            encode(Event::Release(Button::Right), 0, 300, none(), &m),
            b"\x1b[<2;301;1m"
        );
        assert_eq!(
            encode(Event::Drag(Button::Left), 2, 3, none(), &m),
            b"\x1b[<32;4;3M",
            "1002 reports a drag"
        );
        assert_eq!(encode(Event::Move, 2, 3, none(), &m), NOTHING);
        let ctrl_shift = Mods {
            shift: true,
            ctrl: true,
            alt: false,
        };
        assert_eq!(
            encode(Event::WheelDown, 0, 0, ctrl_shift, &m),
            b"\x1b[<85;1;1M",
            "65 + Shift 4 + Ctrl 16"
        );
    }

    #[test]
    fn mode_1003_reports_moves_and_x10_only_presses_without_modifiers_r_ssh_18() {
        let any = modes(b"\x1b[?1003h\x1b[?1006h");
        assert_eq!(encode(Event::Move, 0, 0, none(), &any), b"\x1b[<35;1;1M");
        let x10 = modes(b"\x1b[?9h");
        let alt = Mods {
            alt: true,
            ..Mods::default()
        };
        assert_eq!(
            encode(Event::Press(Button::Middle), 0, 0, alt, &x10),
            b"\x1b[M\x21\x21\x21"
        );
        assert_eq!(
            encode(Event::Release(Button::Middle), 0, 0, none(), &x10),
            NOTHING
        );
        assert_eq!(encode(Event::WheelUp, 0, 0, none(), &x10), NOTHING);
    }

    #[test]
    fn places_past_what_the_encoding_holds_send_nothing_r_ssh_18() {
        let default = modes(b"\x1b[?1000h");
        assert_eq!(
            encode(Event::Press(Button::Left), 0, 222, none(), &default),
            b"\x1b[M\x20\xff\x21",
            "column 223 is the last"
        );
        assert_eq!(
            encode(Event::Press(Button::Left), 0, 223, none(), &default),
            NOTHING
        );
        let utf8 = modes(b"\x1b[?1000h\x1b[?1005h");
        assert_eq!(
            encode(Event::Press(Button::Left), 0, 300, none(), &utf8),
            "\x1b[M\u{20}\u{14d}\u{21}".as_bytes(),
            "301 + 32 as a UTF-8 character"
        );
        assert_eq!(
            encode(Event::Press(Button::Left), 0, 2015, none(), &utf8),
            NOTHING
        );
    }

    #[test]
    fn resetting_a_mode_turns_tracking_off_and_the_encoding_back_r_ssh_18() {
        let m = modes(b"\x1b[?1002h\x1b[?1006h\x1b[?1000l\x1b[?1006l");
        assert_eq!(
            (m.mouse, m.mouse_encoding),
            (MouseTracking::Off, MouseEncoding::Default)
        );
        let m = modes(b"\x1b[?1000h\x1b[?1003h\x1b[?1005h\x1b[?1006l");
        assert_eq!(
            (m.mouse, m.mouse_encoding),
            (MouseTracking::AnyEvent, MouseEncoding::Utf8),
            "the last mode set counts; resetting another encoding keeps this one"
        );
    }
}
