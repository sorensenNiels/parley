//! What the display sends the host: input fields for read commands, and the query reply.

use super::datastream::SBA;
use super::input::Aid;
use super::{ReadKind, Session};
use crate::record::Opcode;
use crate::screen::{BLANK, Field, MINUS, ScreenSize, negative_zone};

// Device capability byte 1 of the query reply (table 12-39; bit 0 is the high-order bit).
const SIZE_27X132: u8 = 0x20;
const SIZE_24X80: u8 = 0x10;
const COLOUR: u8 = 0x01;

impl Session {
    /// The reply to a read command: cursor row and column and the AID (not for Read Immediate),
    /// then fields. Read MDT Fields sends the modified fields, each after a Set Buffer Address;
    /// Read Input Fields sends every field, without addresses, if any was modified.
    pub(super) fn send_fields(&mut self, kind: ReadKind, aid: Option<Aid>) {
        let mut data = Vec::new();
        if let Some(aid) = aid {
            let (row, column) = self.screen.row_column(self.screen.cursor);
            data.extend([one_based(row), one_based(column), aid.code()]);
        }
        let any_modified = self.screen.fields().iter().any(|f| f.modified);
        let with_address = matches!(
            kind,
            ReadKind::MdtFields | ReadKind::MdtFieldsAlternate | ReadKind::ImmediateAlternate
        );
        let keep_nulls = matches!(
            kind,
            ReadKind::MdtFieldsAlternate | ReadKind::ImmediateAlternate
        );
        for field in self.screen.fields() {
            let include = if with_address {
                field.modified
            } else {
                any_modified
            };
            if !include {
                continue;
            }
            if with_address {
                let (row, column) = self.screen.row_column(field.start);
                data.extend([SBA, one_based(row), one_based(column)]);
            }
            data.extend(self.field_data(field, !keep_nulls));
        }
        self.send_record(0, Opcode::PutGet, &data);
    }

    /// A field's data as sent (5494 Functions Reference SC30-3533-02, "READ INPUT FIELDS
    /// Command", "READ MDT FIELDS Command", "READ MDT ALTERNATE Command"). Nulls become blanks,
    /// except for Read MDT Fields Alternate, which keeps leading and embedded nulls and strips
    /// trailing ones. The manual's "converts all null fields to blanks" is read as every null
    /// position, as other emulators do; its "Null data is suppressed" under Read MDT Fields is
    /// read the same way, since nulls staying nulls is what sets Read MDT Alternate apart.
    /// A signed numeric field sends its digits without the sign position; a minus sign makes
    /// the zone of the last digit X'D'. The manual does not say what happens when that position
    /// is not a digit (an empty field with a minus); Parley leaves it as it is.
    fn field_data(&self, field: &Field, nulls_to_blanks: bool) -> Vec<u8> {
        let bytes = self.screen.field_bytes(field);
        // A signed numeric field's sign position is the one past its data end.
        let (digits, sign) = bytes.split_at((field.data_end() - field.start).min(bytes.len()));
        let mut out: Vec<u8> = digits
            .iter()
            .map(|&b| if b == 0 && nulls_to_blanks { BLANK } else { b })
            .collect();
        if !nulls_to_blanks {
            let kept = out.iter().rposition(|&b| b != 0).map_or(0, |i| i + 1);
            out.truncate(kept);
        }
        if sign == [MINUS]
            && let Some(last) = out.last_mut()
            && let Some(negative) = negative_zone(*last)
        {
            *last = negative;
        }
        out
    }

    /// The reply to a 5250 Query (5494 Functions Reference SC30-3533-02, "5250 QUERY Command",
    /// tables 12-38 and 12-39): what kind of display this is. Parley reports a plain colour 5250
    /// emulator without GUI support (IDEA-5) that claims only Read MDT Immediate Alternate
    /// (R-DS-7) in device capability byte 0.
    pub(super) fn send_query_reply(&mut self) {
        // The model is 3 bytes (table 12-39). "FC " for the 3477 is left-aligned and blank-padded
        // as tn5250 sends it; the manual gives no example of a model with letters.
        let (device, model, sizes): (&[u8; 4], &[u8; 3], u8) = match self.config.screen_size {
            ScreenSize::Standard => (b"3179", b"002", SIZE_24X80),
            ScreenSize::Wide => (b"3477", b"FC ", SIZE_24X80 | SIZE_27X132),
        };
        let page = self.code_page();
        let ebcdic = |text: &[u8]| -> Vec<u8> {
            text.iter()
                .map(|&c| page.to_byte(char::from(c)).unwrap_or(0))
                .collect()
        };
        let mut data = vec![
            0x00, 0x00, // cursor row and column
            0x88, // AID: inbound write structured field
            0x00, 0x3A, // length of the query reply, from here
            0xD9, 0x70, // class and type: 5250 query
            0x80, // flags: a query response
            0x06,
            0x00, // controller hardware class: another 5250 emulator (a 5494 says X'0043')
            0x01, 0x01, 0x00, // controller code level
        ];
        data.extend([0; 16]); // reserved
        data.push(0x01); // a display
        data.extend(ebcdic(device));
        data.extend(ebcdic(model));
        data.extend([
            0x02, // keyboard: standard
            0x00, // extended keyboard
            0x00, // PC keyboard
            0x00,
            0x00,
            0x00,
            0x00, // display serial number
            0x01,
            0x00, // maximum number of input fields: 256
            0x00,
            0x00, // control unit customisation
            0x00, // reserved
            // Device capabilities byte 0 (SC30-3533-04 §15.27.2): only bit 7, Read MDT
            // Immediate Alternate (R-DS-7); Parley claims none of the other optional commands.
            0x01,
            sizes | COLOUR, // byte 1: screen sizes and colour
        ]);
        data.resize(61, 0x00); // device capability bytes 2-11
        self.send_record(0, Opcode::NoOperation, &data);
    }
}

/// Screen rows and columns go on the wire 1-based; screens have at most 132 columns.
fn one_based(n: usize) -> u8 {
    u8::try_from(n + 1).unwrap_or(u8::MAX)
}
