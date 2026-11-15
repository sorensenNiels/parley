//! GUI windows (R-DS-10, D38; SC30-3533-04 §15.6.13.1 "CREATE WINDOW Structured Field",
//! §15.6.13.2-4). A window is written into the screen as the 5494 writes it — its border
//! attributes and characters, its title and footer, and a cleared inside — so reading, copying
//! and saving the screen work as before. Parley also keeps the window: what it covers, to put
//! back when it is removed (on a GUI display a window covers the screen), and its place, so the
//! UI can draw it as a frame and the cursor can be kept inside.

use super::{Rectangle, Screen};

/// The default border attribute: blue (§15.6.13.1, "Default Border Attribute").
pub const DEFAULT_BORDER_ATTRIBUTE: u8 = 0x3A;
/// Normal intensity: ends the border's colour inside the window.
const NORMAL: u8 = 0x20;

/// The border's characters, as EBCDIC bytes: upper left, top, upper right, left, right, lower
/// left, bottom, lower right.
pub type BorderChars = [u8; 8];

/// Where the title or footer sits in its border row (Table 55, flag bits 0-1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Center,
    Right,
    Left,
}

/// A title or footer: its text (EBCDIC) and the attribute before it, if any.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Caption {
    pub text: Vec<u8>,
    pub align: Align,
    pub attribute: Option<u8>,
}

/// What Create Window asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowSpec {
    /// The top-left corner: the address the window is created at, 0-based.
    pub row: usize,
    pub column: usize,
    /// Rows and columns inside the border.
    pub depth: usize,
    pub width: usize,
    pub restrict_cursor: bool,
    pub pull_down: bool,
    pub attribute: u8,
    pub chars: BorderChars,
    pub title: Option<Caption>,
    pub footer: Option<Caption>,
}

/// A window on the screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Window {
    pub spec: WindowSpec,
    /// The screen bytes under the window when it was created, row by row.
    covered: Vec<Vec<u8>>,
}

impl Window {
    /// Every position the window takes, its border included.
    pub const fn outer(&self) -> Rectangle {
        let s = &self.spec;
        Rectangle {
            top: s.row,
            left: s.column,
            bottom: s.row + s.depth + 1,
            right: s.column + s.width + 5,
        }
    }

    /// The positions inside the border.
    pub const fn inner(&self) -> Rectangle {
        let s = &self.spec;
        Rectangle {
            top: s.row + 1,
            left: s.column + 3,
            bottom: s.row + s.depth,
            right: s.column + s.width + 2,
        }
    }
}

impl Screen {
    pub fn windows(&self) -> &[Window] {
        &self.windows
    }

    /// The window the cursor must stay in, if one asked for that.
    pub fn cursor_limit(&self) -> Option<Rectangle> {
        self.cursor_limit
    }

    /// Creates a window as `spec` says; `None` when it does not fit on the screen. A window in
    /// the same place and of the same size as another replaces it; there are at most 128.
    pub fn create_window(&mut self, spec: WindowSpec) -> Option<()> {
        let (rows, columns) = (
            usize::from(self.size.rows()),
            usize::from(self.size.columns()),
        );
        if spec.depth == 0
            || spec.width == 0
            || spec.row + spec.depth + 1 >= rows
            || spec.column + spec.width + 5 >= columns
        {
            return None;
        }
        self.windows.retain(|w| {
            !(w.spec.row == spec.row
                && w.spec.column == spec.column
                && w.spec.depth == spec.depth
                && w.spec.width == spec.width)
        });
        if self.windows.len() >= MAX_WINDOWS {
            return None;
        }
        let mut window = Window {
            spec,
            covered: Vec::new(),
        };
        let outer = window.outer();
        window.covered = (outer.top..=outer.bottom)
            .map(|r| {
                let row = self.row_bytes(r);
                row.get(outer.left..=outer.right)
                    .unwrap_or_default()
                    .to_vec()
            })
            .collect();
        self.draw_window(&window);
        self.cursor_limit = window.spec.restrict_cursor.then(|| window.inner());
        self.windows.push(window);
        Some(())
    }

    /// Removes the window created at `row`, `column` and puts back what it covered; removing
    /// the newest window lets the cursor go anywhere again (§15.6.13.3). False if there is none.
    pub fn remove_window(&mut self, row: usize, column: usize) -> bool {
        let Some(i) = self
            .windows
            .iter()
            .rposition(|w| w.spec.row == row && w.spec.column == column)
        else {
            return false;
        };
        let newest = i + 1 == self.windows.len();
        let window = self.windows.remove(i);
        self.uncover(&window);
        if newest {
            self.cursor_limit = None;
        }
        true
    }

    /// Removes every window, newest first, and lets the cursor go anywhere (§15.6.13.4).
    pub fn remove_all_windows(&mut self) {
        while let Some(window) = self.windows.pop() {
            self.uncover(&window);
        }
        self.cursor_limit = None;
    }

    /// Lets the cursor go anywhere again (§15.6.13.2).
    pub const fn unrestrict_cursor(&mut self) {
        self.cursor_limit = None;
    }

    fn uncover(&mut self, window: &Window) {
        let outer = window.outer();
        for (i, bytes) in window.covered.iter().enumerate() {
            let start = (outer.top + i) * usize::from(self.size.columns()) + outer.left;
            for (k, &b) in bytes.iter().enumerate() {
                self.put(start + k, b);
            }
        }
    }

    /// Writes the border, the title and footer, and the cleared inside (§15.6.13.1, "How the
    /// 5494 Processes the CREATE WINDOW Structured Field", figures 22-24).
    fn draw_window(&mut self, window: &Window) {
        let s = &window.spec;
        let outer = window.outer();
        let columns = usize::from(self.size.columns());
        let at = |r: usize, c: usize| r * columns + c;
        let [ul, top, ur, left, right, ll, bottom, lr] = s.chars;
        let across = s.width + 2;
        let put_row = |screen: &mut Self,
                       r: usize,
                       first: u8,
                       fill: u8,
                       last: u8,
                       caption: Option<&Caption>| {
            screen.put(at(r, outer.left), s.attribute);
            screen.put(at(r, outer.left + 1), first);
            let mut line = vec![fill; across];
            if let Some(caption) = caption {
                write_caption(&mut line, caption, s.attribute);
            }
            for (k, b) in line.into_iter().enumerate() {
                screen.put(at(r, outer.left + 2 + k), b);
            }
            screen.put(at(r, outer.right - 1), last);
            screen.put(at(r, outer.right), NORMAL);
        };
        let title = if s.pull_down { None } else { s.title.as_ref() };
        put_row(self, outer.top, ul, top, ur, title);
        for r in outer.top + 1..outer.bottom {
            self.put(at(r, outer.left), s.attribute);
            self.put(at(r, outer.left + 1), left);
            self.put(at(r, outer.left + 2), NORMAL);
            self.fill(at(r, outer.left + 3), at(r, outer.left + 2 + s.width), 0);
            self.put(at(r, outer.right - 2), s.attribute);
            self.put(at(r, outer.right - 1), right);
            self.put(at(r, outer.right), NORMAL);
        }
        put_row(self, outer.bottom, ll, bottom, lr, s.footer.as_ref());
    }
}

/// At most this many windows on a screen (§15.6.13.1).
pub const MAX_WINDOWS: usize = 128;

/// Writes `caption` into a border row's characters, cut to fit, with its attribute before it
/// and the border's after it when it has one (Table 55).
fn write_caption(line: &mut [u8], caption: &Caption, border: u8) {
    let mut text = Vec::new();
    if let Some(a) = caption.attribute {
        text.push(a);
    }
    text.extend(&caption.text);
    if caption.attribute.is_some() {
        text.push(border);
    }
    text.truncate(line.len());
    let start = match caption.align {
        Align::Left => 0,
        Align::Right => line.len() - text.len(),
        Align::Center => (line.len() - text.len()) / 2,
    };
    line[start..start + text.len()].copy_from_slice(&text);
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

    fn spec(row: usize, column: usize, depth: usize, width: usize) -> WindowSpec {
        let dot = PAGE.to_byte('.').unwrap_or(0);
        let colon = PAGE.to_byte(':').unwrap_or(0);
        WindowSpec {
            row,
            column,
            depth,
            width,
            restrict_cursor: true,
            pull_down: false,
            attribute: DEFAULT_BORDER_ATTRIBUTE,
            chars: [dot, dot, dot, colon, colon, colon, dot, colon],
            title: Some(Caption {
                text: ebcdic("Æble"),
                align: Align::Center,
                attribute: None,
            }),
            footer: None,
        }
    }

    /// Columns `from..to` of row `r`, as characters.
    fn cols(screen: &Screen, r: usize, from: usize, to: usize) -> String {
        let text = screen.text(PAGE);
        let line = text.lines().nth(r).unwrap_or_default();
        line.chars().skip(from).take(to - from).collect()
    }

    #[test]
    fn a_window_is_written_as_the_5494_writes_it_r_ds_10() {
        let mut screen = Screen::new(ScreenSize::Standard);
        screen.set_row_bytes(3, &ebcdic("underneath underneath underneath"));
        assert!(screen.create_window(spec(2, 4, 2, 6)).is_some());
        // Attribute, corner, 8 border characters with the title, corner, attribute.
        assert_eq!(cols(&screen, 2, 4, 16), " ...Æble... ");
        assert_eq!(
            cols(&screen, 3, 4, 16),
            " :        : ",
            "the inside is cleared"
        );
        assert_eq!(cols(&screen, 5, 4, 16), " :........: ");
        assert_eq!(cols(&screen, 3, 0, 4), "unde", "outside stays");
        assert_eq!(screen.windows().len(), 1);
        assert_eq!(
            screen.cursor_limit(),
            Some(Rectangle {
                top: 3,
                left: 7,
                bottom: 4,
                right: 12
            })
        );
    }

    #[test]
    fn removing_a_window_puts_back_what_it_covered_r_ds_10() {
        let mut screen = Screen::new(ScreenSize::Standard);
        screen.set_row_bytes(3, &ebcdic("underneath underneath underneath"));
        let before = screen.text(PAGE);
        screen.create_window(spec(2, 4, 2, 6));
        assert!(
            !screen.remove_window(2, 5),
            "only at the address it was made at"
        );
        assert!(screen.remove_window(2, 4));
        assert_eq!(screen.text(PAGE), before);
        assert_eq!(screen.cursor_limit(), None);
        assert_eq!(screen.windows().len(), 0);
    }

    #[test]
    fn remove_all_and_clear_take_every_window_away_r_ds_10() {
        let mut screen = Screen::new(ScreenSize::Standard);
        screen.create_window(spec(1, 1, 2, 6));
        screen.create_window(spec(10, 10, 3, 20));
        screen.remove_all_windows();
        assert!(screen.windows().is_empty() && screen.cursor_limit().is_none());
        assert_eq!(screen.text(PAGE).trim(), "");
        screen.create_window(spec(1, 1, 2, 6));
        screen.clear(ScreenSize::Standard);
        assert_eq!(screen.windows().len(), 0);
    }

    #[test]
    fn a_window_that_does_not_fit_is_refused_and_the_same_place_replaces_r_ds_10() {
        let mut screen = Screen::new(ScreenSize::Standard);
        assert!(
            screen.create_window(spec(22, 0, 1, 10)).is_none(),
            "past the bottom"
        );
        assert!(
            screen.create_window(spec(0, 70, 1, 10)).is_none(),
            "past the right"
        );
        assert!(screen.create_window(spec(0, 0, 0, 10)).is_none(), "no rows");
        screen.create_window(spec(1, 1, 2, 6));
        screen.create_window(spec(1, 1, 2, 6));
        assert_eq!(screen.windows().len(), 1);
    }

    #[test]
    fn a_long_title_is_cut_and_a_right_footer_sits_right_r_ds_10() {
        let mut screen = Screen::new(ScreenSize::Standard);
        let mut s = spec(0, 0, 1, 4);
        s.title = Some(Caption {
            text: ebcdic("Much too long"),
            align: Align::Left,
            attribute: None,
        });
        s.footer = Some(Caption {
            text: ebcdic("F3"),
            align: Align::Right,
            attribute: None,
        });
        screen.create_window(s);
        assert_eq!(cols(&screen, 0, 0, 10), " .Much t. ");
        assert_eq!(cols(&screen, 2, 0, 10), " :....F3: ");
    }
}
