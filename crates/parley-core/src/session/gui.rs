//! Write to Display Structured Field (WDSF) with IBM i's GUI-like 5250 (R-DS-10..12, D38, D57;
//! SC30-3533-04 §15.6.13): only while the session's "GUI elements" switch is on, the only time
//! the query reply claims the enhanced user interface. Windows, selection fields, scroll bars,
//! Write Data and programmable mouse buttons; grid lines (X'60', X'61') are skipped.
//!
//! The manual itself was not at hand (D57): the layouts are from IBM i's own bytes as traced in
//! IBM's TRC5250 output and the field names other emulators and Wireshark took from §15.6.13.

use super::Session;
use super::mouse::{MouseDefinition, MouseEvent};
use crate::error::{Error, Result};
use crate::screen::{
    Align, BorderChars, Caption, Choice, ChoiceColors, ChoiceState, DEFAULT_BORDER_ATTRIBUTE,
    Indicator, ScrollBar, SelectionField, SelectionKind, WindowSpec, is_attribute,
};

/// The 5250 class of structured field.
const CLASS: u8 = 0xD9;
const DEFINE_SELECTION: u8 = 0x50;
const CREATE_WINDOW: u8 = 0x51;
const UNRESTRICTED_CURSOR: u8 = 0x52;
const DEFINE_SCROLL_BAR: u8 = 0x53;
const WRITE_DATA: u8 = 0x54;
const MOUSE_BUTTONS: u8 = 0x55;
const REMOVE_SELECTION: u8 = 0x58;
const REMOVE_WINDOW: u8 = 0x59;
const REMOVE_SCROLL_BAR: u8 = 0x5B;
const REMOVE_ALL: u8 = 0x5F;
/// Create Window's minor structures (Tables 54 and 55).
const BORDER: u8 = 0x01;
const TITLE: u8 = 0x10;
/// Define Selection Field's minor structures.
const PRESENTATION: u8 = 0x01;
const CHOICE_TEXT: u8 = 0x10;
/// A minor structure meant for GUI devices (choice text flag byte 3 and presentation flags,
/// bit 0): the ones a display that claims the GUI uses.
const FOR_GUI: u8 = 0x80;

impl Session {
    /// Carries out a WDSF order's structured field, `body` being what follows its length, at
    /// screen address `address`. Without the switch on, or of a type Parley does not draw, it is
    /// skipped.
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
        let doesnt_fit =
            |what: &str| Error::data(format!("{what} at row {} that does not fit", row + 1));
        match *kind {
            CREATE_WINDOW => {
                let spec = self.window_spec(rest, row, column)?;
                self.screen
                    .create_window(spec)
                    .ok_or_else(|| doesnt_fit("a window"))?;
            }
            UNRESTRICTED_CURSOR => self.screen.unrestrict_cursor(),
            DEFINE_SELECTION => {
                if let Some(field) = selection_field(rest, address)? {
                    self.screen
                        .define_selection(field)
                        .ok_or_else(|| doesnt_fit("a selection field"))?;
                }
            }
            DEFINE_SCROLL_BAR => {
                self.screen
                    .define_scroll_bar(scroll_bar(rest, address)?)
                    .ok_or_else(|| doesnt_fit("a scroll bar"))?;
            }
            WRITE_DATA => self.write_data(rest, address)?,
            MOUSE_BUTTONS => self.mouse_buttons = mouse_buttons(rest),
            REMOVE_SELECTION => {
                self.screen.remove_selection(address);
            }
            REMOVE_WINDOW => {
                self.screen.remove_window(row, column);
            }
            REMOVE_SCROLL_BAR => {
                self.screen.remove_scroll_bar(address);
            }
            REMOVE_ALL => self.screen.remove_all_gui(),
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
        for minor in minor_structures(minors, "a Create Window")? {
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
        }
        Ok(spec)
    }

    /// Write Data (X'54', §15.6.13.9): with flag bit 0, the data goes into the entry field at
    /// `address`, from its first position, cut to fit and the rest of the field nulled.
    fn write_data(&mut self, rest: &[u8], address: usize) -> Result<()> {
        let [flags, _reserved, data @ ..] = rest else {
            return Err(Error::data("a Write Data cut short"));
        };
        if flags & 0x80 == 0 {
            return Ok(());
        }
        let Some(field) = self
            .screen
            .field_at(address)
            .and_then(|i| self.screen.field(i))
            .copied()
        else {
            return Ok(());
        };
        let bytes = self.screen.field_bytes_mut(&field);
        bytes.fill(0);
        let n = data.len().min(bytes.len());
        bytes[..n].copy_from_slice(&data[..n]);
        Ok(())
    }
}

/// The minor structures after a major structure: each its own length (at least 2) and type.
fn minor_structures<'a>(mut bytes: &'a [u8], what: &str) -> Result<Vec<&'a [u8]>> {
    let mut out = Vec::new();
    while let [len, ..] = *bytes {
        let len = usize::from(len);
        let Some(minor) = bytes.get(..len).filter(|_| len >= 2) else {
            return Err(Error::data(format!("{what} minor structure cut short")));
        };
        out.push(minor);
        bytes = &bytes[len..];
    }
    Ok(out)
}

/// Define Selection Field (X'50', §15.6.13.5): the 16 bytes after the type, two more four-byte
/// counts when it has a scroll bar (flag byte 2 bit 0; Parley draws no scroll bar in a list and
/// skips them), then the minor structures. `None` for a field meant only for a display without
/// a mouse (flag byte 1 bits 0-1 = B'01'): Parley claims one.
fn selection_field(rest: &[u8], start: usize) -> Result<Option<SelectionField>> {
    let Some((major, after)) = rest.split_first_chunk::<16>() else {
        return Err(Error::data("a Define Selection Field cut short"));
    };
    let [
        flags1,
        flags2,
        flags3,
        kind,
        gui,
        _,
        _,
        _,
        _,
        text_size,
        _rows,
        columns,
        padding,
        ..,
    ] = *major;
    let minors = if flags2 & 0x80 != 0 {
        after
            .get(8..)
            .ok_or_else(|| Error::data("a Define Selection Field cut short"))?
    } else {
        after
    };
    let kind = SelectionKind::from_code(kind)
        .ok_or_else(|| Error::data(format!("a selection field of type X'{kind:02X}'")))?;
    let mut colors = ChoiceColors::default();
    let mut choices = Vec::new();
    for minor in minor_structures(minors, "a Define Selection Field")? {
        match minor[1] {
            PRESENTATION if minor.get(2).is_some_and(|f| f & FOR_GUI != 0) => {
                apply_colors(&mut colors, minor);
            }
            CHOICE_TEXT => {
                if let Some(choice) = choice(minor)? {
                    choices.push(choice);
                }
            }
            _ => {}
        }
    }
    if flags1 & 0xC0 == 0x40 {
        return Ok(None);
    }
    Ok(Some(SelectionField {
        start,
        kind,
        indicator: Indicator::from_characteristics(gui),
        auto_enter: flags1 & 0x0C != 0,
        reset_on_unlock: flags3 & 0x80 != 0,
        keeps_cursor: flags2 & 0x04 != 0,
        modified: flags1 & 0x01 != 0,
        text_size: usize::from(text_size),
        columns: usize::from(columns),
        padding: usize::from(padding),
        colors,
        choices,
    }))
}

/// The colour display's attributes in the presentation minor structure: available, selected
/// and unavailable choices at bytes 11, 13 and 15; X'00' keeps Parley's.
fn apply_colors(colors: &mut ChoiceColors, minor: &[u8]) {
    for (slot, at) in [
        (&mut colors.available, 10),
        (&mut colors.selected, 12),
        (&mut colors.unavailable, 14),
    ] {
        if let Some(&a) = minor.get(at).filter(|&&a| is_attribute(a)) {
            *slot = a;
        }
    }
}

/// A choice text minor structure: three flag bytes, then a mnemonic offset, an AID and one or
/// two numeric selection characters as flag byte 1 says, then the text. `None` when it is not
/// meant for a GUI display.
fn choice(minor: &[u8]) -> Result<Option<Choice>> {
    let cut = || Error::data("a choice text cut short");
    let [_, _, flags1, flags2, flags3, rest @ ..] = minor else {
        return Err(cut());
    };
    if flags3 & FOR_GUI == 0 {
        return Ok(None);
    }
    let mut rest = rest;
    let mut take = |present: bool| -> Result<Option<u8>> {
        if !present {
            return Ok(None);
        }
        let (&b, tail) = rest.split_first().ok_or_else(cut)?;
        rest = tail;
        Ok(Some(b))
    };
    let mnemonic = take(flags1 & 0x08 != 0)?.map(usize::from);
    let aid = take(flags1 & 0x04 != 0)?;
    for _ in 0..match flags1 & 0x03 {
        1 => 1,
        2 => 2,
        _ => 0,
    } {
        take(true)?;
    }
    Ok(Some(Choice {
        text: rest.to_vec(),
        state: ChoiceState::from_flags(*flags1),
        mnemonic,
        aid,
        new_row: flags1 & 0x20 != 0,
        flags: *flags2,
        address: 0,
    }))
}

/// Define Scroll Bar Field (X'53', §15.6.13.7): flags, a reserved byte, the rows (or columns)
/// to scroll through and the slider's position, each four bytes, and the bar's size.
fn scroll_bar(rest: &[u8], start: usize) -> Result<ScrollBar> {
    let Some([flags, _, t0, t1, t2, t3, p0, p1, p2, p3, size]) = rest.first_chunk::<11>().copied()
    else {
        return Err(Error::data("a Define Scroll Bar Field cut short"));
    };
    Ok(ScrollBar {
        start,
        horizontal: flags & 0x80 != 0,
        total: u32::from_be_bytes([t0, t1, t2, t3]),
        position: u32::from_be_bytes([p0, p1, p2, p3]),
        size: usize::from(size),
        move_cursor: flags & 0x40 != 0,
        modified: flags & 0x01 != 0,
    })
}

/// Programmable Mouse Buttons (X'55', §15.6.13.10): two reserved bytes, then four bytes a
/// definition: flags, the first and second mouse events, and the AID. Unknown events and a
/// short last entry are skipped.
fn mouse_buttons(rest: &[u8]) -> Vec<MouseDefinition> {
    rest.get(2..)
        .unwrap_or_default()
        .as_chunks::<4>()
        .0
        .iter()
        .filter_map(|d| {
            Some(MouseDefinition {
                two_events: d[0] & 0x80 != 0,
                move_cursor: d[0] & 0x40 != 0,
                first: MouseEvent::from_code(d[1])?,
                second: MouseEvent::from_code(d[2]),
                aid: d[3],
            })
        })
        .collect()
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
