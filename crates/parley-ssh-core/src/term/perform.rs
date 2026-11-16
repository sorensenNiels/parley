//! What each control and sequence does (R-SSH-6). Section numbers are ECMA-48's; DEC private modes and replies
//! follow xterm's ctlseqs.

use unicode_width::UnicodeWidthChar;

use super::cell::{Cell, Width};
use super::{Charset, CursorShape, KeyMode, Modes, Terminal, sgr};

/// The longest window title kept; hosts can send any length.
const TITLE_LIMIT: usize = 256;

/// Primary device attributes: a VT220 with ANSI colour (DA1, §8.3.24).
const PRIMARY_ATTRIBUTES: &[u8] = b"\x1b[?62;22c";
/// Secondary device attributes: terminal type 1 (VT220), version 10, no ROM cartridge.
const SECONDARY_ATTRIBUTES: &[u8] = b"\x1b[>1;10;0c";

impl vte::Perform for Terminal {
    fn print(&mut self, c: char) {
        let c = match self.charsets.get(self.shift) {
            Some(Charset::DecSpecial) => dec_special(c),
            _ => c,
        };
        self.print_char(c);
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            0x07 => self.bells += 1,
            0x08 => self.backspace(),
            0x09 => self.tab_forward(1),
            0x0A..=0x0C => {
                self.index();
                if self.modes.newline {
                    self.cursor.column = 0;
                }
            }
            0x0D => {
                self.cursor.column = 0;
                self.cursor.pending_wrap = false;
            }
            0x0E => self.shift = 1,
            0x0F => self.shift = 0,
            _ => {}
        }
    }

    fn csi_dispatch(
        &mut self,
        params: &vte::Params,
        intermediates: &[u8],
        ignore: bool,
        action: char,
    ) {
        if ignore {
            return;
        }
        let args: Vec<usize> = params
            .iter()
            .map(|p| usize::from(p.first().copied().unwrap_or(0)))
            .collect();
        let arg = |i: usize| args.get(i).copied().unwrap_or(0);
        // Counts and positions: missing or 0 means 1.
        let n = arg(0).max(1);
        match (intermediates, action) {
            ([], '@') => self.insert_blanks(n),
            ([], 'A') => self.cursor_up(n),
            ([], 'B' | 'e') => self.cursor_down(n),
            ([], 'C' | 'a') => self.move_to_column(self.cursor.column.saturating_add(n)),
            ([], 'D') => self.move_to_column(self.cursor.column.saturating_sub(n)),
            ([], 'E') => {
                self.cursor_down(n);
                self.cursor.column = 0;
            }
            ([], 'F') => {
                self.cursor_up(n);
                self.cursor.column = 0;
            }
            ([], 'G' | '`') => self.move_to_column(n - 1),
            ([], 'H' | 'f') => self.goto(n - 1, arg(1).max(1) - 1),
            ([], 'I') => self.tab_forward(n),
            ([], 'Z') => self.tab_backward(n),
            ([] | [b'?'], 'J') => self.erase_display(arg(0)),
            ([] | [b'?'], 'K') => self.erase_line(arg(0)),
            ([], 'L') => self.insert_lines(n),
            ([], 'M') => self.delete_lines(n),
            ([], 'P') => self.delete_chars(n),
            ([], 'S') => self.scroll_up(self.top, self.bottom, n),
            // `CSI T` with five parameters is xterm's highlight mouse tracking, not a scroll.
            ([], 'T') if args.len() <= 1 => self.scroll_down(self.top, self.bottom, n),
            ([], 'X') => {
                self.cursor.pending_wrap = false;
                self.erase(
                    self.cursor.row,
                    self.cursor.column,
                    self.cursor.column.saturating_add(n),
                );
            }
            ([], 'b') => {
                if let Some(c) = self.last_printed {
                    // Bounded so a huge count cannot keep the terminal busy.
                    for _ in 0..n.min(self.columns * self.rows) {
                        self.print_char(c);
                    }
                }
            }
            ([], 'c') if arg(0) == 0 => self.replies.extend_from_slice(PRIMARY_ATTRIBUTES),
            ([b'>'], 'c') if arg(0) == 0 => self.replies.extend_from_slice(SECONDARY_ATTRIBUTES),
            ([], 'd') => {
                let column = self.cursor.column;
                self.goto(n - 1, column);
            }
            ([], 'g') => self.clear_tabs(arg(0)),
            ([], 'h') => args.iter().for_each(|&m| self.set_ansi_mode(m, true)),
            ([], 'l') => args.iter().for_each(|&m| self.set_ansi_mode(m, false)),
            ([b'?'], 'h') => args.iter().for_each(|&m| self.set_dec_mode(m, true)),
            ([b'?'], 'l') => args.iter().for_each(|&m| self.set_dec_mode(m, false)),
            ([], 'm') => sgr::apply(&mut self.cursor.style, params),
            ([], 'n') => self.device_status(arg(0), ""),
            ([b'?'], 'n') => self.device_status(arg(0), "?"),
            ([], 'r') => self.set_region(arg(0), arg(1)),
            ([], 's') => self.save_cursor(),
            ([], 'u') => self.restore_cursor(),
            ([], 't') if arg(0) == 18 => {
                let reply = format!("\x1b[8;{};{}t", self.rows, self.columns);
                self.replies.extend_from_slice(reply.as_bytes());
            }
            ([b' '], 'q') => self.set_cursor_style(arg(0)),
            ([b'!'], 'p') => self.soft_reset(),
            _ => {}
        }
    }

    fn esc_dispatch(&mut self, intermediates: &[u8], ignore: bool, byte: u8) {
        if ignore {
            return;
        }
        match (intermediates, byte) {
            ([], b'7') => self.save_cursor(),
            ([], b'8') => self.restore_cursor(),
            ([b'#'], b'8') => self.alignment_test(),
            ([], b'D') => self.index(),
            ([], b'E') => {
                self.index();
                self.cursor.column = 0;
            }
            ([], b'H') => {
                if let Some(stop) = self.tabs.get_mut(self.cursor.column) {
                    *stop = true;
                }
            }
            ([], b'M') => self.reverse_index(),
            ([], b'c') => self.reset(),
            ([], b'=') => self.modes.keypad = KeyMode::Application,
            ([], b'>') => self.modes.keypad = KeyMode::Normal,
            ([b'('], set) => self.charsets[0] = charset(set),
            ([b')'], set) => self.charsets[1] = charset(set),
            _ => {}
        }
    }

    fn osc_dispatch(&mut self, params: &[&[u8]], _bell_terminated: bool) {
        // OSC 0 and 2: the window title (OSC 1, the icon name, is not shown).
        if let [kind, rest @ ..] = params
            && matches!(*kind, b"0" | b"2")
        {
            let text = String::from_utf8_lossy(&rest.join(&b';')).into_owned();
            self.title = text
                .chars()
                .filter(|c| !c.is_control())
                .take(TITLE_LIMIT)
                .collect();
        }
    }
}

impl Terminal {
    pub(super) fn print_char(&mut self, c: char) {
        let width = c.width().unwrap_or(0);
        if width == 0 {
            self.combine(c);
            return;
        }
        if self.cursor.pending_wrap && self.modes.auto_wrap {
            self.wrap();
        }
        if width == 2 && self.cursor.column + 1 >= self.columns {
            if !self.modes.auto_wrap || self.columns < 2 {
                return;
            }
            // No room for both halves: the last column stays blank and the character goes to the next line.
            self.erase(self.cursor.row, self.cursor.column, self.columns);
            self.wrap();
        }
        if self.modes.insert {
            self.insert_blanks(width);
        }
        let (row, column) = (self.cursor.row, self.cursor.column);
        self.split_wide(row, column);
        if width == 2 {
            self.split_wide(row, column + 1);
        }
        let style = self.cursor.style;
        let width_kind = if width == 2 {
            Width::Wide
        } else {
            Width::Narrow
        };
        if let Some(cell) = self.cell_mut(row, column) {
            *cell = Cell {
                ch: c,
                combining: Vec::new(),
                style,
                width: width_kind,
            };
        }
        if width == 2
            && let Some(cell) = self.cell_mut(row, column + 1)
        {
            *cell = Cell {
                ch: ' ',
                combining: Vec::new(),
                style,
                width: Width::Spacer,
            };
        }
        self.last_printed = Some(c);
        let next = column + width;
        if next >= self.columns {
            self.cursor.column = self.columns - 1;
            self.cursor.pending_wrap = self.modes.auto_wrap;
        } else {
            self.cursor.column = next;
        }
    }

    /// Adds a combining mark to the character before the cursor.
    fn combine(&mut self, c: char) {
        let (row, mut column) = (self.cursor.row, self.cursor.column);
        if !self.cursor.pending_wrap {
            let Some(before) = column.checked_sub(1) else {
                return;
            };
            column = before;
        }
        if self
            .screen()
            .get(row)
            .and_then(|r| r.cells.get(column))
            .is_some_and(|c| c.width == Width::Spacer)
        {
            column = column.saturating_sub(1);
        }
        if let Some(cell) = self.cell_mut(row, column)
            && cell.combining.len() < 8
        {
            cell.combining.push(c);
        }
    }

    /// Goes to the start of the next line, marking this one as wrapped.
    fn wrap(&mut self) {
        let row = self.cursor.row;
        if let Some(line) = self.screen_mut().get_mut(row) {
            line.wrapped = true;
        }
        self.index();
        self.cursor.column = 0;
    }

    /// Before a cell is overwritten: the other half of a wide character there becomes a blank.
    fn split_wide(&mut self, row: usize, column: usize) {
        let Some(line) = self.screen_mut().get_mut(row) else {
            return;
        };
        let other = match line.cells.get(column).map(|c| c.width) {
            Some(Width::Wide) => column + 1,
            Some(Width::Spacer) => match column.checked_sub(1) {
                Some(left) => left,
                None => return,
            },
            Some(Width::Narrow) | None => return,
        };
        if let Some(cell) = line.cells.get_mut(other) {
            *cell = Cell::blank(cell.style);
        }
    }

    fn backspace(&mut self) {
        if self.cursor.pending_wrap {
            self.cursor.pending_wrap = false;
        } else {
            self.cursor.column = self.cursor.column.saturating_sub(1);
        }
    }

    fn cursor_up(&mut self, n: usize) {
        let limit = if self.cursor.row >= self.top {
            self.top
        } else {
            0
        };
        self.cursor.row = self.cursor.row.saturating_sub(n).max(limit);
        self.cursor.pending_wrap = false;
    }

    fn cursor_down(&mut self, n: usize) {
        let limit = if self.cursor.row <= self.bottom {
            self.bottom
        } else {
            self.rows - 1
        };
        self.cursor.row = self.cursor.row.saturating_add(n).min(limit);
        self.cursor.pending_wrap = false;
    }

    fn move_to_column(&mut self, column: usize) {
        self.cursor.column = column.min(self.columns - 1);
        self.cursor.pending_wrap = false;
    }

    fn tab_forward(&mut self, n: usize) {
        for _ in 0..n {
            let next =
                (self.cursor.column + 1..self.columns).find(|&c| self.tabs.get(c) == Some(&true));
            self.cursor.column = next.unwrap_or(self.columns - 1);
        }
        self.cursor.pending_wrap = false;
    }

    fn tab_backward(&mut self, n: usize) {
        for _ in 0..n {
            let previous = (0..self.cursor.column)
                .rev()
                .find(|&c| self.tabs.get(c) == Some(&true));
            self.cursor.column = previous.unwrap_or(0);
        }
        self.cursor.pending_wrap = false;
    }

    fn clear_tabs(&mut self, which: usize) {
        match which {
            0 => {
                if let Some(stop) = self.tabs.get_mut(self.cursor.column) {
                    *stop = false;
                }
            }
            3 => self.tabs.iter_mut().for_each(|stop| *stop = false),
            _ => {}
        }
    }

    /// ED (§8.3.39): 0 from the cursor on, 1 up to the cursor, 2 all, 3 the scrollback (xterm).
    fn erase_display(&mut self, which: usize) {
        let (row, column) = (self.cursor.row, self.cursor.column);
        match which {
            0 => {
                self.erase(row, column, self.columns);
                self.erase_rows(row + 1, self.rows);
            }
            1 => {
                self.erase_rows(0, row);
                self.erase(row, 0, column + 1);
            }
            2 => self.erase_rows(0, self.rows),
            3 => self.clear_history(),
            _ => {}
        }
        self.cursor.pending_wrap = false;
    }

    /// EL (§8.3.41): 0 from the cursor on, 1 up to the cursor, 2 the whole line.
    fn erase_line(&mut self, which: usize) {
        let (row, column) = (self.cursor.row, self.cursor.column);
        match which {
            0 => self.erase(row, column, self.columns),
            1 => self.erase(row, 0, column + 1),
            2 => self.erase(row, 0, self.columns),
            _ => {}
        }
        self.cursor.pending_wrap = false;
    }

    /// ICH (§8.3.64) and insert mode: blanks at the cursor push the rest of the line right.
    fn insert_blanks(&mut self, n: usize) {
        let (row, column) = (self.cursor.row, self.cursor.column);
        self.split_wide(row, column);
        let blank = Cell::blank(self.cursor.style);
        let Some(line) = self.screen_mut().get_mut(row) else {
            return;
        };
        let width = line.cells.len();
        let n = n.min(width - column);
        line.cells.truncate(width - n);
        line.cells
            .splice(column..column, std::iter::repeat_n(blank, n));
        if line.cells.last().is_some_and(|c| c.width == Width::Wide) {
            let last = width - 1;
            line.cells[last] = Cell::blank(line.cells[last].style);
        }
        self.cursor.pending_wrap = false;
    }

    /// DCH (§8.3.26): characters at the cursor go; blanks come in at the right.
    fn delete_chars(&mut self, n: usize) {
        let (row, column) = (self.cursor.row, self.cursor.column);
        self.split_wide(row, column);
        let blank = Cell::blank(self.cursor.style);
        let Some(line) = self.screen_mut().get_mut(row) else {
            return;
        };
        let n = n.min(line.cells.len() - column);
        line.cells.drain(column..column + n);
        line.cells.extend(std::iter::repeat_n(blank, n));
        if line
            .cells
            .get(column)
            .is_some_and(|c| c.width == Width::Spacer)
        {
            line.cells[column] = Cell::blank(line.cells[column].style);
        }
        self.cursor.pending_wrap = false;
    }

    /// IL (§8.3.67): only inside the scrolling region; the cursor goes to the line's start.
    fn insert_lines(&mut self, n: usize) {
        if (self.top..=self.bottom).contains(&self.cursor.row) {
            self.scroll_down(self.cursor.row, self.bottom, n);
            self.cursor.column = 0;
            self.cursor.pending_wrap = false;
        }
    }

    /// DL (§8.3.32): only inside the scrolling region; deleted lines never go into the scrollback.
    fn delete_lines(&mut self, n: usize) {
        if (self.top..=self.bottom).contains(&self.cursor.row) {
            let n = n.min(self.bottom + 1 - self.cursor.row);
            let style = self.cursor.style;
            let columns = self.columns;
            let (row, bottom) = (self.cursor.row, self.bottom);
            let screen = self.screen_mut();
            for _ in 0..n {
                screen.remove(row);
                screen.insert(bottom, super::Row::new(columns, style));
            }
            self.cursor.column = 0;
            self.cursor.pending_wrap = false;
        }
    }

    /// DECSTBM: rows are one-based; a region of fewer than two rows is ignored. The cursor goes home.
    fn set_region(&mut self, top: usize, bottom: usize) {
        let top = top.max(1) - 1;
        let bottom = if bottom == 0 {
            self.rows
        } else {
            bottom.min(self.rows)
        } - 1;
        if top < bottom {
            self.top = top;
            self.bottom = bottom;
            self.goto(0, 0);
        }
    }

    /// DSR (§8.3.35): 5 status, 6 the cursor's position (relative to the region in origin mode).
    fn device_status(&mut self, which: usize, private: &str) {
        let reply = match which {
            5 => "\x1b[0n".to_owned(),
            6 => {
                let row = if self.modes.origin {
                    self.cursor.row - self.top
                } else {
                    self.cursor.row
                };
                format!("\x1b[{private}{};{}R", row + 1, self.cursor.column + 1)
            }
            _ => return,
        };
        self.replies.extend_from_slice(reply.as_bytes());
    }

    /// SM and RM (§8.3.125, §8.3.106): 4 insert, 20 automatic new line.
    fn set_ansi_mode(&mut self, mode: usize, on: bool) {
        match mode {
            4 => self.modes.insert = on,
            20 => self.modes.newline = on,
            _ => {}
        }
    }

    /// DECSET and DECRST.
    fn set_dec_mode(&mut self, mode: usize, on: bool) {
        match mode {
            1 => {
                self.modes.cursor_keys = if on {
                    KeyMode::Application
                } else {
                    KeyMode::Normal
                }
            }
            6 => {
                self.modes.origin = on;
                self.goto(0, 0);
            }
            7 => self.modes.auto_wrap = on,
            12 => self.modes.cursor_blink = on,
            25 => self.modes.cursor_visible = on,
            47 => self.use_alternate(on, false, false),
            1047 => self.use_alternate(on, false, !on),
            1048 => {
                if on {
                    self.save_cursor();
                } else {
                    self.restore_cursor();
                }
            }
            1049 => self.use_alternate(on, true, true),
            1004 => self.modes.focus_events = on,
            2004 => self.modes.bracketed_paste = on,
            _ => {}
        }
    }

    /// DECSCUSR: 0 and 1 blinking block, 2 steady block, 3/4 underline, 5/6 bar.
    fn set_cursor_style(&mut self, style: usize) {
        let (shape, blink) = match style {
            0 | 1 => (CursorShape::Block, true),
            2 => (CursorShape::Block, false),
            3 => (CursorShape::Underline, true),
            4 => (CursorShape::Underline, false),
            5 => (CursorShape::Bar, true),
            6 => (CursorShape::Bar, false),
            _ => return,
        };
        self.modes.cursor_shape = shape;
        self.modes.cursor_blink = blink;
    }

    /// DECSTR: modes, region, character sets and the saved cursor back to their defaults; the screen stays.
    fn soft_reset(&mut self) {
        let shape = (self.modes.cursor_shape, self.modes.cursor_blink);
        self.modes = Modes {
            cursor_shape: shape.0,
            cursor_blink: shape.1,
            ..Modes::default()
        };
        self.top = 0;
        self.bottom = self.rows - 1;
        self.cursor.style = super::Style::default();
        self.cursor.pending_wrap = false;
        self.charsets = [Charset::Ascii; 2];
        self.shift = 0;
        self.saved[self.buffer as usize] = super::Saved::default();
    }

    /// DECALN: the screen filled with `E`, the region reset, the cursor home.
    fn alignment_test(&mut self) {
        self.top = 0;
        self.bottom = self.rows - 1;
        let fill = Cell {
            ch: 'E',
            ..Cell::default()
        };
        for row in self.screen_mut() {
            row.cells.iter_mut().for_each(|cell| *cell = fill.clone());
            row.wrapped = false;
        }
        self.goto(0, 0);
    }
}

/// SCS final bytes: `0` DEC Special Graphics; everything else (`B` US ASCII, national sets) as ASCII.
fn charset(set: u8) -> Charset {
    if set == b'0' {
        Charset::DecSpecial
    } else {
        Charset::Ascii
    }
}

/// DEC Special Graphics (VT100 User Guide table 3-9), as Unicode.
fn dec_special(c: char) -> char {
    match c {
        '_' => ' ',
        '`' => '◆',
        'a' => '▒',
        'b' => '␉',
        'c' => '␌',
        'd' => '␍',
        'e' => '␊',
        'f' => '°',
        'g' => '±',
        'h' => '␤',
        'i' => '␋',
        'j' => '┘',
        'k' => '┐',
        'l' => '┌',
        'm' => '└',
        'n' => '┼',
        'o' => '⎺',
        'p' => '⎻',
        'q' => '─',
        'r' => '⎼',
        's' => '⎽',
        't' => '├',
        'u' => '┤',
        'v' => '┴',
        'w' => '┬',
        'x' => '│',
        'y' => '≤',
        'z' => '≥',
        '{' => 'π',
        '|' => '≠',
        '}' => '£',
        '~' => '·',
        other => other,
    }
}
