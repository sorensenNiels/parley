//! What the display sends the host: input fields for read commands, and the query reply.

use super::datastream::SBA;
use super::input::Aid;
use super::{ReadKind, Session};
use crate::record::Opcode;
use crate::screen::{Field, ScreenSize, Shift};

const BLANK: u8 = 0x40;
const MINUS: u8 = 0x60;

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
        let with_address = matches!(kind, ReadKind::MdtFields | ReadKind::MdtFieldsAlternate);
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
            data.extend(self.field_data(field, kind != ReadKind::MdtFieldsAlternate));
        }
        self.send_record(0, Opcode::PutGet, &data);
    }

    /// A field's data as sent. Nulls become blanks, except for Read MDT Fields Alternate.
    /// A signed numeric field sends its digits without the sign position; a minus sign makes
    /// the zone of the last digit X'D'.
    fn field_data(&self, field: &Field, nulls_to_blanks: bool) -> Vec<u8> {
        let bytes = self.screen.field_bytes(field);
        let (digits, sign) = match field.format.shift() {
            Shift::SignedNumeric => bytes.split_at(bytes.len().saturating_sub(1)),
            _ => (bytes, &[][..]),
        };
        let mut out: Vec<u8> = digits
            .iter()
            .map(|&b| if b == 0 && nulls_to_blanks { BLANK } else { b })
            .collect();
        if sign == [MINUS]
            && let Some(last) = out.last_mut().filter(|b| (0xF0..=0xF9).contains(*b))
        {
            *last = 0xD0 | (*last & 0x0F);
        }
        out
    }

    /// The reply to a 5250 Query (5250 Functions Reference, "Query command"): what kind of
    /// display this is. Parley reports a plain 5250 emulator without GUI support.
    pub(super) fn send_query_reply(&mut self) {
        let (device, model, sizes): (&[u8; 4], &[u8; 2], u8) = match self.config.screen_size {
            ScreenSize::Standard => (b"3179", b"02", 0x10),
            ScreenSize::Wide => (b"3477", b"FC", 0x30),
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
            0x80, // flags
            0x06, 0x00, // controller hardware class: another 5250 emulator
            0x01, 0x01, 0x00, // controller code level
        ];
        data.extend([0; 16]); // reserved
        data.push(0x01); // a display
        data.extend(ebcdic(device));
        data.extend(ebcdic(model));
        data.extend([
            0x02, // keyboard: standard
            0x00, // extended keyboard
            0x00, // reserved
            0x00, 0x00, 0x00, 0x00, // display serial number
            0x00, 0xFF, // maximum number of input fields
            0x00, // control unit customisation
            0x00, 0x00, // reserved
            0x00, sizes, // capabilities: screen sizes (24×80, or 24×80 and 27×132)
        ]);
        data.resize(61, 0x00);
        self.send_record(0, Opcode::NoOperation, &data);
    }
}

/// Screen rows and columns go on the wire 1-based; screens have at most 132 columns.
fn one_based(n: usize) -> u8 {
    u8::try_from(n + 1).unwrap_or(u8::MAX)
}
