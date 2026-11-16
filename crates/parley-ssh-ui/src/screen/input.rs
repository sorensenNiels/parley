//! egui's keyboard events as what the terminal does with them (R-SSH-7, R-SSH-9): bytes for the host, or one of
//! Parley SSH's own shortcuts, which use Ctrl+Shift (Cmd on macOS) so Ctrl+letter stays the host's.

use egui::{Event, Key as EguiKey, Modifiers};
use parley_ssh_core::keys::{self, Key, Mods};
use parley_ssh_core::term::Modes;

/// A shortcut of Parley SSH's own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shortcut {
    NewSession,
    Duplicate,
    Restart,
}

/// What a key or clipboard event asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Typed {
    Send(Vec<u8>),
    Copy,
    /// Read the clipboard; its text comes back as a paste event.
    RequestPaste,
    Paste(String),
    /// Shift+Page Up (1) or Page Down (-1): the scrollback by a page, nothing sent (R-SSH-8).
    ScrollPage(isize),
    Shortcut(Shortcut),
}

/// Reads one frame's events. `awaiting_paste` is set by [`Typed::RequestPaste`] and cleared by the paste that
/// answers it, which is then pasted whatever keys are held.
pub fn read(events: &[Event], modes: &Modes, mac: bool, awaiting_paste: &mut bool) -> Vec<Typed> {
    let mut typed = Vec::new();
    // Alt+letter is sent from its key event; the text event some systems add for it is dropped.
    let mut alt_sent = false;
    for event in events {
        match event {
            Event::Text(text) => {
                if !std::mem::take(&mut alt_sent) {
                    typed.push(Typed::Send(text.as_bytes().to_vec()));
                }
            }
            Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } => {
                if let Some(t) = key_event(*key, *modifiers, modes, mac) {
                    alt_sent = is_alt_char(*key, *modifiers, mac);
                    typed.push(t);
                }
            }
            Event::Copy => typed.push(if mac {
                Typed::Copy
            } else {
                Typed::Send(vec![0x03])
            }),
            Event::Cut if !mac => typed.push(Typed::Send(vec![0x18])),
            Event::Paste(text) => {
                if std::mem::take(awaiting_paste) || mac {
                    typed.push(Typed::Paste(text.clone()));
                } else {
                    // Ctrl+V: egui made it a paste; the host gets its control character.
                    typed.push(Typed::Send(vec![0x16]));
                }
            }
            _ => {}
        }
    }
    if typed.contains(&Typed::RequestPaste) {
        *awaiting_paste = true;
    }
    typed
}

/// Parley SSH's own shortcuts: Ctrl+Shift+letter, or Cmd+letter on macOS.
fn shortcut(key: EguiKey, m: Modifiers, mac: bool) -> Option<Typed> {
    let ours = if mac {
        m.mac_cmd
    } else {
        m.ctrl && m.shift && !m.alt
    };
    if !ours {
        return None;
    }
    Some(match key {
        EguiKey::C => Typed::Copy,
        EguiKey::V => Typed::RequestPaste,
        EguiKey::N => Typed::Shortcut(Shortcut::NewSession),
        EguiKey::D => Typed::Shortcut(Shortcut::Duplicate),
        EguiKey::R => Typed::Shortcut(Shortcut::Restart),
        _ => return None,
    })
}

fn key_event(key: EguiKey, m: Modifiers, terminal: &Modes, mac: bool) -> Option<Typed> {
    if let Some(ours) = shortcut(key, m, mac) {
        return Some(ours);
    }
    let mods = Mods {
        shift: m.shift,
        ctrl: m.ctrl,
        alt: m.alt && !mac,
    };
    match key {
        EguiKey::Insert if m.shift && !m.ctrl => return Some(Typed::RequestPaste),
        EguiKey::PageUp if m.shift => return Some(Typed::ScrollPage(1)),
        EguiKey::PageDown if m.shift => return Some(Typed::ScrollPage(-1)),
        _ => {}
    }
    if let Some(named) = named(key) {
        return Some(Typed::Send(keys::encode(named, mods, terminal)));
    }
    // Ctrl+key and Alt+key: the text event does not carry them.
    let c = character(key, m.shift)?;
    let ctrl = m.ctrl && !m.alt;
    if ctrl || is_alt_char(key, m, mac) {
        return Some(Typed::Send(keys::encode(
            Key::Char(c),
            Mods {
                shift: false,
                ctrl,
                alt: !ctrl,
            },
            terminal,
        )));
    }
    None
}

/// Alt (not `AltGr`, which comes as Ctrl+Alt on Windows) with a character key, outside macOS where Option types.
fn is_alt_char(key: EguiKey, m: Modifiers, mac: bool) -> bool {
    m.alt && !m.ctrl && !mac && character(key, m.shift).is_some()
}

fn named(key: EguiKey) -> Option<Key> {
    Some(match key {
        EguiKey::Enter => Key::Enter,
        EguiKey::Backspace => Key::Backspace,
        EguiKey::Tab => Key::Tab,
        EguiKey::Escape => Key::Escape,
        EguiKey::ArrowUp => Key::Up,
        EguiKey::ArrowDown => Key::Down,
        EguiKey::ArrowRight => Key::Right,
        EguiKey::ArrowLeft => Key::Left,
        EguiKey::Home => Key::Home,
        EguiKey::End => Key::End,
        EguiKey::Insert => Key::Insert,
        EguiKey::Delete => Key::Delete,
        EguiKey::PageUp => Key::PageUp,
        EguiKey::PageDown => Key::PageDown,
        EguiKey::F1 => Key::F(1),
        EguiKey::F2 => Key::F(2),
        EguiKey::F3 => Key::F(3),
        EguiKey::F4 => Key::F(4),
        EguiKey::F5 => Key::F(5),
        EguiKey::F6 => Key::F(6),
        EguiKey::F7 => Key::F(7),
        EguiKey::F8 => Key::F(8),
        EguiKey::F9 => Key::F(9),
        EguiKey::F10 => Key::F(10),
        EguiKey::F11 => Key::F(11),
        EguiKey::F12 => Key::F(12),
        _ => return None,
    })
}

/// The character a key types on a US layout, for Ctrl and Alt combinations.
fn character(key: EguiKey, shift: bool) -> Option<char> {
    let name = key.name();
    let c = match key {
        EguiKey::Space => ' ',
        EguiKey::OpenBracket => '[',
        EguiKey::CloseBracket => ']',
        EguiKey::Backslash => '\\',
        EguiKey::Minus => '-',
        EguiKey::Slash => '_',
        EguiKey::Period => '.',
        EguiKey::Comma => ',',
        _ if name.len() == 1 => name.chars().next()?.to_ascii_lowercase(),
        _ => return None,
    };
    Some(if shift { c.to_ascii_uppercase() } else { c })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(key: EguiKey, modifiers: Modifiers) -> Event {
        Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }
    }

    fn read_one(events: &[Event]) -> Vec<Typed> {
        read(events, &Modes::default(), false, &mut false)
    }

    const CTRL: Modifiers = Modifiers {
        ctrl: true,
        command: true,
        ..Modifiers::NONE
    };
    const CTRL_SHIFT: Modifiers = Modifiers {
        ctrl: true,
        command: true,
        shift: true,
        ..Modifiers::NONE
    };
    const ALT: Modifiers = Modifiers {
        alt: true,
        ..Modifiers::NONE
    };

    #[test]
    fn typed_text_and_named_keys_are_sent_r_ssh_7() {
        let typed = read_one(&[
            Event::Text("æx".into()),
            key(EguiKey::Enter, Modifiers::NONE),
        ]);
        assert_eq!(
            typed,
            [
                Typed::Send("æx".as_bytes().to_vec()),
                Typed::Send(b"\r".to_vec())
            ]
        );
        assert_eq!(
            read_one(&[key(EguiKey::F5, Modifiers::NONE)]),
            [Typed::Send(b"\x1b[15~".to_vec())]
        );
    }

    #[test]
    fn ctrl_letters_go_to_the_host_also_when_egui_made_them_clipboard_events_r_ssh_7() {
        assert_eq!(
            read_one(&[key(EguiKey::D, CTRL)]),
            [Typed::Send(vec![0x04])]
        );
        assert_eq!(read_one(&[Event::Copy]), [Typed::Send(vec![0x03])]);
        assert_eq!(read_one(&[Event::Cut]), [Typed::Send(vec![0x18])]);
        assert_eq!(
            read_one(&[Event::Paste("x".into())]),
            [Typed::Send(vec![0x16])]
        );
        assert_eq!(
            read_one(&[key(EguiKey::OpenBracket, CTRL)]),
            [Typed::Send(vec![0x1B])]
        );
    }

    #[test]
    fn ctrl_shift_letters_are_parley_sshs_own_r_ssh_7() {
        assert_eq!(read_one(&[key(EguiKey::C, CTRL_SHIFT)]), [Typed::Copy]);
        assert_eq!(
            read_one(&[key(EguiKey::R, CTRL_SHIFT)]),
            [Typed::Shortcut(Shortcut::Restart)]
        );
        assert_eq!(
            read_one(&[key(EguiKey::X, CTRL_SHIFT)]),
            [Typed::Send(vec![0x18])]
        );
    }

    #[test]
    fn a_requested_paste_is_pasted_whatever_keys_are_held_r_ssh_9() {
        let mut awaiting = false;
        let typed = read(
            &[key(EguiKey::V, CTRL_SHIFT)],
            &Modes::default(),
            false,
            &mut awaiting,
        );
        assert_eq!(typed, [Typed::RequestPaste]);
        assert!(awaiting);
        let typed = read(
            &[Event::Paste("ls\n".into())],
            &Modes::default(),
            false,
            &mut awaiting,
        );
        assert_eq!(typed, [Typed::Paste("ls\n".into())]);
        assert!(!awaiting);
        let shift_insert = Modifiers {
            shift: true,
            ..Modifiers::NONE
        };
        assert_eq!(
            read_one(&[key(EguiKey::Insert, shift_insert)]),
            [Typed::RequestPaste]
        );
    }

    #[test]
    fn alt_letters_send_escape_first_and_their_text_is_dropped_r_ssh_7() {
        let typed = read_one(&[
            key(EguiKey::B, ALT),
            Event::Text("b".into()),
            Event::Text("c".into()),
        ]);
        assert_eq!(
            typed,
            [Typed::Send(b"\x1bb".to_vec()), Typed::Send(b"c".to_vec())]
        );
    }

    #[test]
    fn altgr_as_ctrl_alt_types_its_text() {
        let altgr = Modifiers {
            ctrl: true,
            alt: true,
            ..Modifiers::NONE
        };
        let typed = read_one(&[key(EguiKey::Num2, altgr), Event::Text("@".into())]);
        assert_eq!(typed, [Typed::Send(b"@".to_vec())]);
    }

    #[test]
    fn shift_page_keys_scroll_the_scrollback_r_ssh_8() {
        let shift = Modifiers {
            shift: true,
            ..Modifiers::NONE
        };
        assert_eq!(
            read_one(&[key(EguiKey::PageUp, shift)]),
            [Typed::ScrollPage(1)]
        );
        assert_eq!(
            read_one(&[key(EguiKey::PageDown, shift)]),
            [Typed::ScrollPage(-1)]
        );
    }

    #[test]
    fn on_macos_cmd_copies_and_pastes_and_ctrl_stays_the_hosts() {
        let cmd = Modifiers {
            mac_cmd: true,
            command: true,
            ..Modifiers::NONE
        };
        let mac_ctrl = Modifiers {
            ctrl: true,
            ..Modifiers::NONE
        };
        let modes = Modes::default();
        assert_eq!(
            read(&[Event::Copy], &modes, true, &mut false),
            [Typed::Copy]
        );
        assert_eq!(
            read(&[Event::Paste("x".into())], &modes, true, &mut false),
            [Typed::Paste("x".into())]
        );
        assert_eq!(
            read(&[key(EguiKey::R, cmd)], &modes, true, &mut false),
            [Typed::Shortcut(Shortcut::Restart)]
        );
        assert_eq!(
            read(&[key(EguiKey::C, mac_ctrl)], &modes, true, &mut false),
            [Typed::Send(vec![0x03])]
        );
    }
}
