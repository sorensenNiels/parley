//! Programmable mouse buttons (R-DS-12, D57; SC30-3533-04 §15.6.13.10): the host names mouse
//! events, or pairs of them, that send an AID. Without definitions a click does what it always
//! did.

use super::Session;
use super::input::Aid;

/// A mouse button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// What the button did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseAction {
    Press,
    Release,
    DoubleClick,
}

/// A mouse event as the host names it (codes X'01'-X'12': left, right and middle pressed,
/// released and double-clicked, then the same with Shift).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MouseEvent {
    pub button: MouseButton,
    pub action: MouseAction,
    pub shift: bool,
}

impl MouseEvent {
    pub const fn from_code(code: u8) -> Option<Self> {
        if code == 0 || code > 0x12 {
            return None;
        }
        let n = code - 1;
        let button = match (n % 9) / 3 {
            0 => MouseButton::Left,
            1 => MouseButton::Right,
            _ => MouseButton::Middle,
        };
        let action = match n % 3 {
            0 => MouseAction::Press,
            1 => MouseAction::Release,
            _ => MouseAction::DoubleClick,
        };
        Some(Self {
            button,
            action,
            shift: n >= 9,
        })
    }
}

/// One definition: an event, or a first and a second one, and the AID it sends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MouseDefinition {
    pub two_events: bool,
    /// The cursor goes where the mouse is first.
    pub move_cursor: bool,
    pub first: MouseEvent,
    pub second: Option<MouseEvent>,
    pub aid: u8,
}

impl Session {
    /// A mouse event at a 0-based row and column. True if a definition from the host took it;
    /// then the UI does nothing else with it. A definition of two events takes the first and
    /// sends its AID on the second.
    pub fn mouse(&mut self, event: MouseEvent, row: usize, column: usize) -> bool {
        if !self.config.gui || self.mouse_buttons.is_empty() {
            return false;
        }
        let Some(address) = self.screen.address(
            u8::try_from(row + 1).unwrap_or(0),
            u8::try_from(column + 1).unwrap_or(0),
        ) else {
            return false;
        };
        let pending = self.mouse_first.take();
        let fired = self.mouse_buttons.iter().enumerate().find_map(|(i, d)| {
            let fires = if d.two_events {
                pending == Some(i) && d.second == Some(event)
            } else {
                d.first == event
            };
            fires.then_some(*d)
        });
        if let Some(d) = fired {
            if !self.locked && self.error.is_none() {
                if d.move_cursor {
                    self.screen.cursor = address;
                }
                self.send_aid(Aid::from_code(d.aid));
            }
            return true;
        }
        if let Some(i) = self
            .mouse_buttons
            .iter()
            .position(|d| d.two_events && d.first == event)
        {
            self.mouse_first = Some(i);
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mouse_event_codes_name_button_action_and_shift_r_ds_12() {
        let event = |button, action, shift| {
            Some(MouseEvent {
                button,
                action,
                shift,
            })
        };
        assert_eq!(MouseEvent::from_code(0x00), None);
        assert_eq!(
            MouseEvent::from_code(0x01),
            event(MouseButton::Left, MouseAction::Press, false)
        );
        assert_eq!(
            MouseEvent::from_code(0x06),
            event(MouseButton::Right, MouseAction::DoubleClick, false)
        );
        assert_eq!(
            MouseEvent::from_code(0x08),
            event(MouseButton::Middle, MouseAction::Release, false)
        );
        assert_eq!(
            MouseEvent::from_code(0x0A),
            event(MouseButton::Left, MouseAction::Press, true)
        );
        assert_eq!(
            MouseEvent::from_code(0x12),
            event(MouseButton::Middle, MouseAction::DoubleClick, true)
        );
        assert_eq!(MouseEvent::from_code(0x13), None);
    }
}
