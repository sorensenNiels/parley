//! What one cell of the screen holds: a character, its colours and its attributes (R-SSH-6).

/// A colour as the host chose it (SGR 30–37, 38, 90–97 and the backgrounds; ECMA-48 §8.3.117, xterm ctlseqs).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Color {
    /// The terminal's own foreground or background.
    #[default]
    Default,
    /// One of the 256 xterm colours: 0–7 normal, 8–15 bright, 16–231 the colour cube, 232–255 greys.
    Indexed(u8),
    /// A 24-bit colour.
    Rgb(u8, u8, u8),
}

/// How text is underlined (SGR 4, 4:2, 4:3, 21, 24).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Underline {
    #[default]
    None,
    Single,
    Double,
    Curly,
}

/// Is the text bold, faint or neither (SGR 1, 2, 22)?
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Weight {
    #[default]
    Normal,
    Bold,
    Faint,
}

/// The attributes a cell is drawn with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each is an independent SGR attribute"
)]
pub struct Style {
    pub fg: Color,
    pub bg: Color,
    pub weight: Weight,
    pub underline: Underline,
    pub italic: bool,
    pub blink: bool,
    pub inverse: bool,
    pub hidden: bool,
    pub strike: bool,
}

/// Which part of a character a cell holds: East Asian wide characters and most emoji take two cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Width {
    #[default]
    Narrow,
    /// The left half of a wide character.
    Wide,
    /// The right half of a wide character: drawn by the cell to its left.
    Spacer,
}

/// One cell of the screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    pub ch: char,
    /// Combining marks that follow `ch` (accents, variation selectors).
    pub combining: Vec<char>,
    pub style: Style,
    pub width: Width,
}

impl Default for Cell {
    fn default() -> Self {
        Self::blank(Style::default())
    }
}

impl Cell {
    /// An empty cell. Erasing keeps the background colour in use (xterm's "background colour erase").
    pub fn blank(style: Style) -> Self {
        Self {
            ch: ' ',
            combining: Vec::new(),
            style: Style {
                bg: style.bg,
                ..Style::default()
            },
            width: Width::Narrow,
        }
    }

    /// The text this cell shows: its character and combining marks; nothing for the right half of a wide one.
    pub fn text(&self) -> String {
        if self.width == Width::Spacer {
            return String::new();
        }
        let mut text = String::with_capacity(4);
        text.push(self.ch);
        text.extend(&self.combining);
        text
    }
}

/// One line of the screen or the scrollback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub cells: Vec<Cell>,
    /// The text goes on in the next line: it was wrapped there, not ended. Copy joins such lines (R-SSH-9).
    pub wrapped: bool,
}

impl Row {
    pub(crate) fn new(columns: usize, style: Style) -> Self {
        Self {
            cells: vec![Cell::blank(style); columns],
            wrapped: false,
        }
    }

    /// The row's text, with the trailing blanks dropped.
    pub fn text(&self) -> String {
        self.text_between(0, self.cells.len()).trim_end().to_owned()
    }

    /// The text of the columns `from..to` (cut to the row), blanks kept.
    pub fn text_between(&self, from: usize, to: usize) -> String {
        let to = to.min(self.cells.len());
        self.cells
            .get(from..to)
            .map_or_else(String::new, |cells| cells.iter().map(Cell::text).collect())
    }

    pub(crate) fn resize(&mut self, columns: usize) {
        self.cells.resize(columns, Cell::default());
        // A wide character cut in half at the new edge becomes a blank.
        if let Some(last) = self.cells.last_mut()
            && last.width == Width::Wide
        {
            *last = Cell::blank(last.style);
        }
    }
}
