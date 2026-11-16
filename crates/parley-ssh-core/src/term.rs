//! The terminal: xterm emulation, sans-I/O (R-SSH-6, D43). Bytes from the host go in with [`Terminal::feed`];
//! the screen, the scrollback and the modes come out, and so do the replies the host asked for
//! ([`Terminal::take_replies`]). Control sequences follow ECMA-48, DEC STD 070 and xterm's ctlseqs; `vte`
//! parses them and [`perform`] carries them out.

mod cell;
mod perform;
mod sgr;
#[cfg(test)]
mod tests;

use std::collections::VecDeque;

pub use cell::{Cell, Color, Row, Style, Underline, Weight, Width};

/// How many lines the scrollback keeps unless told otherwise (R-SSH-8).
pub const DEFAULT_HISTORY: usize = 10_000;

/// Do the cursor keys or the keypad send their normal or their application sequences (DECCKM, DECKPAM)?
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum KeyMode {
    #[default]
    Normal,
    Application,
}

/// The cursor's shape (DECSCUSR).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CursorShape {
    #[default]
    Block,
    Underline,
    Bar,
}

/// The modes the host can set (SM/RM, DECSET/DECRST).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each is an independent terminal mode"
)]
pub struct Modes {
    pub cursor_keys: KeyMode,
    pub keypad: KeyMode,
    /// DECAWM: text that reaches the right edge goes on in the next line.
    pub auto_wrap: bool,
    /// DECOM: cursor addresses count from the top of the scrolling region.
    pub origin: bool,
    /// IRM: printed text pushes the rest of the line right.
    pub insert: bool,
    /// LNM: a line feed also returns the carriage.
    pub newline: bool,
    /// DECTCEM.
    pub cursor_visible: bool,
    pub cursor_shape: CursorShape,
    pub cursor_blink: bool,
    /// Mode 2004: pasted text is wrapped in `CSI 200~` and `CSI 201~` (R-SSH-9).
    pub bracketed_paste: bool,
    /// Mode 1004: the host is told when the window gains or loses focus.
    pub focus_events: bool,
}

impl Default for Modes {
    fn default() -> Self {
        Self {
            cursor_keys: KeyMode::Normal,
            keypad: KeyMode::Normal,
            auto_wrap: true,
            origin: false,
            insert: false,
            newline: false,
            cursor_visible: true,
            cursor_shape: CursorShape::Block,
            cursor_blink: true,
            bracketed_paste: false,
            focus_events: false,
        }
    }
}

/// The character sets G0 and G1 can hold (SCS, `ESC ( 0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Charset {
    #[default]
    Ascii,
    /// DEC Special Graphics: the line-drawing characters.
    DecSpecial,
}

/// Which screen is shown: the main one with its scrollback, or the alternate one full-screen programs use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Buffer {
    Main,
    Alternate,
}

/// What DECSC saves and DECRC restores.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct Saved {
    row: usize,
    column: usize,
    style: Style,
    origin: bool,
    charsets: [Charset; 2],
    shift: usize,
}

/// Where the cursor is and what it prints with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct Cursor {
    row: usize,
    column: usize,
    style: Style,
    /// The last column was written: the next character wraps first (DEC's "last column flag").
    pending_wrap: bool,
}

/// A place in the text: `line` counts from the oldest line of the scrollback, `column` from the left.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Point {
    pub line: usize,
    pub column: usize,
}

/// An xterm-like terminal of `columns` × `rows` cells.
pub struct Terminal {
    parser: vte::Parser,
    columns: usize,
    rows: usize,
    main: Vec<Row>,
    alternate: Vec<Row>,
    buffer: Buffer,
    history: VecDeque<Row>,
    history_limit: usize,
    /// Lines dropped from the front of the scrollback since the start, so a [`Point`] can be kept valid.
    dropped: u64,
    /// How many lines the view is scrolled back into the history; 0 shows the screen.
    display_offset: usize,
    cursor: Cursor,
    saved: [Saved; 2],
    /// The scrolling region, first and last row, inclusive (DECSTBM).
    top: usize,
    bottom: usize,
    tabs: Vec<bool>,
    modes: Modes,
    charsets: [Charset; 2],
    /// Which of G0 and G1 is in use (SI, SO).
    shift: usize,
    title: String,
    replies: Vec<u8>,
    bells: usize,
    /// The last character printed, for REP.
    last_printed: Option<char>,
}

impl std::fmt::Debug for Terminal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Terminal")
            .field("columns", &self.columns)
            .field("rows", &self.rows)
            .field("history", &self.history.len())
            .field("cursor", &self.cursor)
            .field("modes", &self.modes)
            .finish_non_exhaustive()
    }
}

impl Terminal {
    /// A blank terminal; sizes below 1 × 1 are taken as 1.
    pub fn new(columns: usize, rows: usize) -> Self {
        let columns = columns.max(1);
        let rows = rows.max(1);
        Self {
            parser: vte::Parser::new(),
            columns,
            rows,
            main: blank_rows(columns, rows),
            alternate: blank_rows(columns, rows),
            buffer: Buffer::Main,
            history: VecDeque::new(),
            history_limit: DEFAULT_HISTORY,
            dropped: 0,
            display_offset: 0,
            cursor: Cursor::default(),
            saved: [Saved::default(); 2],
            top: 0,
            bottom: rows - 1,
            tabs: default_tabs(columns),
            modes: Modes::default(),
            charsets: [Charset::Ascii; 2],
            shift: 0,
            title: String::new(),
            replies: Vec::new(),
            bells: 0,
            last_printed: None,
        }
    }

    /// Carries out what the host sent. Sequences cut in two by the network are completed by the next call.
    pub fn feed(&mut self, bytes: &[u8]) {
        let mut parser = std::mem::replace(&mut self.parser, vte::Parser::new());
        parser.advance(self, bytes);
        self.parser = parser;
    }

    /// What the terminal must send back to the host (device attributes, cursor position), since the last call.
    pub fn take_replies(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.replies)
    }

    /// How many times the host rang the bell since the last call.
    pub fn take_bells(&mut self) -> usize {
        std::mem::take(&mut self.bells)
    }

    pub fn columns(&self) -> usize {
        self.columns
    }

    pub fn rows(&self) -> usize {
        self.rows
    }

    pub fn modes(&self) -> &Modes {
        &self.modes
    }

    /// The window title the host set (OSC 0 or 2); empty if none.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// The cursor's row and column on the screen.
    pub fn cursor(&self) -> (usize, usize) {
        (self.cursor.row, self.cursor.column)
    }

    /// Is the alternate screen shown (full-screen programs)? It has no scrollback.
    pub fn alternate_screen(&self) -> bool {
        self.buffer == Buffer::Alternate
    }

    /// Sets how many lines the scrollback keeps, dropping the oldest beyond it.
    pub fn set_history_limit(&mut self, lines: usize) {
        self.history_limit = lines;
        self.trim_history();
    }

    /// Lines in the scrollback.
    pub fn history_len(&self) -> usize {
        self.history.len()
    }

    /// Lines dropped from the front of the scrollback so far; a selection made before a change of this is stale.
    pub fn dropped(&self) -> u64 {
        self.dropped
    }

    /// How far the view is scrolled back (R-SSH-8).
    pub fn display_offset(&self) -> usize {
        self.display_offset
    }

    /// Scrolls the view: positive back into the history, negative towards the screen.
    pub fn scroll_display(&mut self, lines: isize) {
        let offset = self.display_offset.saturating_add_signed(lines);
        self.display_offset = offset.min(self.history.len());
    }

    /// Shows the screen again, as a key sent to the host does (R-SSH-8).
    pub fn scroll_to_bottom(&mut self) {
        self.display_offset = 0;
    }

    /// The row shown at `row` of the view, scrolled back by the display offset.
    pub fn visible_row(&self, row: usize) -> Option<&Row> {
        let line = (self.history.len() + row).checked_sub(self.display_offset)?;
        self.line(line)
    }

    /// The line at `line` counted from the oldest line of the scrollback, the screen following it.
    pub fn line(&self, line: usize) -> Option<&Row> {
        match line.checked_sub(self.history.len()) {
            None => self.history.get(line),
            Some(row) => self.screen().get(row),
        }
    }

    /// The [`Point`] of a cell of the view.
    pub fn point_in_view(&self, row: usize, column: usize) -> Point {
        Point {
            line: (self.history.len() + row).saturating_sub(self.display_offset),
            column,
        }
    }

    /// The text from `from` to `to`, both included, in reading order: trailing blanks dropped from each line,
    /// and lines the terminal wrapped joined (R-SSH-9).
    pub fn text(&self, from: Point, to: Point) -> String {
        let (from, to) = if from <= to { (from, to) } else { (to, from) };
        let mut text = String::new();
        for line in from.line..=to.line {
            let Some(row) = self.line(line) else { break };
            let start = if line == from.line { from.column } else { 0 };
            let end = if line == to.line {
                to.column + 1
            } else {
                row.cells.len()
            };
            let piece = row.text_between(start, end);
            if line < to.line && row.wrapped {
                text.push_str(&piece);
            } else {
                text.push_str(piece.trim_end());
                if line < to.line {
                    text.push('\n');
                }
            }
        }
        text
    }

    /// Every line, the scrollback and the screen, as text (Copy all).
    pub fn all_text(&self) -> String {
        let last = self.history.len() + self.rows - 1;
        let mut text = self.text(
            Point { line: 0, column: 0 },
            Point {
                line: last,
                column: self.columns,
            },
        );
        let kept = text.trim_end_matches('\n').len();
        text.truncate(kept);
        text
    }

    /// Empties the scrollback (Edit ▸ Clear scrollback).
    pub fn clear_history(&mut self) {
        self.dropped += self.history.len() as u64;
        self.history.clear();
        self.display_offset = 0;
    }

    /// Changes the size. Rows that no longer fit go into the scrollback, keeping the cursor's line on screen,
    /// as xterm does; lines are not re-wrapped.
    pub fn resize(&mut self, columns: usize, rows: usize) {
        let columns = columns.max(1);
        let rows = rows.max(1);
        if columns == self.columns && rows == self.rows {
            return;
        }
        if rows < self.rows {
            let surplus = self.rows - rows;
            // Blank lines under the cursor go first; then lines from the top, into the scrollback.
            let below = self.rows - 1 - self.cursor.row;
            let from_bottom = surplus.min(below);
            for screen in [&mut self.main, &mut self.alternate] {
                screen.truncate(self.rows - from_bottom);
            }
            let from_top = surplus - from_bottom;
            for _ in 0..from_top {
                let row = self.main.remove(0);
                self.push_history(row);
                self.alternate.remove(0);
            }
            self.cursor.row -= from_top;
        } else {
            for screen in [&mut self.main, &mut self.alternate] {
                screen.resize(rows, Row::new(self.columns, Style::default()));
            }
        }
        for row in self
            .main
            .iter_mut()
            .chain(self.alternate.iter_mut())
            .chain(self.history.iter_mut())
        {
            row.resize(columns);
        }
        self.columns = columns;
        self.rows = rows;
        self.top = 0;
        self.bottom = rows - 1;
        self.tabs = default_tabs(columns);
        self.cursor.row = self.cursor.row.min(rows - 1);
        self.cursor.column = self.cursor.column.min(columns - 1);
        self.cursor.pending_wrap = false;
        self.display_offset = self.display_offset.min(self.history.len());
    }

    /// Back to the state at power-on (RIS), keeping the size and the scrollback.
    pub fn reset(&mut self) {
        let mut fresh = Self::new(self.columns, self.rows);
        fresh.history = std::mem::take(&mut self.history);
        fresh.history_limit = self.history_limit;
        fresh.dropped = self.dropped;
        *self = fresh;
    }

    fn screen(&self) -> &Vec<Row> {
        match self.buffer {
            Buffer::Main => &self.main,
            Buffer::Alternate => &self.alternate,
        }
    }

    fn screen_mut(&mut self) -> &mut Vec<Row> {
        match self.buffer {
            Buffer::Main => &mut self.main,
            Buffer::Alternate => &mut self.alternate,
        }
    }

    fn push_history(&mut self, row: Row) {
        if self.history_limit == 0 {
            self.dropped += 1;
            return;
        }
        self.history.push_back(row);
        if self.display_offset > 0 {
            // Keep what the user is reading in place while new lines arrive.
            self.display_offset = (self.display_offset + 1).min(self.history_limit);
        }
        self.trim_history();
    }

    fn trim_history(&mut self) {
        while self.history.len() > self.history_limit {
            self.history.pop_front();
            self.dropped += 1;
        }
        self.display_offset = self.display_offset.min(self.history.len());
    }

    /// Scrolls the rows `top..=bottom` up by `count`, blank rows coming in at the bottom. Rows leaving the top
    /// of the main screen go into the scrollback.
    fn scroll_up(&mut self, top: usize, bottom: usize, count: usize) {
        let count = count.min(bottom + 1 - top);
        let style = self.cursor.style;
        let to_history = self.buffer == Buffer::Main && top == 0;
        let columns = self.columns;
        for _ in 0..count {
            let row = self.screen_mut().remove(top);
            self.screen_mut().insert(bottom, Row::new(columns, style));
            if to_history {
                self.push_history(row);
            }
        }
    }

    /// Scrolls the rows `top..=bottom` down by `count`, blank rows coming in at the top.
    fn scroll_down(&mut self, top: usize, bottom: usize, count: usize) {
        let count = count.min(bottom + 1 - top);
        let style = self.cursor.style;
        let columns = self.columns;
        let screen = self.screen_mut();
        for _ in 0..count {
            screen.remove(bottom);
            screen.insert(top, Row::new(columns, style));
        }
    }

    /// Moves down a line, scrolling at the bottom of the scrolling region (IND, LF).
    fn index(&mut self) {
        self.cursor.pending_wrap = false;
        if self.cursor.row == self.bottom {
            self.scroll_up(self.top, self.bottom, 1);
        } else if self.cursor.row + 1 < self.rows {
            self.cursor.row += 1;
        }
    }

    /// Moves up a line, scrolling down at the top of the scrolling region (RI).
    fn reverse_index(&mut self) {
        self.cursor.pending_wrap = false;
        if self.cursor.row == self.top {
            self.scroll_down(self.top, self.bottom, 1);
        } else {
            self.cursor.row = self.cursor.row.saturating_sub(1);
        }
    }

    /// Puts the cursor at `row`, `column` (zero-based; rows inside the region in origin mode).
    fn goto(&mut self, row: usize, column: usize) {
        let (first, last) = if self.modes.origin {
            (self.top, self.bottom)
        } else {
            (0, self.rows - 1)
        };
        self.cursor.row = (first + row).min(last);
        self.cursor.column = column.min(self.columns - 1);
        self.cursor.pending_wrap = false;
    }

    fn cell_mut(&mut self, row: usize, column: usize) -> Option<&mut Cell> {
        self.screen_mut().get_mut(row)?.cells.get_mut(column)
    }

    /// Blanks the columns `from..to` of `row`, wide characters cut in half included.
    fn erase(&mut self, row: usize, from: usize, to: usize) {
        let blank = Cell::blank(self.cursor.style);
        let Some(line) = self.screen_mut().get_mut(row) else {
            return;
        };
        let to = to.min(line.cells.len());
        let from = from.min(to);
        // A wide character whose other half is erased goes too.
        let from = if line
            .cells
            .get(from)
            .is_some_and(|c| c.width == Width::Spacer)
        {
            from.saturating_sub(1)
        } else {
            from
        };
        let to = if to > 0
            && line
                .cells
                .get(to - 1)
                .is_some_and(|c| c.width == Width::Wide)
        {
            (to + 1).min(line.cells.len())
        } else {
            to
        };
        for cell in &mut line.cells[from..to] {
            *cell = blank.clone();
        }
        if to == line.cells.len() {
            line.wrapped = false;
        }
    }

    fn erase_rows(&mut self, from: usize, to: usize) {
        for row in from..to {
            self.erase(row, 0, self.columns);
        }
    }

    fn save_cursor(&mut self) {
        let slot = self.buffer as usize;
        self.saved[slot] = Saved {
            row: self.cursor.row,
            column: self.cursor.column,
            style: self.cursor.style,
            origin: self.modes.origin,
            charsets: self.charsets,
            shift: self.shift,
        };
    }

    fn restore_cursor(&mut self) {
        let saved = self.saved[self.buffer as usize];
        self.cursor = Cursor {
            row: saved.row.min(self.rows - 1),
            column: saved.column.min(self.columns - 1),
            style: saved.style,
            pending_wrap: false,
        };
        self.modes.origin = saved.origin;
        self.charsets = saved.charsets;
        self.shift = saved.shift;
    }

    /// Mode 1049, 1047 and 47: to the alternate screen and back.
    fn use_alternate(&mut self, on: bool, save_cursor: bool, clear: bool) {
        let wanted = if on { Buffer::Alternate } else { Buffer::Main };
        if self.buffer == wanted {
            return;
        }
        if on {
            if save_cursor {
                self.save_cursor();
            }
            self.buffer = Buffer::Alternate;
            if clear {
                self.erase_rows(0, self.rows);
            }
            self.display_offset = 0;
        } else {
            if clear {
                self.erase_rows(0, self.rows);
            }
            self.buffer = Buffer::Main;
            if save_cursor {
                self.restore_cursor();
            }
        }
    }
}

fn blank_rows(columns: usize, rows: usize) -> Vec<Row> {
    vec![Row::new(columns, Style::default()); rows]
}

/// A tab stop every eight columns.
fn default_tabs(columns: usize) -> Vec<bool> {
    (0..columns).map(|c| c > 0 && c % 8 == 0).collect()
}
