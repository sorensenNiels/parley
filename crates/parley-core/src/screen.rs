//! The 5250 display (R-DS-2): a buffer of EBCDIC bytes, the format table of fields, and the cursor.
//!
//! Bytes X'20' to X'3F' in the buffer are screen attributes: each takes one position, shows as a
//! blank, and sets the colour and highlighting of the positions after it, up to the next attribute.

use serde::{Deserialize, Serialize};

use crate::CodePage;

/// The display sizes a 5250 session can use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ScreenSize {
    /// 24 rows of 80 columns: every 5250 display station supports it.
    #[default]
    Standard,
    /// 27 rows of 132 columns (for example device type 3477-FC).
    Wide,
}

impl ScreenSize {
    pub const fn rows(self) -> u16 {
        match self {
            Self::Standard => 24,
            Self::Wide => 27,
        }
    }

    pub const fn columns(self) -> u16 {
        match self {
            Self::Standard => 80,
            Self::Wide => 132,
        }
    }

    pub const fn positions(self) -> usize {
        self.rows() as usize * self.columns() as usize
    }
}

/// The attribute a screen starts with: green, no highlighting.
pub const NORMAL: u8 = 0x20;

pub(crate) const fn is_attribute(byte: u8) -> bool {
    byte >= 0x20 && byte <= 0x3F
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    Green,
    White,
    Red,
    Turquoise,
    Yellow,
    Pink,
    Blue,
}

/// How a position is shown, from the screen attribute in front of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "independent flags of one attribute byte"
)]
pub struct Style {
    pub color: Color,
    pub reverse: bool,
    pub underline: bool,
    pub blink: bool,
    pub column_separators: bool,
    /// Non-display: the text is there (for example a password) but is not shown.
    pub hidden: bool,
}

impl Style {
    /// The colour attributes of a 5250 colour display (IBM 3179-2 / 3477-FC).
    pub const fn from_attribute(attribute: u8) -> Self {
        let a = attribute & 0x1F;
        let bright = a & 0x02 != 0;
        let underline = a & 0x04 != 0;
        let (color, blink, column_separators) = match a & 0x18 {
            0x00 => (
                if bright { Color::White } else { Color::Green },
                false,
                false,
            ),
            0x08 => (Color::Red, bright, false),
            0x10 => (
                if bright {
                    Color::Yellow
                } else {
                    Color::Turquoise
                },
                false,
                !underline,
            ),
            _ => (if bright { Color::Blue } else { Color::Pink }, false, false),
        };
        let hidden = a & 0x07 == 0x07;
        Self {
            color,
            reverse: !hidden && a & 0x01 != 0,
            underline: !hidden && underline,
            blink,
            column_separators,
            hidden,
        }
    }
}

/// One position as the UI shows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cell {
    pub ch: char,
    pub style: Style,
}

/// The field format word (FFW) of an input field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldFormat(pub u16);

/// Which characters an input field accepts (FFW bits 5-7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shift {
    AlphaShift,
    AlphaOnly,
    NumericShift,
    NumericOnly,
    Katakana,
    DigitsOnly,
    /// Input from a feature such as a magnetic stripe reader, not the keyboard.
    Io,
    SignedNumeric,
}

/// How Field Exit fills a field (FFW bits 13-15).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Adjust {
    None,
    RightZeroFill,
    RightBlankFill,
    MandatoryFill,
}

impl FieldFormat {
    const fn bit(self, mask: u16) -> bool {
        self.0 & mask != 0
    }

    pub const fn bypass(self) -> bool {
        self.bit(0x2000)
    }

    pub const fn modified(self) -> bool {
        self.bit(0x0800)
    }

    pub const fn shift(self) -> Shift {
        match (self.0 >> 8) & 0x07 {
            0 => Shift::AlphaShift,
            1 => Shift::AlphaOnly,
            2 => Shift::NumericShift,
            3 => Shift::NumericOnly,
            4 => Shift::Katakana,
            5 => Shift::DigitsOnly,
            6 => Shift::Io,
            _ => Shift::SignedNumeric,
        }
    }

    pub const fn auto_enter(self) -> bool {
        self.bit(0x0080)
    }

    pub const fn exit_required(self) -> bool {
        self.bit(0x0040)
    }

    pub const fn monocase(self) -> bool {
        self.bit(0x0020)
    }

    pub const fn mandatory_enter(self) -> bool {
        self.bit(0x0008)
    }

    pub const fn adjust(self) -> Adjust {
        match self.0 & 0x07 {
            5 => Adjust::RightZeroFill,
            6 => Adjust::RightBlankFill,
            7 => Adjust::MandatoryFill,
            _ => Adjust::None,
        }
    }
}

/// An input field in the format table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Field {
    /// The buffer position of the first data byte (the attribute is the position before it).
    pub start: usize,
    pub len: usize,
    pub format: FieldFormat,
    /// The modified data tag (MDT): the field is sent to the host by Read MDT Fields.
    pub modified: bool,
}

impl Field {
    pub const fn end(&self) -> usize {
        self.start + self.len
    }

    pub const fn contains(&self, address: usize) -> bool {
        address >= self.start && address < self.end()
    }

    /// Positions the operator can type in: a signed numeric field keeps its last one for the sign.
    pub fn data_end(&self) -> usize {
        match self.format.shift() {
            Shift::SignedNumeric => self.end().saturating_sub(1).max(self.start),
            _ => self.end(),
        }
    }

    /// The operator can type in it.
    pub const fn is_input(&self) -> bool {
        !self.format.bypass()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Screen {
    size: ScreenSize,
    buffer: Vec<u8>,
    fields: Vec<Field>,
    /// The cursor's buffer position.
    pub cursor: usize,
}

impl Screen {
    pub fn new(size: ScreenSize) -> Self {
        Self {
            size,
            buffer: vec![0; size.positions()],
            fields: Vec::new(),
            cursor: 0,
        }
    }

    pub const fn size(&self) -> ScreenSize {
        self.size
    }

    pub fn buffer(&self) -> &[u8] {
        &self.buffer
    }

    pub fn fields(&self) -> &[Field] {
        &self.fields
    }

    /// Nulls everywhere, no fields, cursor home: what Clear Unit does.
    pub fn clear(&mut self, size: ScreenSize) {
        *self = Self::new(size);
    }

    pub fn clear_fields(&mut self) {
        self.fields.clear();
    }

    /// The buffer position of a 1-based row and column, if it is on the screen.
    pub fn address(&self, row: u8, column: u8) -> Option<usize> {
        let (row, column) = (u16::from(row), u16::from(column));
        if row == 0 || column == 0 || row > self.size.rows() || column > self.size.columns() {
            return None;
        }
        Some(usize::from(row - 1) * usize::from(self.size.columns()) + usize::from(column - 1))
    }

    /// The 0-based row and column of a buffer position.
    pub fn row_column(&self, address: usize) -> (usize, usize) {
        let columns = usize::from(self.size.columns());
        (address / columns, address % columns)
    }

    /// The position after `address`, wrapping from the last position to the first.
    pub fn next(&self, address: usize) -> usize {
        (address + 1) % self.buffer.len()
    }

    pub fn previous(&self, address: usize) -> usize {
        address.checked_sub(1).unwrap_or(self.buffer.len() - 1)
    }

    pub fn get(&self, address: usize) -> u8 {
        self.buffer.get(address).copied().unwrap_or(0)
    }

    pub fn put(&mut self, address: usize, byte: u8) {
        if let Some(b) = self.buffer.get_mut(address) {
            *b = byte;
        }
    }

    pub fn fill(&mut self, from: usize, to_inclusive: usize, byte: u8) {
        let end = to_inclusive.min(self.buffer.len() - 1);
        if let Some(range) = self.buffer.get_mut(from..=end) {
            range.fill(byte);
        }
    }

    pub fn row_bytes(&self, row: usize) -> &[u8] {
        let columns = usize::from(self.size.columns());
        self.buffer
            .get(row * columns..(row + 1) * columns)
            .unwrap_or(&[])
    }

    pub fn set_row_bytes(&mut self, row: usize, bytes: &[u8]) {
        let columns = usize::from(self.size.columns());
        if let Some(target) = self.buffer.get_mut(row * columns..(row + 1) * columns) {
            let n = bytes.len().min(columns);
            target[..n].copy_from_slice(&bytes[..n]);
        }
    }

    /// Adds a field, or replaces the one starting at the same position. Fields that would run
    /// past the end of the screen are cut short there.
    pub fn add_field(&mut self, mut field: Field) {
        field.len = field.len.min(self.buffer.len().saturating_sub(field.start));
        match self.fields.binary_search_by_key(&field.start, |f| f.start) {
            Ok(i) => self.fields[i] = field,
            Err(i) => self.fields.insert(i, field),
        }
    }

    /// The index of the field holding `address`.
    pub fn field_at(&self, address: usize) -> Option<usize> {
        self.fields.iter().position(|f| f.contains(address))
    }

    pub fn field(&self, index: usize) -> Option<&Field> {
        self.fields.get(index)
    }

    pub fn field_mut(&mut self, index: usize) -> Option<&mut Field> {
        self.fields.get_mut(index)
    }

    pub fn field_bytes(&self, field: &Field) -> &[u8] {
        self.buffer.get(field.start..field.end()).unwrap_or(&[])
    }

    pub fn field_bytes_mut(&mut self, field: &Field) -> &mut [u8] {
        self.buffer
            .get_mut(field.start..field.end())
            .unwrap_or(&mut [])
    }

    /// The first input field after `address` (wrapping), or `None` if there is none.
    pub fn next_input_field(&self, address: usize) -> Option<&Field> {
        let inputs = || self.fields.iter().filter(|f| f.is_input());
        inputs()
            .find(|f| f.start > address)
            .or_else(|| inputs().next())
    }

    /// The last input field that starts before `address` (wrapping).
    pub fn previous_input_field(&self, address: usize) -> Option<&Field> {
        let inputs = || self.fields.iter().filter(|f| f.is_input());
        inputs()
            .rev()
            .find(|f| f.start < address)
            .or_else(|| inputs().next_back())
    }

    /// Where the cursor goes when the host does not say: the first input field, or row 1 column 1.
    pub fn home(&self) -> usize {
        self.fields
            .iter()
            .find(|f| f.is_input())
            .map_or(0, |f| f.start)
    }

    /// Every position with its character and style, row by row.
    ///
    /// An input field's attribute covers the field only: after its last position the style goes
    /// back to normal until the next attribute. IBM i does not always write an ending attribute
    /// after a field (its own sign-on screen does not), and its underline must not run on.
    ///
    /// A non-display input field (a password) is underlined, so the operator sees where to type;
    /// its contents stay hidden.
    pub fn cells(&self, code_page: CodePage) -> Vec<Cell> {
        let blank = |style| Cell { ch: ' ', style };
        let normal = Style::from_attribute(NORMAL);
        let mut field_ends = vec![false; self.buffer.len()];
        let mut in_input_field = vec![false; self.buffer.len()];
        for field in &self.fields {
            if let Some(end) = field_ends.get_mut(field.end()) {
                *end = true;
            }
            if field.is_input()
                && let Some(positions) = in_input_field.get_mut(field.start..field.end())
            {
                positions.fill(true);
            }
        }
        let mut style = normal;
        self.buffer
            .iter()
            .zip(field_ends.into_iter().zip(in_input_field))
            .map(|(&byte, (field_ended, input))| {
                if field_ended {
                    style = normal;
                }
                if is_attribute(byte) {
                    style = Style::from_attribute(byte);
                    // The attribute's own position is a plain blank.
                    return blank(Style {
                        reverse: false,
                        underline: false,
                        ..style
                    });
                }
                if style.hidden {
                    return blank(Style {
                        underline: input,
                        ..style
                    });
                }
                let ch = match byte {
                    // DUP shows as an overscored asterisk on a real display.
                    0x1C => '*',
                    _ => code_page.to_char(byte),
                };
                Cell { ch, style }
            })
            .collect()
    }

    /// The screen as text, one line per row: for copying and for tests.
    pub fn text(&self, code_page: CodePage) -> String {
        let columns = usize::from(self.size.columns());
        self.cells(code_page)
            .chunks(columns)
            .map(|row| row.iter().map(|c| c.ch).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_session_uses_the_standard_24_by_80_screen() {
        let size = ScreenSize::default();
        assert_eq!((size.rows(), size.columns()), (24, 80));
    }

    #[test]
    fn the_wide_screen_is_27_by_132() {
        assert_eq!(
            (ScreenSize::Wide.rows(), ScreenSize::Wide.columns()),
            (27, 132)
        );
    }

    #[test]
    fn addresses_are_one_based_and_checked() {
        let screen = Screen::new(ScreenSize::Standard);
        assert_eq!(screen.address(1, 1), Some(0));
        assert_eq!(screen.address(2, 3), Some(82));
        assert_eq!(screen.address(24, 80), Some(1919));
        assert_eq!(screen.address(0, 1), None);
        assert_eq!(screen.address(25, 1), None);
        assert_eq!(screen.address(1, 81), None);
        assert_eq!(screen.row_column(82), (1, 2));
    }

    #[test]
    fn colour_attributes_follow_the_3179_table() {
        let s = Style::from_attribute;
        assert_eq!(s(0x20).color, Color::Green);
        assert_eq!(s(0x22).color, Color::White);
        assert!(s(0x21).reverse);
        assert!(s(0x24).underline);
        assert_eq!(s(0x28).color, Color::Red);
        assert!(s(0x2A).blink);
        assert!(s(0x30).column_separators);
        assert_eq!(s(0x32).color, Color::Yellow);
        assert!(!s(0x34).column_separators && s(0x34).underline);
        assert_eq!(s(0x38).color, Color::Pink);
        assert_eq!(s(0x3A).color, Color::Blue);
        for hidden in [0x27, 0x2F, 0x37, 0x3F] {
            assert!(s(hidden).hidden && !s(hidden).reverse && !s(hidden).underline);
        }
    }

    #[test]
    fn attributes_show_as_blanks_and_style_what_follows() {
        let mut screen = Screen::new(ScreenSize::Standard);
        let page = CodePage::Ccsid37;
        screen.put(0, 0x22);
        screen.put(1, 0xC1);
        screen.put(2, 0x27);
        screen.put(3, 0xC2);
        let cells = screen.cells(page);
        assert_eq!(cells[0].ch, ' ');
        assert_eq!(
            cells[1],
            Cell {
                ch: 'A',
                style: Style::from_attribute(0x22)
            }
        );
        assert_eq!(cells[3].ch, ' ', "non-display hides the text");
        assert!(screen.text(page).starts_with(" A  "));
    }

    #[test]
    fn an_input_fields_underline_stops_at_the_end_of_the_field() {
        // IBM i's sign-on screen writes no ending attribute after its fields.
        let mut screen = Screen::new(ScreenSize::Standard);
        screen.put(9, 0x24);
        screen.add_field(Field {
            start: 10,
            len: 5,
            format: FieldFormat(0x4000),
            modified: false,
        });
        let cells = screen.cells(CodePage::Ccsid37);
        assert!(cells[10..15].iter().all(|c| c.style.underline));
        assert!(
            cells[15..].iter().all(|c| !c.style.underline),
            "nothing after the field is underlined"
        );
    }

    #[test]
    fn a_non_display_input_field_is_underlined_but_its_text_is_hidden() {
        let mut screen = Screen::new(ScreenSize::Standard);
        screen.put(9, 0x27);
        screen.add_field(Field {
            start: 10,
            len: 5,
            format: FieldFormat(0x4000),
            modified: false,
        });
        screen.put(10, 0xC1);
        let cells = screen.cells(CodePage::Ccsid37);
        assert!(
            cells[10..15]
                .iter()
                .all(|c| c.ch == ' ' && c.style.underline && c.style.hidden)
        );
        assert!(!cells[9].style.underline && !cells[15].style.underline);
    }

    #[test]
    fn non_display_text_outside_input_fields_has_no_underline() {
        let mut screen = Screen::new(ScreenSize::Standard);
        screen.put(9, 0x27);
        screen.put(10, 0xC1);
        assert!(!screen.cells(CodePage::Ccsid37)[10].style.underline);
    }

    #[test]
    fn an_attribute_written_after_a_field_still_applies() {
        let mut screen = Screen::new(ScreenSize::Standard);
        screen.put(9, 0x24);
        screen.add_field(Field {
            start: 10,
            len: 5,
            format: FieldFormat(0x4000),
            modified: false,
        });
        screen.put(15, 0x28);
        screen.put(16, 0xC1);
        assert_eq!(screen.cells(CodePage::Ccsid37)[16].style.color, Color::Red);
    }

    #[test]
    fn field_format_words_decode() {
        // Bypass off, MDT on, signed numeric; auto enter, monocase, right adjust zero fill.
        let f = FieldFormat(0x4F_A5);
        assert!(!f.bypass());
        assert!(f.modified());
        assert_eq!(f.shift(), Shift::SignedNumeric);
        assert!(f.auto_enter() && f.monocase() && !f.exit_required());
        assert_eq!(f.adjust(), Adjust::RightZeroFill);
        assert!(FieldFormat(0x6000).bypass());
    }

    #[test]
    fn fields_are_kept_in_order_and_replaced_by_start() {
        let mut screen = Screen::new(ScreenSize::Standard);
        let field = |start, len| Field {
            start,
            len,
            format: FieldFormat(0x4000),
            modified: false,
        };
        screen.add_field(field(100, 5));
        screen.add_field(field(10, 5));
        screen.add_field(field(100, 8));
        assert_eq!(
            screen
                .fields()
                .iter()
                .map(|f| (f.start, f.len))
                .collect::<Vec<_>>(),
            [(10, 5), (100, 8)]
        );
        assert_eq!(screen.next_input_field(10).map(|f| f.start), Some(100));
        assert_eq!(
            screen.next_input_field(100).map(|f| f.start),
            Some(10),
            "wraps"
        );
        assert_eq!(
            screen.previous_input_field(10).map(|f| f.start),
            Some(100),
            "wraps"
        );
        screen.add_field(field(1915, 10));
        assert_eq!(
            screen.fields().last().map(|f| f.len),
            Some(5),
            "cut at the screen's end"
        );
    }
}
