//! Extended attributes (R-DS-9, SC30-3533-04 §15.6.11): the extended character buffer that
//! Write Extended Attribute writes to. An extended attribute takes no position on the screen;
//! it holds from its position until the next one of its type and takes precedence over the
//! screen attributes, and X'80' gives them back.

use super::{Cell, Color, Style, is_attribute};

/// Which plane of the extended character buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtendedKind {
    /// Type X'01': column separators, blink, underline, high intensity, reverse image.
    Primary,
    /// Type X'03': the foreground colour.
    Color,
}

/// The null extended attribute: the one in effect goes on.
const NULL: u8 = 0x00;
/// The normal extended attribute: the screen attributes govern.
const NORMAL: u8 = 0x80;

/// The two planes, a byte per position; X'00' where none was written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Extended {
    primary: Vec<u8>,
    color: Vec<u8>,
}

impl Extended {
    pub fn new(positions: usize) -> Self {
        Self {
            primary: vec![NULL; positions],
            color: vec![NULL; positions],
        }
    }

    pub fn put(&mut self, address: usize, kind: ExtendedKind, value: u8) {
        let plane = match kind {
            ExtendedKind::Primary => &mut self.primary,
            ExtendedKind::Color => &mut self.color,
        };
        if let Some(slot) = plane.get_mut(address) {
            *slot = value;
        }
    }

    /// Lays the extended attributes over `cells`, made from `buffer`; the positions that hold
    /// screen attributes stay plain blanks.
    pub fn overlay(&self, buffer: &[u8], cells: &mut [Cell]) {
        let (mut primary, mut color) = (NORMAL, NORMAL);
        for (i, cell) in cells.iter_mut().enumerate() {
            if let Some(&p) = self.primary.get(i).filter(|&&p| p != NULL) {
                primary = p;
            }
            if let Some(&c) = self.color.get(i).filter(|&&c| c != NULL) {
                color = c;
            }
            if buffer.get(i).is_some_and(|&b| is_attribute(b)) {
                continue;
            }
            cell.style = with_primary(cell.style, primary);
            match extended_color(color) {
                Foreground::Screen => {}
                Foreground::Color(c) => cell.style.color = c,
                // The background's colour: nothing shows.
                Foreground::Background => cell.ch = ' ',
            }
        }
    }
}

/// `style` under extended primary attribute `p` (table 46; bit 0 the high-order bit).
fn with_primary(style: Style, p: u8) -> Style {
    if p & 0x80 == 0 || p == NORMAL {
        return style;
    }
    let high_intensity = p & 0x02 != 0;
    Style {
        column_separators: p & 0x10 != 0,
        blink: p & 0x08 != 0,
        underline: p & 0x04 != 0,
        reverse: p & 0x01 != 0,
        // High intensity shows as white, as the screen attributes' high intensity does.
        color: if high_intensity {
            Color::White
        } else {
            style.color
        },
        ..style
    }
}

/// What an extended colour attribute does to the text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Foreground {
    /// X'80' (or not a colour): the screen attributes' colour.
    Screen,
    /// X'81': the background's colour.
    Background,
    Color(Color),
}

/// The colour extended attribute `c` asks for (§15.6.11.2); each colour's two shades show alike.
const fn extended_color(c: u8) -> Foreground {
    Foreground::Color(match c {
        0x81 => return Foreground::Background,
        0x82 | 0x83 => Color::Blue,
        0x84 | 0x85 => Color::Green,
        0x86 | 0x87 => Color::Turquoise,
        0x88 | 0x89 => Color::Red,
        0x8A | 0x8B => Color::Pink,
        0x8C | 0x8D => Color::Yellow,
        0x8E | 0x8F => Color::White,
        _ => return Foreground::Screen,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CodePage;
    use crate::screen::{Screen, ScreenSize};

    fn screen_with(text: &str) -> Screen {
        let mut screen = Screen::new(ScreenSize::Standard);
        let bytes: Vec<u8> = text
            .chars()
            .map(|c| CodePage::Ccsid37.to_byte(c).unwrap_or(0x40))
            .collect();
        screen.set_row_bytes(0, &bytes);
        screen
    }

    #[test]
    fn an_extended_colour_holds_until_the_next_and_x80_gives_the_screen_s_back_r_ds_9() {
        let mut screen = screen_with("abcdef");
        screen.put_extended(1, ExtendedKind::Color, 0x88); // red from b
        screen.put_extended(3, ExtendedKind::Color, 0x00); // null: red goes on
        screen.put_extended(4, ExtendedKind::Color, 0x80); // normal from e
        let colors: Vec<Color> = screen.cells(CodePage::Ccsid37)[..6]
            .iter()
            .map(|c| c.style.color)
            .collect();
        let (g, r) = (Color::Green, Color::Red);
        assert_eq!(colors, [g, r, r, r, g, g]);
    }

    #[test]
    fn an_extended_primary_attribute_replaces_the_screen_s_highlighting_r_ds_9() {
        let mut screen = screen_with("æøå");
        screen.put_extended(0, ExtendedKind::Primary, 0x80 | 0x04 | 0x01); // underline, reverse
        screen.put_extended(2, ExtendedKind::Primary, 0x80 | 0x02); // high intensity
        let cells = screen.cells(CodePage::Ccsid37);
        assert!(cells[0].style.underline && cells[0].style.reverse);
        assert_eq!(cells[0].ch, 'æ');
        assert!(!cells[2].style.underline && cells[2].style.color == Color::White);
    }

    #[test]
    fn the_background_colour_hides_text_and_clear_unit_drops_the_attributes_r_ds_9() {
        let mut screen = screen_with("abc");
        screen.put_extended(1, ExtendedKind::Color, 0x81);
        assert_eq!(screen.cells(CodePage::Ccsid37)[1].ch, ' ');
        screen.clear(ScreenSize::Standard);
        screen.set_row_bytes(0, &[0x81, 0x82, 0x83]);
        assert_eq!(screen.cells(CodePage::Ccsid37)[1].ch, 'b');
    }

    #[test]
    fn an_address_past_the_end_is_ignored() {
        let mut screen = Screen::new(ScreenSize::Standard);
        screen.put_extended(99_999, ExtendedKind::Primary, 0x81);
        assert_eq!(screen.cells(CodePage::Ccsid37).len(), 1920);
    }
}
