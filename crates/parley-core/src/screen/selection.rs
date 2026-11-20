//! GUI selection fields and scroll bars (R-DS-11, R-DS-12, D38, D57; SC30-3533-04 §15.6.13.5
//! "DEFINE SELECTION FIELD", §15.6.13.7 "DEFINE SCROLL BAR FIELD"). Both are input fields the
//! host defines with Write to Display Structured Field instead of Start of Field: they are kept
//! with the format table, cleared with it, and sent back by the read commands in address order
//! with the other fields.
//!
//! A selection field's choices are written into the screen, so reading, copying and finding text
//! work as before: each choice is a leading attribute (its state's colour), an indicator (`/`
//! when selected, `_` when not) for radio buttons and check boxes, a blank on each side for a
//! push button, its text, and a normal attribute. The manual's exact geometry per indicator was
//! not at hand; this is Parley's reading (D57). The UI draws the widgets over these positions.

use super::{NORMAL, Rectangle, Screen};

/// EBCDIC `/` and `_`, the same in every code page Parley has: a choice's indicator.
const SELECTED_MARK: u8 = 0x61;
const EMPTY_MARK: u8 = 0x6D;

/// The type of selection field (§15.6.13.5, major structure byte 8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionKind {
    MenuBar,
    /// Radio buttons.
    SingleChoice,
    /// Check boxes.
    MultipleChoice,
    SingleList,
    MultipleList,
    SinglePullDown,
    MultiplePullDown,
    PushButtons,
    /// Push buttons in a pull-down menu.
    PullDownButtons,
}

impl SelectionKind {
    pub const fn from_code(code: u8) -> Option<Self> {
        Some(match code {
            0x01 => Self::MenuBar,
            0x11 => Self::SingleChoice,
            0x12 => Self::MultipleChoice,
            0x21 => Self::SingleList,
            0x22 => Self::MultipleList,
            0x31 => Self::SinglePullDown,
            0x32 => Self::MultiplePullDown,
            0x41 => Self::PushButtons,
            0x51 => Self::PullDownButtons,
            _ => return None,
        })
    }

    /// More than one choice may be selected (check boxes); the others take one.
    pub const fn multiple(self) -> bool {
        match self {
            Self::MultipleChoice | Self::MultipleList | Self::MultiplePullDown => true,
            Self::MenuBar
            | Self::SingleChoice
            | Self::SingleList
            | Self::SinglePullDown
            | Self::PushButtons
            | Self::PullDownButtons => false,
        }
    }

    /// Enter on one of its choices selects it (a menu bar or push buttons).
    pub const fn pressed(self) -> bool {
        match self {
            Self::MenuBar | Self::PushButtons | Self::PullDownButtons => true,
            Self::SingleChoice
            | Self::MultipleChoice
            | Self::SingleList
            | Self::MultipleList
            | Self::SinglePullDown
            | Self::MultiplePullDown => false,
        }
    }
}

/// What is drawn with each choice (the GUI device characteristics byte, bits 0-3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Indicator {
    /// A radio button or check box left of the choice.
    Mark,
    /// A push button around the choice.
    Button,
    None,
}

impl Indicator {
    pub const fn from_characteristics(byte: u8) -> Self {
        match byte >> 4 {
            0 => Self::Mark,
            2..=6 => Self::Button,
            _ => Self::None,
        }
    }

    /// Positions before the text: the mark and a blank, or a push button's blank.
    const fn before(self) -> usize {
        match self {
            Self::Mark => 2,
            Self::Button => 1,
            Self::None => 0,
        }
    }

    const fn after(self) -> usize {
        match self {
            Self::Button => 1,
            Self::Mark | Self::None => 0,
        }
    }
}

/// A choice's state (choice text flag byte 1, bits 0-1). The sources disagree; Parley reads
/// B'10' as selected and B'11' as not available, as IBM i's own menu bar bytes show (D57).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChoiceState {
    Available,
    Selected,
    Unavailable,
}

impl ChoiceState {
    pub const fn from_flags(flags: u8) -> Self {
        match flags & 0xC0 {
            0x80 => Self::Selected,
            0xC0 => Self::Unavailable,
            _ => Self::Available,
        }
    }
}

/// A cursor key, for the Roll AIDs a choice may send instead of moving (flag byte 2, bits 1-4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choice {
    /// EBCDIC.
    pub text: Vec<u8>,
    pub state: ChoiceState,
    /// The mnemonic's index in `text`.
    pub mnemonic: Option<usize>,
    /// The AID sent when it is selected.
    pub aid: Option<u8>,
    /// A menu bar choice that starts a new row.
    pub new_row: bool,
    /// Flag byte 2 as sent: the cursor and Roll AID bits.
    pub flags: u8,
    /// Its leading attribute's position, set when the field is laid out.
    pub address: usize,
}

impl Choice {
    /// The Roll AID a cursor key sends on this choice, if it says so (X'F4' Roll Down for Up,
    /// X'F5' Roll Up for Down, X'D9' Roll Left, X'DA' Roll Right).
    pub const fn roll(&self, direction: Direction) -> Option<u8> {
        let (bit, aid) = match direction {
            Direction::Up => (0x40, 0xF4),
            Direction::Down => (0x20, 0xF5),
            Direction::Left => (0x10, 0xD9),
            Direction::Right => (0x08, 0xDA),
        };
        if self.flags & bit != 0 {
            Some(aid)
        } else {
            None
        }
    }

    pub const fn takes_cursor(&self) -> bool {
        self.flags & 0x80 == 0 && !matches!(self.state, ChoiceState::Unavailable)
    }
}

/// The colour attributes of a choice in each state (the presentation minor structure).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChoiceColors {
    pub available: u8,
    pub selected: u8,
    pub unavailable: u8,
}

impl Default for ChoiceColors {
    /// Green, white and blue.
    fn default() -> Self {
        Self {
            available: NORMAL,
            selected: 0x22,
            unavailable: 0x3A,
        }
    }
}

/// A selection field.
#[derive(Debug, Clone, PartialEq, Eq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "independent flags of the Define Selection Field major structure"
)]
pub struct SelectionField {
    /// Where the host defined it: the first choice's leading attribute.
    pub start: usize,
    pub kind: SelectionKind,
    pub indicator: Indicator,
    /// Selecting a choice sends it to the host (flag byte 1, bits 4-5).
    pub auto_enter: bool,
    /// Selected choices go back to available when the keyboard unlocks (flag byte 3, bit 0).
    pub reset_on_unlock: bool,
    /// The cursor keys may not leave the field (flag byte 2, bit 5).
    pub keeps_cursor: bool,
    pub modified: bool,
    /// Text positions per choice.
    pub text_size: usize,
    pub columns: usize,
    /// Blanks between choices.
    pub padding: usize,
    pub colors: ChoiceColors,
    pub choices: Vec<Choice>,
}

impl SelectionField {
    /// Positions per choice: two attributes, the indicator and the text.
    pub const fn choice_width(&self) -> usize {
        2 + self.indicator.before() + self.text_size + self.indicator.after()
    }

    /// The first text position of choice `i`.
    pub fn text_start(&self, i: usize) -> Option<usize> {
        self.choices
            .get(i)
            .map(|c| c.address + 1 + self.indicator.before())
    }

    /// The choice whose positions hold `address`.
    pub fn choice_at(&self, address: usize) -> Option<usize> {
        let width = self.choice_width();
        self.choices
            .iter()
            .position(|c| address >= c.address && address < c.address + width)
    }

    pub fn selected(&self) -> impl Iterator<Item = usize> + '_ {
        self.choices
            .iter()
            .enumerate()
            .filter(|(_, c)| c.state == ChoiceState::Selected)
            .map(|(i, _)| i)
    }

    /// What the field sends the host (§15.6.13.5 as IBM i's DSM programs read it): one choice
    /// as its 1-based number plus X'1F' in two bytes (X'0000' for none); several as one byte
    /// a choice, X'F1' selected and X'00' not, in the order they were defined.
    pub fn reply(&self) -> Vec<u8> {
        if self.kind.multiple() {
            return self
                .choices
                .iter()
                .map(|c| {
                    if c.state == ChoiceState::Selected {
                        0xF1
                    } else {
                        0x00
                    }
                })
                .collect();
        }
        let number = self
            .selected()
            .next()
            .and_then(|i| u16::try_from(i + 1 + 0x1F).ok())
            .unwrap_or(0);
        number.to_be_bytes().to_vec()
    }

    /// Places each choice: `columns` to a row, or for a menu bar one row with a new one where a
    /// choice says so. `None` if a choice would run off its row or the screen.
    fn lay_out(&mut self, screen_columns: usize, positions: usize) -> Option<()> {
        let width = self.choice_width();
        let (first_row, first_column) = (self.start / screen_columns, self.start % screen_columns);
        let (mut row, mut column) = (0, 0);
        for (i, choice) in self.choices.iter_mut().enumerate() {
            if self.kind == SelectionKind::MenuBar {
                if choice.new_row && i > 0 {
                    row += 1;
                    column = 0;
                }
            } else {
                row = i / self.columns.max(1);
                column = i % self.columns.max(1);
            }
            let left = first_column + column * (width + self.padding);
            let address = (first_row + row) * screen_columns + left;
            if left + width > screen_columns || address + width > positions {
                return None;
            }
            choice.address = address;
            if self.kind == SelectionKind::MenuBar {
                column += 1;
            }
        }
        Some(())
    }

    /// The positions the field takes on the screen, row by row.
    fn spans(&self) -> impl Iterator<Item = (usize, usize)> + '_ {
        let width = self.choice_width();
        self.choices.iter().map(move |c| (c.address, width))
    }
}

/// A scroll bar (§15.6.13.7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScrollBar {
    pub start: usize,
    pub horizontal: bool,
    /// Rows (or columns) there are to scroll through.
    pub total: u32,
    /// The first one shown, from 0.
    pub position: u32,
    /// Rows (or columns) the bar takes on the screen.
    pub size: usize,
    /// A click or drag puts the cursor on the bar (flag bit 1).
    pub move_cursor: bool,
    pub modified: bool,
}

impl ScrollBar {
    /// The farthest the slider goes: the last screenful.
    pub fn last_position(&self) -> u32 {
        self.total
            .saturating_sub(u32::try_from(self.size).unwrap_or(u32::MAX))
    }

    /// What it sends the host: the slider's position in four bytes. Not in any source at hand
    /// (D57); Parley sends it as the host sent it.
    pub fn reply(&self) -> Vec<u8> {
        self.position.to_be_bytes().to_vec()
    }

    /// The positions it takes, as a rectangle of 0-based rows and columns.
    pub fn area(&self, screen_columns: usize) -> Rectangle {
        let (top, left) = (self.start / screen_columns, self.start % screen_columns);
        let last = self.size.saturating_sub(1);
        if self.horizontal {
            Rectangle {
                top,
                left,
                bottom: top,
                right: left + last,
            }
        } else {
            Rectangle {
                top,
                left,
                bottom: top + last,
                right: left,
            }
        }
    }
}

/// A GUI input field's place, whether it was changed, and the bytes it sends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiInput {
    pub start: usize,
    pub modified: bool,
    pub data: Vec<u8>,
}

impl Screen {
    pub fn selection_fields(&self) -> &[SelectionField] {
        &self.selections
    }

    pub fn scroll_bars(&self) -> &[ScrollBar] {
        &self.scroll_bars
    }

    /// Defines a selection field and writes its choices; one at the same place is replaced.
    /// `None` when it does not fit on the screen.
    pub fn define_selection(&mut self, mut field: SelectionField) -> Option<()> {
        field.lay_out(usize::from(self.size.columns()), self.buffer.len())?;
        self.remove_selection(field.start);
        self.draw_selection(&field);
        self.selections.push(field);
        self.selections.sort_by_key(|f| f.start);
        Some(())
    }

    /// Removes the selection field defined at `start` and blanks its positions (§15.6.13.6).
    pub fn remove_selection(&mut self, start: usize) -> bool {
        let Some(i) = self.selections.iter().position(|f| f.start == start) else {
            return false;
        };
        let field = self.selections.remove(i);
        for (address, width) in field.spans() {
            self.fill(address, address + width - 1, 0);
        }
        true
    }

    /// Selects choice `choice` of field `field` (the other one goes, unless the field takes
    /// several), or takes a check box's selection away. False if it cannot be selected.
    pub fn toggle_choice(&mut self, field: usize, choice: usize) -> bool {
        let Some(f) = self.selections.get_mut(field) else {
            return false;
        };
        let multiple = f.kind.multiple();
        let Some(state) = f.choices.get(choice).map(|c| c.state) else {
            return false;
        };
        match (state, multiple) {
            (ChoiceState::Unavailable, _) => return false,
            (ChoiceState::Selected, true) => f.choices[choice].state = ChoiceState::Available,
            (ChoiceState::Selected | ChoiceState::Available, false) => {
                for c in &mut f.choices {
                    if c.state == ChoiceState::Selected {
                        c.state = ChoiceState::Available;
                    }
                }
                f.choices[choice].state = ChoiceState::Selected;
            }
            (ChoiceState::Available, true) => f.choices[choice].state = ChoiceState::Selected,
        }
        f.modified = true;
        let field = f.clone();
        self.draw_selection(&field);
        true
    }

    /// When the keyboard unlocks: selected choices of fields that ask for it go back to
    /// available (flag byte 3, bit 0).
    pub fn reset_selections(&mut self) {
        let mut changed = Vec::new();
        for f in self.selections.iter_mut().filter(|f| f.reset_on_unlock) {
            for c in &mut f.choices {
                if c.state == ChoiceState::Selected {
                    c.state = ChoiceState::Available;
                }
            }
            changed.push(f.clone());
        }
        for f in &changed {
            self.draw_selection(f);
        }
    }

    fn draw_selection(&mut self, field: &SelectionField) {
        for choice in &field.choices {
            let attribute = match choice.state {
                ChoiceState::Available => field.colors.available,
                ChoiceState::Selected => field.colors.selected,
                ChoiceState::Unavailable => field.colors.unavailable,
            };
            let mut bytes = vec![attribute];
            match field.indicator {
                Indicator::Mark => bytes.extend([
                    if choice.state == ChoiceState::Selected {
                        SELECTED_MARK
                    } else {
                        EMPTY_MARK
                    },
                    0x40,
                ]),
                Indicator::Button => bytes.push(0x40),
                Indicator::None => {}
            }
            let mut text = choice.text.clone();
            text.resize(field.text_size, 0x40);
            bytes.extend(text);
            bytes.extend(std::iter::repeat_n(0x40, field.indicator.after()));
            bytes.push(NORMAL);
            for (k, b) in bytes.into_iter().enumerate() {
                self.put(choice.address + k, b);
            }
        }
    }

    /// Defines a scroll bar; one at the same place is replaced. `None` when it does not fit.
    pub fn define_scroll_bar(&mut self, bar: ScrollBar) -> Option<()> {
        let columns = usize::from(self.size.columns());
        let area = bar.area(columns);
        if bar.size == 0 || area.bottom >= usize::from(self.size.rows()) || area.right >= columns {
            return None;
        }
        self.remove_scroll_bar(bar.start);
        self.scroll_bars.push(bar);
        self.scroll_bars.sort_by_key(|b| b.start);
        Some(())
    }

    /// Removes the scroll bar defined at `start` (§15.6.13.8).
    pub fn remove_scroll_bar(&mut self, start: usize) -> bool {
        let before = self.scroll_bars.len();
        self.scroll_bars.retain(|b| b.start != start);
        self.scroll_bars.len() != before
    }

    /// Moves scroll bar `i`'s slider to `position`, kept within the bar; false if it did not move.
    pub fn set_scroll_position(&mut self, i: usize, position: u32) -> bool {
        let Some(bar) = self.scroll_bars.get_mut(i) else {
            return false;
        };
        let position = position.min(bar.last_position());
        if position == bar.position {
            return false;
        }
        bar.position = position;
        bar.modified = true;
        true
    }

    /// Every selection field and scroll bar, as input fields for the read commands.
    pub fn gui_inputs(&self) -> Vec<GuiInput> {
        let fields = self.selections.iter().map(|f| GuiInput {
            start: f.start,
            modified: f.modified,
            data: f.reply(),
        });
        let bars = self.scroll_bars.iter().map(|b| GuiInput {
            start: b.start,
            modified: b.modified,
            data: b.reply(),
        });
        fields.chain(bars).collect()
    }

    /// Forgets the selection fields and scroll bars in `area` (a window being removed).
    pub(super) fn forget_gui_fields_in(&mut self, area: Rectangle) {
        let columns = usize::from(self.size.columns());
        let inside = |start: usize| area.contains(start / columns, start % columns);
        self.selections.retain(|f| !inside(f.start));
        self.scroll_bars.retain(|b| !inside(b.start));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CodePage;
    use crate::screen::ScreenSize;

    const PAGE: CodePage = CodePage::Ccsid37;

    fn ebcdic(text: &str) -> Vec<u8> {
        text.chars()
            .map(|c| PAGE.to_byte(c).unwrap_or(0x40))
            .collect()
    }

    fn choice(text: &str, state: ChoiceState) -> Choice {
        Choice {
            text: ebcdic(text),
            state,
            mnemonic: None,
            aid: None,
            new_row: false,
            flags: 0,
            address: 0,
        }
    }

    fn field(kind: SelectionKind, indicator: Indicator, choices: Vec<Choice>) -> SelectionField {
        SelectionField {
            start: 2 * 80 + 4,
            kind,
            indicator,
            auto_enter: false,
            reset_on_unlock: false,
            keeps_cursor: false,
            modified: false,
            text_size: 5,
            columns: 1,
            padding: 0,
            colors: ChoiceColors::default(),
            choices,
        }
    }

    fn line(screen: &Screen, r: usize) -> String {
        screen
            .text(PAGE)
            .lines()
            .nth(r)
            .unwrap_or_default()
            .to_owned()
    }

    #[test]
    fn radio_buttons_are_written_one_a_row_with_their_marks_r_ds_11() {
        let mut screen = Screen::new(ScreenSize::Standard);
        let f = field(
            SelectionKind::SingleChoice,
            Indicator::Mark,
            vec![
                choice("Æble", ChoiceState::Available),
                choice("Pære", ChoiceState::Selected),
            ],
        );
        assert!(screen.define_selection(f).is_some());
        assert_eq!(line(&screen, 2).trim_end(), "     _ Æble");
        assert_eq!(line(&screen, 3).trim_end(), "     / Pære");
        assert_eq!(screen.selection_fields()[0].reply(), [0x00, 0x21]);
        assert_eq!(screen.selection_fields()[0].choice_at(3 * 80 + 9), Some(1));
        assert_eq!(screen.selection_fields()[0].text_start(1), Some(3 * 80 + 7));
    }

    #[test]
    fn selecting_a_radio_button_takes_the_other_away_and_sets_the_mdt_r_ds_11() {
        let mut screen = Screen::new(ScreenSize::Standard);
        let f = field(
            SelectionKind::SingleChoice,
            Indicator::Mark,
            vec![
                choice("One", ChoiceState::Selected),
                choice("Two", ChoiceState::Available),
                choice("Three", ChoiceState::Unavailable),
            ],
        );
        screen.define_selection(f);
        assert!(screen.toggle_choice(0, 1));
        assert!(!screen.toggle_choice(0, 2), "not available");
        let f = &screen.selection_fields()[0];
        assert!(f.modified);
        assert_eq!(f.selected().collect::<Vec<_>>(), [1]);
        assert_eq!(f.reply(), [0x00, 0x21]);
        assert_eq!(line(&screen, 2).trim_end(), "     _ One");
    }

    #[test]
    fn check_boxes_toggle_and_send_a_byte_each_r_ds_11() {
        let mut screen = Screen::new(ScreenSize::Standard);
        let mut f = field(
            SelectionKind::MultipleChoice,
            Indicator::Mark,
            vec![
                choice("A", ChoiceState::Available),
                choice("B", ChoiceState::Selected),
                choice("C", ChoiceState::Available),
            ],
        );
        f.columns = 3;
        f.padding = 1;
        screen.define_selection(f);
        assert_eq!(line(&screen, 2).trim_end(), "     _ A       / B       _ C");
        screen.toggle_choice(0, 0);
        screen.toggle_choice(0, 1);
        assert_eq!(screen.selection_fields()[0].reply(), [0xF1, 0x00, 0x00]);
    }

    #[test]
    fn a_menu_bar_lies_in_a_row_and_starts_another_when_asked_r_ds_11() {
        let mut screen = Screen::new(ScreenSize::Standard);
        let mut third = choice("Help", ChoiceState::Available);
        third.new_row = true;
        let mut f = field(
            SelectionKind::MenuBar,
            Indicator::None,
            vec![
                choice("File", ChoiceState::Available),
                choice("Edit", ChoiceState::Available),
                third,
            ],
        );
        f.start = 1;
        f.columns = 3;
        f.padding = 1;
        screen.define_selection(f);
        assert_eq!(line(&screen, 0).trim_end(), "  File    Edit");
        assert_eq!(line(&screen, 1).trim_end(), "  Help");
        assert_eq!(screen.selection_fields()[0].reply(), [0x00, 0x00], "none");
    }

    #[test]
    fn a_selection_field_that_runs_off_the_screen_is_refused_r_ds_11() {
        let mut screen = Screen::new(ScreenSize::Standard);
        let mut f = field(
            SelectionKind::PushButtons,
            Indicator::Button,
            vec![choice("Ok", ChoiceState::Available)],
        );
        f.start = 77;
        assert!(screen.define_selection(f.clone()).is_none(), "past the row");
        f.start = 23 * 80;
        f.choices.push(choice("No", ChoiceState::Available));
        assert!(screen.define_selection(f).is_none(), "past the last row");
        assert_eq!(screen.selection_fields(), []);
    }

    #[test]
    fn removing_a_selection_field_blanks_it_and_clearing_the_format_table_forgets_it_r_ds_11() {
        let mut screen = Screen::new(ScreenSize::Standard);
        let f = field(
            SelectionKind::PushButtons,
            Indicator::Button,
            vec![choice("Ok", ChoiceState::Available)],
        );
        screen.define_selection(f.clone());
        assert_eq!(line(&screen, 2).trim_end(), "      Ok");
        assert!(!screen.remove_selection(0));
        assert!(screen.remove_selection(f.start));
        assert_eq!(line(&screen, 2).trim_end(), "");
        screen.define_selection(f);
        screen.clear_fields();
        assert_eq!(screen.selection_fields(), []);
    }

    #[test]
    fn selected_choices_go_back_when_the_field_asks_r_ds_11() {
        let mut screen = Screen::new(ScreenSize::Standard);
        let mut f = field(
            SelectionKind::PushButtons,
            Indicator::Button,
            vec![choice("Ok", ChoiceState::Selected)],
        );
        f.reset_on_unlock = true;
        screen.define_selection(f);
        screen.reset_selections();
        assert_eq!(screen.selection_fields()[0].selected().count(), 0);
    }

    fn bar(start: usize, horizontal: bool) -> ScrollBar {
        ScrollBar {
            start,
            horizontal,
            total: 100,
            position: 0,
            size: 10,
            move_cursor: false,
            modified: false,
        }
    }

    #[test]
    fn a_scroll_bar_fits_or_is_refused_and_its_slider_stays_inside_r_ds_12() {
        let mut screen = Screen::new(ScreenSize::Standard);
        assert!(screen.define_scroll_bar(bar(15 * 80 + 79, false)).is_none());
        assert!(screen.define_scroll_bar(bar(5 * 80 + 75, true)).is_none());
        assert!(screen.define_scroll_bar(bar(2 * 80 + 79, false)).is_some());
        assert!(screen.set_scroll_position(0, 500));
        assert_eq!(screen.scroll_bars()[0].position, 90, "the last screenful");
        assert!(!screen.set_scroll_position(0, 90));
        assert_eq!(screen.gui_inputs()[0].data, [0, 0, 0, 90]);
        assert!(screen.gui_inputs()[0].modified);
        assert!(screen.remove_scroll_bar(2 * 80 + 79));
        assert_eq!(screen.scroll_bars(), []);
    }
}
