//! The operator's keys (R-DS-3, R-KEY-1): typing into fields under their field format word,
//! Field Exit, cursor movement, and AID keys that send input to the host.

use super::{OperatorError, Session};
use crate::record::Opcode;
use crate::record::flags;
use crate::screen::{Adjust, BLANK, Field, MINUS, Shift, ZERO, negative_zone};

/// An attention identifier: the key that sends input to the host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Aid(u8);

impl Aid {
    pub const ENTER: Self = Self(0xF1);
    pub const HELP: Self = Self(0xF3);
    /// Page Up on a PC keyboard.
    pub const ROLL_DOWN: Self = Self(0xF4);
    /// Page Down on a PC keyboard.
    pub const ROLL_UP: Self = Self(0xF5);
    pub const PRINT: Self = Self(0xF6);
    pub const RECORD_BACKSPACE: Self = Self(0xF8);
    pub const CLEAR: Self = Self(0xBD);

    /// F1 to F24.
    pub const fn function(n: u8) -> Option<Self> {
        match n {
            1..=12 => Some(Self(0x30 + n)),
            13..=24 => Some(Self(0xB0 + n - 12)),
            _ => None,
        }
    }

    /// The AID with this code, as a macro file names one without a name (`aid-XX`).
    pub(crate) const fn from_code(code: u8) -> Self {
        Self(code)
    }

    pub const fn code(self) -> u8 {
        self.0
    }

    /// The function key's number, 1-24, for F1-F24.
    pub const fn function_number(self) -> Option<u8> {
        match self.0 {
            0x31..=0x3C => Some(self.0 - 0x30),
            0xB1..=0xBC => Some(self.0 - 0xB0 + 12),
            _ => None,
        }
    }
}

/// A 5250 keyboard function. The UI maps PC keys to these.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Char(char),
    Aid(Aid),
    FieldExit,
    FieldPlus,
    FieldMinus,
    /// Field Advance: the next input field.
    Tab,
    /// Field Backspace: the start of this field, or the previous one.
    BackTab,
    Up,
    Down,
    Left,
    Right,
    Home,
    /// After the last character in the field.
    End,
    /// Deletes the character left of the cursor.
    Backspace,
    Delete,
    ToggleInsert,
    EraseEof,
    Reset,
    Attention,
    SystemRequest,
    /// Puts the cursor at a 0-based row and column, as a mouse click does.
    MoveTo {
        row: usize,
        column: usize,
    },
}

/// The sign Field Exit gives a number: Field Exit and Field+ make it positive, Field- negative.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Sign {
    Plus,
    Minus,
}

impl Session {
    /// Carries out a key, from the operator ([`Session::key`]) or a macro.
    pub(super) fn press(&mut self, key: Key) {
        // Attention, System Request and Reset work while the keyboard is locked or in error.
        match key {
            Key::Attention => return self.send_record(flags::ATTENTION, Opcode::NoOperation, &[]),
            Key::SystemRequest => {
                return self.send_record(flags::SYSTEM_REQUEST, Opcode::NoOperation, &[]);
            }
            Key::Reset => return self.reset(),
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
            | Key::EraseEof
            | Key::MoveTo { .. } => {}
        }
        if self.error.is_some() || self.locked {
            return;
        }
        let result = match key {
            Key::Char(c) => self.type_char(c),
            Key::Aid(aid) => {
                self.send_aid(aid);
                Ok(())
            }
            Key::FieldExit | Key::FieldPlus => self.field_exit(Sign::Plus),
            Key::FieldMinus => self.field_exit(Sign::Minus),
            Key::Delete => self.delete(),
            Key::Backspace => self.backspace(),
            Key::EraseEof => {
                self.erase_eof();
                Ok(())
            }
            Key::ToggleInsert => {
                self.insert = !self.insert;
                Ok(())
            }
            Key::Tab
            | Key::BackTab
            | Key::Up
            | Key::Down
            | Key::Left
            | Key::Right
            | Key::Home
            | Key::End
            | Key::MoveTo { .. } => {
                self.move_cursor(key);
                Ok(())
            }
            Key::Reset | Key::Attention | Key::SystemRequest => Ok(()),
        };
        if let Err(error) = result {
            self.error = Some(error);
        }
    }

    /// Clears an operator error (and puts back the line a host error message covered).
    fn reset(&mut self) {
        if let Some((row, bytes)) = self.saved_error_line.take() {
            self.screen.set_row_bytes(row, &bytes);
        }
        self.error = None;
        self.insert = false;
    }

    /// Sends the input the pending read command asks for, then waits for the host.
    pub(super) fn send_aid(&mut self, aid: Aid) {
        let Some(kind) = self.pending_read.take() else {
            return;
        };
        self.send_fields(kind, Some(aid));
        self.lock();
    }

    /// The input field the cursor is in. `None` at a protected position: an editing key does
    /// nothing there, with no operator error to Reset (R-KEY-8, D18; a real 5250 inhibits the
    /// keyboard, IBM 5250 Functions Reference, operator error 0005). Tab still moves on.
    /// Gives the field's index in the format table and a copy of it.
    fn input_field_at_cursor(&self) -> Option<(usize, Field)> {
        let index = self.screen.field_at(self.screen.cursor)?;
        self.screen
            .field(index)
            .filter(|f| f.is_input())
            .map(|f| (index, *f))
    }

    fn mark_modified(&mut self, index: usize) {
        if let Some(field) = self.screen.field_mut(index) {
            field.modified = true;
        }
    }

    fn type_char(&mut self, c: char) -> Result<(), OperatorError> {
        let Some((index, field)) = self.input_field_at_cursor() else {
            return Ok(());
        };
        let format = field.format;
        let shift = format.shift();
        if shift == Shift::SignedNumeric && matches!(c, '+' | '-') {
            return self.field_exit(if c == '-' { Sign::Minus } else { Sign::Plus });
        }
        let cursor = self.screen.cursor;
        let data_end = field.data_end();
        if cursor >= data_end {
            return Err(OperatorError::SignPosition);
        }
        let c = if format.monocase() {
            single_uppercase(c)
        } else {
            c
        };
        let allowed = match shift {
            Shift::AlphaOnly => c.is_alphabetic() || matches!(c, ',' | '.' | '-' | ' '),
            Shift::NumericOnly => c.is_ascii_digit() || matches!(c, '+' | ',' | '.' | '-' | ' '),
            Shift::DigitsOnly | Shift::SignedNumeric => c.is_ascii_digit(),
            Shift::Io => false,
            Shift::AlphaShift | Shift::NumericShift | Shift::Katakana => true,
        };
        if !allowed {
            return Err(match shift {
                Shift::AlphaOnly => OperatorError::AlphaOnly,
                Shift::NumericOnly => OperatorError::NumericOnly,
                Shift::Io => OperatorError::NotKeyboardInput,
                Shift::DigitsOnly
                | Shift::SignedNumeric
                | Shift::AlphaShift
                | Shift::NumericShift
                | Shift::Katakana => OperatorError::DigitsOnly,
            });
        }
        let byte = self
            .code_page()
            .to_byte(c)
            .ok_or(OperatorError::NotInCodePage)?;
        if self.insert {
            // Only nulls (and blanks) at the end of the field make room.
            if !matches!(self.screen.get(data_end - 1), 0 | BLANK) {
                return Err(OperatorError::NoRoomToInsert);
            }
            for address in (cursor + 1..data_end).rev() {
                let previous = self.screen.get(address - 1);
                self.screen.put(address, previous);
            }
        }
        self.screen.put(cursor, byte);
        self.mark_modified(index);
        if cursor + 1 < data_end {
            self.screen.cursor = cursor + 1;
        } else if format.auto_enter() {
            self.send_aid(Aid::ENTER);
        } else if !format.exit_required() {
            self.advance_from(field.start);
        }
        Ok(())
    }

    /// Field Exit, Field+ and Field-: nulls the rest of the field, adjusts it, sets the sign,
    /// and goes to the next field (or sends Enter in an auto-enter field).
    fn field_exit(&mut self, sign: Sign) -> Result<(), OperatorError> {
        let Some((index, field)) = self.input_field_at_cursor() else {
            return Ok(());
        };
        let shift = field.format.shift();
        if sign == Sign::Minus && !matches!(shift, Shift::NumericOnly | Shift::SignedNumeric) {
            return Err(OperatorError::FieldMinusNotAllowed);
        }
        let data_end = field.data_end();
        let cursor = self.screen.cursor;
        if cursor < data_end {
            self.screen.fill(cursor, data_end - 1, 0);
        }
        let fill = match field.format.adjust() {
            Adjust::RightZeroFill => Some(ZERO),
            Adjust::RightBlankFill => Some(BLANK),
            Adjust::None | Adjust::MandatoryFill => {
                (shift == Shift::SignedNumeric).then_some(BLANK)
            }
        };
        if let Some(fill) = fill {
            self.right_adjust(field.start, data_end, fill);
        }
        if shift == Shift::SignedNumeric && field.len > 0 {
            let byte = match sign {
                Sign::Plus => BLANK,
                Sign::Minus => MINUS,
            };
            self.screen.put(data_end, byte);
        }
        if shift == Shift::NumericOnly && sign == Sign::Minus && data_end > field.start {
            // Field- in a numeric-only field makes the last digit negative.
            if let Some(negative) = negative_zone(self.screen.get(data_end - 1)) {
                self.screen.put(data_end - 1, negative);
            }
        }
        self.mark_modified(index);
        if field.format.auto_enter() {
            self.send_aid(Aid::ENTER);
        } else {
            self.advance_from(field.start);
        }
        Ok(())
    }

    /// Moves the text in `start..end` to the right, without nulls and trailing blanks, and
    /// fills in front of it.
    fn right_adjust(&mut self, start: usize, end: usize, fill: u8) {
        let mut text: Vec<u8> = (start..end)
            .map(|a| self.screen.get(a))
            .filter(|&b| b != 0)
            .collect();
        while text.last() == Some(&BLANK) {
            text.pop();
        }
        let pad = (end - start).saturating_sub(text.len());
        for (i, byte) in std::iter::repeat_n(fill, pad).chain(text).enumerate() {
            self.screen.put(start + i, byte);
        }
    }

    fn delete(&mut self) -> Result<(), OperatorError> {
        let Some((index, field)) = self.input_field_at_cursor() else {
            return Ok(());
        };
        let cursor = self.screen.cursor;
        let data_end = field.data_end();
        if cursor >= data_end {
            return Err(OperatorError::SignPosition);
        }
        for address in cursor..data_end - 1 {
            let next = self.screen.get(address + 1);
            self.screen.put(address, next);
        }
        self.screen.put(data_end - 1, 0);
        self.mark_modified(index);
        Ok(())
    }

    fn backspace(&mut self) -> Result<(), OperatorError> {
        let cursor = self.screen.cursor;
        match self.input_field_at_cursor().map(|(_, f)| f) {
            Some(field) if cursor > field.start => {
                self.screen.cursor = cursor - 1;
                self.delete()
            }
            _ => {
                self.screen.cursor = self.screen.previous(cursor);
                Ok(())
            }
        }
    }

    fn erase_eof(&mut self) {
        let Some((index, field)) = self.input_field_at_cursor() else {
            return;
        };
        if self.screen.cursor < field.data_end() {
            self.screen
                .fill(self.screen.cursor, field.data_end() - 1, 0);
        }
        self.mark_modified(index);
    }

    /// The cursor to the next input field after the one starting at `start`; it stays if there
    /// is none.
    fn advance_from(&mut self, start: usize) {
        if let Some(next) = self.screen.next_input_field(start) {
            self.screen.cursor = next.start;
        }
    }

    fn move_cursor(&mut self, key: Key) {
        let screen = &self.screen;
        let cursor = screen.cursor;
        let positions = screen.buffer().len();
        let columns = usize::from(screen.size().columns());
        let current = self.input_field_at_cursor().map(|(_, f)| f);
        let to = match key {
            Key::Tab => screen.next_input_field(cursor).map_or(cursor, |f| f.start),
            Key::BackTab => match current {
                Some(f) if cursor != f.start => f.start,
                _ => screen
                    .previous_input_field(cursor)
                    .map_or(cursor, |f| f.start),
            },
            Key::Up => (cursor + positions - columns) % positions,
            Key::Down => (cursor + columns) % positions,
            Key::Left => screen.previous(cursor),
            Key::Right => screen.next(cursor),
            Key::Home => screen.home(),
            Key::End => current.map_or(cursor, |f| end_of_text(screen.field_bytes(&f), &f)),
            Key::MoveTo { row, column } if row < positions / columns && column < columns => {
                row * columns + column
            }
            Key::MoveTo { .. }
            | Key::Char(_)
            | Key::Aid(_)
            | Key::FieldExit
            | Key::FieldPlus
            | Key::FieldMinus
            | Key::Backspace
            | Key::Delete
            | Key::ToggleInsert
            | Key::EraseEof
            | Key::Reset
            | Key::Attention
            | Key::SystemRequest => cursor,
        };
        self.screen.cursor = to;
    }
}

/// Where End puts the cursor in `field` (whose bytes are `bytes`): after the last character
/// that is not a null or blank, but not past the last position the operator can type in. A
/// signed numeric field of one position has none, and End stays on its sign position.
fn end_of_text(bytes: &[u8], field: &Field) -> usize {
    let typeable = field.data_end() - field.start;
    let used = bytes
        .get(..typeable)
        .unwrap_or(bytes)
        .iter()
        .rposition(|&b| !matches!(b, 0 | BLANK))
        .map_or(0, |i| i + 1);
    field.start + used.min(typeable.saturating_sub(1))
}

/// The upper case of a letter, when it is a single character (not "ß" → "SS").
fn single_uppercase(c: char) -> char {
    let mut upper = c.to_uppercase();
    match (upper.next(), upper.next()) {
        (Some(u), None) => u,
        _ => c,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn function_keys_have_their_aid_codes() {
        assert_eq!(Aid::function(1).map(Aid::code), Some(0x31));
        assert_eq!(Aid::function(12).map(Aid::code), Some(0x3C));
        assert_eq!(Aid::function(13).map(Aid::code), Some(0xB1));
        assert_eq!(Aid::function(24).map(Aid::code), Some(0xBC));
        assert_eq!(Aid::function(0), None);
        assert_eq!(Aid::function(25), None);
    }

    #[test]
    fn monocase_keeps_letters_without_a_single_upper_case() {
        assert_eq!(single_uppercase('a'), 'A');
        assert_eq!(single_uppercase('æ'), 'Æ');
        assert_eq!(single_uppercase('ß'), 'ß');
        assert_eq!(single_uppercase('1'), '1');
    }
}
