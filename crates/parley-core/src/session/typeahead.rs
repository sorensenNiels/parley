//! Type-ahead (R-KEY-11): keys pressed while the keyboard waits for the host (X SYSTEM) are
//! kept and carried out in order once the host unlocks it, as IBM's emulators can. Off unless
//! the operator turns it on.

use std::collections::VecDeque;

use super::Session;
use super::input::Key;

/// At most this many keys are kept; more are dropped.
pub const TYPE_AHEAD_KEYS: usize = 256;

impl Session {
    /// Turns type-ahead on or off; off drops the keys kept.
    pub fn set_type_ahead(&mut self, on: bool) {
        self.type_ahead_on = on;
        if !on {
            self.typed_ahead.clear();
        }
    }

    /// How many keys are kept to be carried out when the keyboard unlocks.
    pub fn typed_ahead(&self) -> usize {
        self.typed_ahead.len()
    }

    /// Keeps `key` for later when the keyboard waits for the host and type-ahead is on;
    /// true when it was kept (or dropped because the buffer is full). Reset clears the keys
    /// kept; Attention, System Request and Reset itself are never kept, nor a click.
    pub(super) fn hold_for_later(&mut self, key: Key) -> bool {
        if key == Key::Reset {
            self.typed_ahead.clear();
        }
        let kept = match key {
            Key::Reset | Key::Attention | Key::SystemRequest | Key::MoveTo { .. } => false,
            Key::Char(_)
            | Key::Aid(_)
            | Key::FieldExit
            | Key::FieldPlus
            | Key::FieldMinus
            | Key::Tab
            | Key::BackTab
            | Key::Up
            | Key::Down
            | Key::Left
            | Key::Right
            | Key::Home
            | Key::End
            | Key::Backspace
            | Key::Delete
            | Key::ToggleInsert
            | Key::EraseEof => true,
        };
        if !(kept && self.type_ahead_on && self.locked && self.error.is_none()) {
            return false;
        }
        if self.typed_ahead.len() < TYPE_AHEAD_KEYS {
            self.typed_ahead.push_back(key);
        }
        true
    }

    /// Carries out the keys kept while the keyboard is unlocked; stops when one locks it again
    /// (the rest wait for the next unlock) and drops the rest at an operator error, so the
    /// operator sees it before typing on.
    pub(super) fn replay_typed_ahead(&mut self) {
        while !self.locked && self.error.is_none() {
            let Some(key) = self.typed_ahead.pop_front() else {
                return;
            };
            self.key(key);
        }
        if self.error.is_some() {
            self.typed_ahead.clear();
        }
    }
}

/// The keys kept, oldest first.
pub(super) type TypedAhead = VecDeque<Key>;
