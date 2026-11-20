//! The operator in GUI selection fields and scroll bars (R-DS-11, R-DS-12, D57): the cursor
//! keys go from choice to choice, Space, `/`, a mnemonic, a click or (on a menu bar or a push
//! button) Enter selects, and a choice's AID or the field's auto-enter sends it to the host.

use super::Session;
use super::input::{Aid, Key};
use crate::screen::{ChoiceState, Direction, SelectionField, SelectionKind};

impl Session {
    /// The selection field and choice at `address`.
    fn choice_at(&self, address: usize) -> Option<(usize, usize)> {
        self.screen
            .selection_fields()
            .iter()
            .enumerate()
            .find_map(|(i, f)| f.choice_at(address).map(|c| (i, c)))
    }

    /// Carries out `key` when the cursor is on a choice; false when the key is not one a
    /// selection field takes, so it does what it does elsewhere.
    pub(super) fn choice_key(&mut self, key: Key) -> bool {
        if !self.config.gui {
            return false;
        }
        let Some((f, c)) = self.choice_at(self.screen.cursor) else {
            return false;
        };
        let field = &self.screen.selection_fields()[f];
        let direction = match key {
            Key::Up => Some(Direction::Up),
            Key::Down => Some(Direction::Down),
            Key::Left => Some(Direction::Left),
            Key::Right => Some(Direction::Right),
            _ => None,
        };
        if let Some(direction) = direction {
            if let Some(aid) = field.choices[c].roll(direction) {
                self.send_choice_aid(Aid::from_code(aid));
                return true;
            }
            return match neighbour(field, c, direction) {
                Some(n) => {
                    self.screen.cursor = field.text_start(n).unwrap_or(self.screen.cursor);
                    true
                }
                None => field.keeps_cursor,
            };
        }
        match key {
            Key::Char(' ' | '/') => {
                self.choose(f, c);
                true
            }
            Key::Char(ch) => {
                let page = self.code_page();
                let wanted = ch.to_lowercase().next();
                let found = field.choices.iter().position(|choice| {
                    choice
                        .mnemonic
                        .and_then(|m| choice.text.get(m))
                        .and_then(|&b| page.to_char(b).to_lowercase().next())
                        == wanted
                });
                if let Some(n) = found {
                    self.screen.cursor = field.text_start(n).unwrap_or(self.screen.cursor);
                    self.choose(f, n);
                }
                true
            }
            Key::Aid(aid) if aid == Aid::ENTER && field.kind.pressed() => {
                if !self.choose(f, c) {
                    self.send_choice_aid(Aid::ENTER);
                }
                true
            }
            _ => false,
        }
    }

    /// Selects (or for a check box, clears) a choice; sends its AID, or Enter if the field is
    /// auto-enter (the manual does not say which AID; Parley sends Enter). True if it sent.
    fn choose(&mut self, f: usize, c: usize) -> bool {
        if !self.screen.toggle_choice(f, c) {
            return false;
        }
        let field = &self.screen.selection_fields()[f];
        let choice = &field.choices[c];
        let selected = choice.state == ChoiceState::Selected;
        let aid = match (choice.aid, field.auto_enter) {
            (Some(aid), _) if selected => Aid::from_code(aid),
            (_, true) => Aid::ENTER,
            _ => return false,
        };
        self.send_choice_aid(aid);
        true
    }

    fn send_choice_aid(&mut self, aid: Aid) {
        match self.check_before_aid(aid) {
            Ok(()) => self.send_aid(aid),
            Err(error) => self.error = Some(error),
        }
    }

    /// A click at a 0-based row and column (R-DS-11): on a choice it puts the cursor there and
    /// selects it; anywhere else it moves the cursor, as [`Key::MoveTo`].
    pub fn click(&mut self, row: usize, column: usize) {
        let columns = usize::from(self.screen.size().columns());
        let address = row * columns + column;
        let on_choice = self
            .choice_at(address)
            .filter(|_| self.config.gui && !self.locked && self.error.is_none());
        let Some((f, c)) = on_choice else {
            self.key(Key::MoveTo { row, column });
            return;
        };
        let field = &self.screen.selection_fields()[f];
        if !field.choices[c].takes_cursor() {
            return;
        }
        self.screen.cursor = field.text_start(c).unwrap_or(address);
        self.choose(f, c);
    }

    /// Moves scroll bar `bar`'s slider to `position` (R-DS-12): the host is sent Roll Down
    /// (back) or Roll Up (on) with the new position. The manual's scroll AID was not at hand;
    /// these are the 5250's own (D57).
    pub fn scroll(&mut self, bar: usize, position: u32) {
        if !self.config.gui || self.locked || self.error.is_some() {
            return;
        }
        let Some(old) = self.screen.scroll_bars().get(bar).cloned() else {
            return;
        };
        if !self.screen.set_scroll_position(bar, position) {
            return;
        }
        if old.move_cursor {
            self.screen.cursor = old.start;
        }
        let back = position < old.position;
        self.send_choice_aid(if back { Aid::ROLL_DOWN } else { Aid::ROLL_UP });
    }
}

/// The choice the cursor key goes to: the one before or after it, or a row up or down; a menu
/// bar has no rows. Choices that do not take the cursor are passed over.
fn neighbour(field: &SelectionField, c: usize, direction: Direction) -> Option<usize> {
    let step = match direction {
        Direction::Left | Direction::Right => 1,
        Direction::Up | Direction::Down if field.kind == SelectionKind::MenuBar => return None,
        Direction::Up | Direction::Down => field.columns.max(1),
    };
    let back = matches!(direction, Direction::Left | Direction::Up);
    let mut n = c;
    loop {
        n = if back { n.checked_sub(step)? } else { n + step };
        let choice = field.choices.get(n)?;
        if choice.takes_cursor() {
            return Some(n);
        }
    }
}
