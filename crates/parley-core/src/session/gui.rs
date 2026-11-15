//! Write to Display Structured Field (WDSF) with IBM i's GUI-like 5250 (R-DS-10, D38;
//! SC30-3533-04 §15.6.13): only while the session's "GUI elements" switch is on, the only time
//! the query reply claims enhanced user interface level 1. Windows so far; selection fields and
//! scroll bars (R-DS-11, R-DS-12) are skipped.

use super::Session;
use crate::error::{Error, Result};
use crate::screen::{
    Align, BorderChars, Caption, DEFAULT_BORDER_ATTRIBUTE, WindowSpec, is_attribute,
};

/// The 5250 class of structured field.
const CLASS: u8 = 0xD9;
const CREATE_WINDOW: u8 = 0x51;
const UNRESTRICTED_CURSOR: u8 = 0x52;
const REMOVE_WINDOW: u8 = 0x59;
const REMOVE_ALL: u8 = 0x5F;
/// Create Window's minor structures (Tables 54 and 55).
const BORDER: u8 = 0x01;
const TITLE: u8 = 0x10;

impl Session {
    /// Carries out a WDSF order's structured field, `body` being what follows its length, at
    /// screen address `address`. Without the switch on, or of a type Parley does not draw yet,
    /// it is skipped.
    pub(super) fn gui_structured_field(&mut self, body: &[u8], address: usize) -> Result<()> {
        if !self.config.gui {
            return Ok(());
        }
        let [CLASS, kind, rest @ ..] = body else {
            return Err(Error::data(
                "a Write to Display Structured Field of another class",
            ));
        };
        let (row, column) = self.screen.row_column(address);
        match *kind {
            CREATE_WINDOW => {
                let spec = self.window_spec(rest, row, column)?;
                self.screen.create_window(spec).ok_or_else(|| {
                    Error::data(format!("a window at row {} that does not fit", row + 1))
                })?;
            }
            UNRESTRICTED_CURSOR => self.screen.unrestrict_cursor(),
            REMOVE_WINDOW => {
                self.screen.remove_window(row, column);
            }
            REMOVE_ALL => self.screen.remove_all_windows(),
            _ => {}
        }
        Ok(())
    }

    /// Create Window's major structure (Table 53, from byte 5) and its minor structures.
    fn window_spec(&self, rest: &[u8], row: usize, column: usize) -> Result<WindowSpec> {
        let [flags, _flags2, _reserved, depth, width, minors @ ..] = rest else {
            return Err(Error::data("a Create Window cut short"));
        };
        let page = self.code_page();
        let dot = page.to_byte('.').unwrap_or(0x4B);
        let colon = page.to_byte(':').unwrap_or(0x7A);
        let mut spec = WindowSpec {
            row,
            column,
            depth: usize::from(*depth),
            width: usize::from(*width),
            restrict_cursor: flags & 0x80 != 0,
            pull_down: flags & 0x40 != 0,
            attribute: DEFAULT_BORDER_ATTRIBUTE,
            // §15.6.13.1, "Default Border Attribute and Characters".
            chars: [dot, dot, dot, colon, colon, colon, dot, colon],
            title: None,
            footer: None,
        };
        let mut minors = minors;
        while let [len, ..] = *minors {
            let len = usize::from(len);
            let Some(minor) = minors.get(..len).filter(|_| len >= 2) else {
                return Err(Error::data("a Create Window minor structure cut short"));
            };
            match minor[1] {
                BORDER => apply_border(&mut spec, minor),
                TITLE => {
                    let (caption, footer) = caption(minor);
                    if footer {
                        spec.footer = Some(caption);
                    } else {
                        spec.title = Some(caption);
                    }
                }
                _ => {}
            }
            minors = &minors[len..];
        }
        Ok(spec)
    }
}

/// A display attribute, if `b` is one (X'20'-X'3F'); X'00' or anything else means none.
fn attribute(b: Option<&u8>) -> Option<u8> {
    b.copied().filter(|&b| is_attribute(b))
}

/// The border presentation minor structure (Table 54): the colour display's attribute, then
/// any characters that are not X'00'.
fn apply_border(spec: &mut WindowSpec, minor: &[u8]) {
    if let Some(a) = attribute(minor.get(4)) {
        spec.attribute = a;
    }
    let chars: &mut BorderChars = &mut spec.chars;
    for (slot, &b) in chars.iter_mut().zip(minor.iter().skip(5)) {
        if b != 0 {
            *slot = b;
        }
    }
}

/// The window title/footer minor structure (Table 55): the caption and whether it is the footer.
fn caption(minor: &[u8]) -> (Caption, bool) {
    let flags = minor.get(2).copied().unwrap_or(0);
    let align = match flags >> 6 {
        1 => Align::Right,
        2 => Align::Left,
        _ => Align::Center,
    };
    let caption = Caption {
        text: minor.get(6..).unwrap_or_default().to_vec(),
        align,
        attribute: attribute(minor.get(4)),
    };
    (caption, flags & 0x20 != 0)
}
