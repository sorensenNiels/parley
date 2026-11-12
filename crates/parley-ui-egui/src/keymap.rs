//! PC keys to 5250 keyboard functions (R-KEY-1). The defaults follow IBM's own emulators where
//! a PC can tell the keys apart: Esc is Attention, a Ctrl key pressed alone is Reset, Page Up
//! and Page Down roll.

use egui::{Event, Key as PcKey, Modifiers};
use parley_core::{Aid, Key};

/// The keyboard shortcuts, for the help text.
pub const SHORTCUTS: &[(&str, &str)] = &[
    ("Enter", "Enter"),
    ("Shift+Enter, Ctrl+Enter", "Field Exit"),
    ("Ctrl++ / Ctrl+-", "Field+ / Field-"),
    ("Tab / Shift+Tab", "Next / previous field"),
    ("F1–F12, Shift+F1–F12", "F1–F12, F13–F24"),
    ("Page Up / Page Down", "Roll Down / Roll Up"),
    ("Ctrl (alone), Ctrl+R", "Reset"),
    ("Esc / Shift+Esc", "Attention / System Request"),
    ("Insert", "Insert mode"),
    ("Ctrl+End", "Erase to end of field"),
    ("Ctrl+C / Ctrl+V", "Copy the screen / paste into fields"),
];

/// Turns egui input into 5250 keys, frame by frame. It remembers the modifiers between frames
/// to notice a Ctrl key pressed and released on its own.
#[derive(Debug, Default)]
pub struct Keymap {
    previous: Modifiers,
    ctrl_alone: bool,
}

impl Keymap {
    pub fn keys(&mut self, events: &[Event], modifiers: Modifiers) -> Vec<Key> {
        let mut keys = Vec::new();
        let mut other_input = false;
        for event in events {
            match event {
                Event::Text(text) => {
                    other_input = true;
                    keys.extend(text.chars().filter(|c| !c.is_control()).map(Key::Char));
                }
                Event::Paste(text) => {
                    other_input = true;
                    keys.extend(pasted(text));
                }
                Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } => {
                    other_input = true;
                    keys.extend(map_key(*key, *modifiers));
                }
                Event::Copy | Event::Cut => other_input = true,
                _ => {}
            }
        }

        let ctrl_only = modifiers.ctrl && !modifiers.shift && !modifiers.alt;
        let was_ctrl_only = self.previous.ctrl && !self.previous.shift && !self.previous.alt;
        if other_input || (modifiers.ctrl && !ctrl_only) {
            self.ctrl_alone = false;
        } else if ctrl_only && !was_ctrl_only {
            self.ctrl_alone = true;
        } else if !modifiers.ctrl && self.ctrl_alone {
            self.ctrl_alone = false;
            keys.push(Key::Reset);
        }
        self.previous = modifiers;
        keys
    }
}

/// Pasted text goes into fields as typed; a line break or tab moves to the next field.
fn pasted(text: &str) -> impl Iterator<Item = Key> + '_ {
    let mut previous = None;
    text.chars().filter_map(move |c| {
        let after_cr = previous == Some('\r');
        previous = Some(c);
        match c {
            '\n' if after_cr => None,
            '\r' | '\t' | '\n' => Some(Key::FieldExit),
            c if c.is_control() => None,
            c => Some(Key::Char(c)),
        }
    })
}

fn map_key(key: PcKey, m: Modifiers) -> Option<Key> {
    if let Some(n) = function_number(key) {
        let n = if m.shift && n <= 12 { n + 12 } else { n };
        return Aid::function(n).map(Key::Aid);
    }
    Some(match key {
        PcKey::Enter if m.shift || m.ctrl => Key::FieldExit,
        PcKey::Enter => Key::Aid(Aid::ENTER),
        PcKey::Tab if m.shift => Key::BackTab,
        PcKey::Tab => Key::Tab,
        PcKey::Escape if m.shift => Key::SystemRequest,
        PcKey::Escape => Key::Attention,
        PcKey::PageUp => Key::Aid(Aid::ROLL_DOWN),
        PcKey::PageDown => Key::Aid(Aid::ROLL_UP),
        PcKey::ArrowUp => Key::Up,
        PcKey::ArrowDown => Key::Down,
        PcKey::ArrowLeft => Key::Left,
        PcKey::ArrowRight => Key::Right,
        PcKey::Home => Key::Home,
        PcKey::End if m.ctrl => Key::EraseEof,
        PcKey::End => Key::End,
        PcKey::Backspace => Key::Backspace,
        PcKey::Delete => Key::Delete,
        PcKey::Insert => Key::ToggleInsert,
        PcKey::R if m.ctrl => Key::Reset,
        PcKey::Minus if m.ctrl => Key::FieldMinus,
        PcKey::Plus | PcKey::Equals if m.ctrl => Key::FieldPlus,
        _ => return None,
    })
}

fn function_number(key: PcKey) -> Option<u8> {
    const KEYS: [PcKey; 24] = [
        PcKey::F1,
        PcKey::F2,
        PcKey::F3,
        PcKey::F4,
        PcKey::F5,
        PcKey::F6,
        PcKey::F7,
        PcKey::F8,
        PcKey::F9,
        PcKey::F10,
        PcKey::F11,
        PcKey::F12,
        PcKey::F13,
        PcKey::F14,
        PcKey::F15,
        PcKey::F16,
        PcKey::F17,
        PcKey::F18,
        PcKey::F19,
        PcKey::F20,
        PcKey::F21,
        PcKey::F22,
        PcKey::F23,
        PcKey::F24,
    ];
    let index = KEYS.iter().position(|&k| k == key)?;
    u8::try_from(index + 1).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(key: PcKey, modifiers: Modifiers) -> Event {
        Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }
    }

    fn keys_for(events: &[Event]) -> Vec<Key> {
        Keymap::default().keys(events, Modifiers::NONE)
    }

    #[test]
    fn enter_function_and_roll_keys_are_aids() {
        // R-KEY-1
        assert_eq!(
            keys_for(&[press(PcKey::Enter, Modifiers::NONE)]),
            [Key::Aid(Aid::ENTER)]
        );
        assert_eq!(
            keys_for(&[press(PcKey::F3, Modifiers::NONE)]),
            [Key::Aid(Aid::function(3).expect("F3"))]
        );
        assert_eq!(
            keys_for(&[press(PcKey::F1, Modifiers::SHIFT)]),
            [Key::Aid(Aid::function(13).expect("F13"))]
        );
        assert_eq!(
            keys_for(&[press(PcKey::F24, Modifiers::NONE)]),
            [Key::Aid(Aid::function(24).expect("F24"))]
        );
        assert_eq!(
            keys_for(&[press(PcKey::PageDown, Modifiers::NONE)]),
            [Key::Aid(Aid::ROLL_UP)]
        );
    }

    #[test]
    fn field_keys_use_modifiers() {
        assert_eq!(
            keys_for(&[press(PcKey::Enter, Modifiers::SHIFT)]),
            [Key::FieldExit]
        );
        assert_eq!(
            keys_for(&[press(PcKey::Minus, Modifiers::CTRL)]),
            [Key::FieldMinus]
        );
        assert_eq!(
            keys_for(&[press(PcKey::Tab, Modifiers::SHIFT)]),
            [Key::BackTab]
        );
        assert_eq!(
            keys_for(&[press(PcKey::End, Modifiers::CTRL)]),
            [Key::EraseEof]
        );
        assert_eq!(
            keys_for(&[press(PcKey::Escape, Modifiers::NONE)]),
            [Key::Attention]
        );
    }

    #[test]
    fn typed_and_pasted_text_become_characters() {
        let keys = keys_for(&[Event::Text("æ1".into()), Event::Paste("ab\r\nc\td".into())]);
        let expected = [
            Key::Char('æ'),
            Key::Char('1'),
            Key::Char('a'),
            Key::Char('b'),
            Key::FieldExit,
            Key::Char('c'),
            Key::FieldExit,
            Key::Char('d'),
        ];
        assert_eq!(keys, expected);
    }

    #[test]
    fn a_ctrl_key_pressed_and_released_alone_is_reset() {
        let mut keymap = Keymap::default();
        assert_eq!(keymap.keys(&[], Modifiers::CTRL), []);
        assert_eq!(keymap.keys(&[], Modifiers::NONE), [Key::Reset]);
    }

    #[test]
    fn ctrl_with_another_key_is_not_reset() {
        let mut keymap = Keymap::default();
        keymap.keys(&[], Modifiers::CTRL);
        assert_eq!(
            keymap.keys(&[press(PcKey::Minus, Modifiers::CTRL)], Modifiers::CTRL),
            [Key::FieldMinus]
        );
        assert_eq!(keymap.keys(&[], Modifiers::NONE), []);
        keymap.keys(&[], Modifiers::CTRL);
        keymap.keys(&[Event::Copy], Modifiers::CTRL);
        assert_eq!(keymap.keys(&[], Modifiers::NONE), []);
    }
}
