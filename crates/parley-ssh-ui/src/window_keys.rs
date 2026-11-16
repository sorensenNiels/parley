//! The window's own shortcuts (R-SSH-39..41): tabs, panes and sessions. They are taken out of the frame's input
//! before the panes see it, so they work on a start screen as in a terminal, and never reach the host. As R-SSH-7,
//! they use Ctrl+Shift (Cmd on macOS); tabs by number Ctrl+Alt+1…9 (Cmd+1…9), as Windows Terminal, since Ctrl+2…8
//! are control characters for the host; panes Alt+Shift+= and Alt+Shift+-, and Alt+arrows only while a tab is
//! split, so they stay the host's otherwise.

use egui::{Event, Key, Modifiers};

use crate::panes::Direction;

/// A window shortcut.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowKey {
    NewTab,
    CloseTab,
    NextTab,
    PreviousTab,
    /// The tab with this number, from 1.
    Tab(usize),
    SplitRight,
    SplitDown,
    Focus(Direction),
    Duplicate,
    NewWindow,
    Restart,
}

/// Which modifiers a shortcut needs: exactly these, nothing more.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[expect(clippy::struct_excessive_bools, reason = "one per modifier key")]
struct Held {
    ctrl: bool,
    shift: bool,
    alt: bool,
    cmd: bool,
}

impl Held {
    fn of(m: Modifiers) -> Self {
        Self {
            ctrl: m.ctrl,
            shift: m.shift,
            alt: m.alt,
            cmd: m.mac_cmd,
        }
    }
}

const NONE: Held = Held {
    ctrl: false,
    shift: false,
    alt: false,
    cmd: false,
};

/// Takes the window's shortcuts out of `events`; `split` says whether the tab in front has several panes.
pub fn take(events: &mut Vec<Event>, mac: bool, split: bool) -> Vec<WindowKey> {
    let mut taken = Vec::new();
    events.retain(|event| {
        let Event::Key {
            key,
            pressed,
            modifiers,
            ..
        } = event
        else {
            return true;
        };
        match shortcut(*key, Held::of(*modifiers), mac, split) {
            Some(found) => {
                if *pressed {
                    taken.push(found);
                }
                false
            }
            None => true,
        }
    });
    taken
}

fn shortcut(key: Key, held: Held, mac: bool, split: bool) -> Option<WindowKey> {
    // Parley SSH's own: Ctrl+Shift, or Cmd on macOS.
    let ours = if mac {
        Held { cmd: true, ..NONE }
    } else {
        Held {
            ctrl: true,
            shift: true,
            ..NONE
        }
    };
    let ours_alt = Held { alt: true, ..ours };
    let ctrl = Held { ctrl: true, ..NONE };
    let alt_shift = Held {
        alt: true,
        shift: true,
        ..NONE
    };
    let tab_number = if mac {
        Held { cmd: true, ..NONE }
    } else {
        Held {
            ctrl: true,
            alt: true,
            ..NONE
        }
    };
    Some(match (key, held) {
        (Key::T | Key::N, h) if h == ours => WindowKey::NewTab,
        (Key::N, h) if h == ours_alt => WindowKey::NewWindow,
        (Key::W, h) if h == ours => WindowKey::CloseTab,
        (Key::D, h) if h == ours => WindowKey::Duplicate,
        (Key::R, h) if h == ours => WindowKey::Restart,
        (Key::Tab, h) if h == ctrl => WindowKey::NextTab,
        (Key::Tab, h)
            if h == Held {
                shift: true,
                ..ctrl
            } =>
        {
            WindowKey::PreviousTab
        }
        (Key::Equals | Key::Plus, h) if h == alt_shift => WindowKey::SplitRight,
        (Key::Minus, h) if h == alt_shift => WindowKey::SplitDown,
        (Key::ArrowLeft, h) if split && h == Held { alt: true, ..NONE } => {
            WindowKey::Focus(Direction::Left)
        }
        (Key::ArrowRight, h) if split && h == Held { alt: true, ..NONE } => {
            WindowKey::Focus(Direction::Right)
        }
        (Key::ArrowUp, h) if split && h == Held { alt: true, ..NONE } => {
            WindowKey::Focus(Direction::Up)
        }
        (Key::ArrowDown, h) if split && h == Held { alt: true, ..NONE } => {
            WindowKey::Focus(Direction::Down)
        }
        (key, h) if h == tab_number => WindowKey::Tab(digit(key)?),
        _ => return None,
    })
}

fn digit(key: Key) -> Option<usize> {
    Some(match key {
        Key::Num1 => 1,
        Key::Num2 => 2,
        Key::Num3 => 3,
        Key::Num4 => 4,
        Key::Num5 => 5,
        Key::Num6 => 6,
        Key::Num7 => 7,
        Key::Num8 => 8,
        Key::Num9 => 9,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(key: Key, modifiers: Modifiers) -> Event {
        Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }
    }

    fn keys(events: &[Event], mac: bool, split: bool) -> (Vec<WindowKey>, usize) {
        let mut events = events.to_vec();
        let taken = take(&mut events, mac, split);
        (taken, events.len())
    }

    const CTRL_SHIFT: Modifiers = Modifiers {
        ctrl: true,
        shift: true,
        command: true,
        ..Modifiers::NONE
    };
    const ALT_SHIFT: Modifiers = Modifiers {
        alt: true,
        shift: true,
        ..Modifiers::NONE
    };
    const ALT: Modifiers = Modifiers {
        alt: true,
        ..Modifiers::NONE
    };
    const CMD: Modifiers = Modifiers {
        mac_cmd: true,
        command: true,
        ..Modifiers::NONE
    };

    #[test]
    fn tab_and_pane_keys_are_taken_and_never_reach_the_host_r_ssh_39_r_ssh_40() {
        let events = [
            press(Key::T, CTRL_SHIFT),
            press(Key::W, CTRL_SHIFT),
            press(Key::Equals, ALT_SHIFT),
            press(Key::Minus, ALT_SHIFT),
            press(
                Key::Num3,
                Modifiers {
                    ctrl: true,
                    alt: true,
                    command: true,
                    ..Modifiers::NONE
                },
            ),
            press(
                Key::Tab,
                Modifiers {
                    ctrl: true,
                    command: true,
                    ..Modifiers::NONE
                },
            ),
            Event::Text("x".into()),
        ];
        let (taken, left) = keys(&events, false, false);
        assert_eq!(
            taken,
            [
                WindowKey::NewTab,
                WindowKey::CloseTab,
                WindowKey::SplitRight,
                WindowKey::SplitDown,
                WindowKey::Tab(3),
                WindowKey::NextTab
            ]
        );
        assert_eq!(left, 1, "the typed text stays");
    }

    #[test]
    fn ctrl_digits_and_ctrl_letters_stay_the_hosts_r_ssh_39() {
        let ctrl = Modifiers {
            ctrl: true,
            command: true,
            ..Modifiers::NONE
        };
        let (taken, left) = keys(
            &[
                press(Key::Num3, ctrl),
                press(Key::T, ctrl),
                press(Key::W, ctrl),
            ],
            false,
            false,
        );
        assert_eq!((taken, left), (vec![], 3));
    }

    #[test]
    fn alt_arrows_move_between_panes_only_in_a_split_tab_r_ssh_40() {
        assert_eq!(
            keys(&[press(Key::ArrowLeft, ALT)], false, false),
            (vec![], 1),
            "the host's when not split"
        );
        assert_eq!(
            keys(&[press(Key::ArrowLeft, ALT)], false, true).0,
            [WindowKey::Focus(Direction::Left)]
        );
    }

    #[test]
    fn on_macos_cmd_is_parley_sshs_r_ssh_39() {
        let (taken, _) = keys(
            &[
                press(Key::T, CMD),
                press(Key::Num2, CMD),
                press(Key::N, Modifiers { alt: true, ..CMD }),
            ],
            true,
            false,
        );
        assert_eq!(
            taken,
            [WindowKey::NewTab, WindowKey::Tab(2), WindowKey::NewWindow]
        );
        assert_eq!(
            keys(&[press(Key::T, CTRL_SHIFT)], true, false).0,
            [],
            "Ctrl+Shift is the host's on macOS"
        );
    }

    #[test]
    fn a_shortcut_with_another_key_held_too_is_not_taken() {
        let more = Modifiers {
            alt: true,
            ..CTRL_SHIFT
        };
        assert_eq!(keys(&[press(Key::T, more)], false, false), (vec![], 1));
        assert_eq!(
            keys(&[press(Key::N, more)], false, false).0,
            [WindowKey::NewWindow]
        );
    }
}
