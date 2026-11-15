//! The 5250 data stream from the host (R-DS-1): commands, each after an ESC byte, and the
//! orders inside Write To Display. References are to IBM's "5250 Functions Reference"
//! (SA21-9247), chapter 15 "5494 data stream", IBM's "5494 Remote Control Unit Functions
//! Reference" (SC30-3533-02), chapter 12 "Display Data Streams", its release 3.0 (SC30-3533-04,
//! chapter 15) for the partial screen commands and Read MDT Immediate Alternate, and RFC 1205.

use super::{MAX_SAVED_SCREENS, ReadKind, SavedScreen, Session};
use crate::error::{Error, Result};
use crate::record::Opcode;
use crate::screen::{Field, FieldFormat, ScreenSize, SelfCheck, is_attribute};

pub(super) const ESC: u8 = 0x04;

// Commands.
const CLEAR_UNIT: u8 = 0x40;
const CLEAR_UNIT_ALTERNATE: u8 = 0x20;
const CLEAR_FORMAT_TABLE: u8 = 0x50;
const WRITE_TO_DISPLAY: u8 = 0x11;
const WRITE_ERROR_CODE: u8 = 0x21;
const WRITE_ERROR_CODE_TO_WINDOW: u8 = 0x22;
const READ_INPUT_FIELDS: u8 = 0x42;
const READ_MDT_FIELDS: u8 = 0x52;
const READ_MDT_FIELDS_ALTERNATE: u8 = 0x82;
const READ_SCREEN_IMMEDIATE: u8 = 0x62;
const READ_IMMEDIATE: u8 = 0x72;
const READ_MDT_IMMEDIATE_ALTERNATE: u8 = 0x83;
const SAVE_SCREEN: u8 = 0x02;
const SAVE_PARTIAL_SCREEN: u8 = 0x03;
pub(super) const RESTORE_SCREEN: u8 = 0x12;
const RESTORE_PARTIAL_SCREEN: u8 = 0x13;
const ROLL: u8 = 0x23;
const WRITE_STRUCTURED_FIELD: u8 = 0xF3;

// Orders in Write To Display.
const SOH: u8 = 0x01;
const RA: u8 = 0x02;
const EA: u8 = 0x03;
const TD: u8 = 0x10;
pub(super) const SBA: u8 = 0x11;
const WEA: u8 = 0x12;
const IC: u8 = 0x13;
const MC: u8 = 0x14;
const WDSF: u8 = 0x15;
const SF: u8 = 0x1D;

// Control character 2 of Write To Display and the read commands (5494 Functions Reference
// SC30-3533-02, "Write to Display Command", CC2 bits 4-7; bit 0 is the high-order bit).
const CC2_UNLOCK: u8 = 0x08;
const CC2_ALARM: u8 = 0x04;
const CC2_MESSAGE_LIGHT_OFF: u8 = 0x02;
const CC2_MESSAGE_LIGHT_ON: u8 = 0x01;

// The Roll command's control byte ("Roll Command"): bit 0 rolls down, bits 3-7 count lines.
const ROLL_DOWN: u8 = 0x80;
const ROLL_LINES: u8 = 0x1F;

// The first byte of a field format word has bits 0-1 = B'01' ("Start of Field Order"); a
// field control word does not.
const FFW_ID_MASK: u8 = 0xC0;
const FFW_ID: u8 = 0x40;

/// Marks restore data Parley made in reply to Save Screen ("PY" in EBCDIC).
pub(super) const RESTORE_MAGIC: [u8; 2] = [0xD7, 0xE8];

/// Reads a record's data, naming what is missing when it ends too soon.
struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    const fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn peek(&self) -> Option<u8> {
        self.data.get(self.pos).copied()
    }

    fn take(&mut self, n: usize, what: &str) -> Result<&'a [u8]> {
        let bytes = self
            .data
            .get(self.pos..self.pos + n)
            .ok_or_else(|| Error::data(format!("the data stream ends inside {what}")))?;
        self.pos += n;
        Ok(bytes)
    }

    fn byte(&mut self, what: &str) -> Result<u8> {
        Ok(self.take(1, what)?[0])
    }

    fn u16(&mut self, what: &str) -> Result<u16> {
        let b = self.take(2, what)?;
        Ok(u16::from_be_bytes([b[0], b[1]]))
    }

    /// The next byte, unless it starts the next command.
    fn next_order(&mut self) -> Option<u8> {
        let b = self.peek().filter(|&b| b != ESC)?;
        self.pos += 1;
        Some(b)
    }
}

/// What CC1 of Write To Display or a read command does to the fields' MDT and contents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fields {
    None,
    NonBypass,
    NonBypassModified,
    All,
}

impl Session {
    pub(super) fn process_stream(&mut self, data: &[u8]) -> Result<()> {
        let mut r = Reader::new(data);
        // Save Partial Screen's reply goes after the rest of the record (SC30-3533-04 §15.22).
        let mut partial_save = None;
        while let Some(b) = r.peek() {
            r.pos += 1;
            if b != ESC {
                return Err(Error::data(format!(
                    "expected a command (ESC), found X'{b:02X}'"
                )));
            }
            match r.byte("a command")? {
                CLEAR_UNIT => self.clear_unit(ScreenSize::Standard),
                CLEAR_UNIT_ALTERNATE => {
                    if r.byte("Clear Unit Alternate")? == 0x00 {
                        if self.config.screen_size != ScreenSize::Wide {
                            return Err(Error::unsupported("a 27×132 screen to a 24×80 session"));
                        }
                        self.clear_unit(ScreenSize::Wide);
                    } else {
                        self.clear_unit(self.screen.size());
                    }
                }
                CLEAR_FORMAT_TABLE => {
                    self.screen.clear_fields();
                    self.lock();
                }
                WRITE_TO_DISPLAY => self.write_to_display(&mut r)?,
                WRITE_ERROR_CODE => self.write_error_code(&mut r)?,
                WRITE_ERROR_CODE_TO_WINDOW => {
                    r.take(2, "Write Error Code to Window")?;
                    self.write_error_code(&mut r)?;
                }
                READ_INPUT_FIELDS => self.read_command(&mut r, ReadKind::InputFields)?,
                READ_MDT_FIELDS => self.read_command(&mut r, ReadKind::MdtFields)?,
                READ_MDT_FIELDS_ALTERNATE => {
                    self.read_command(&mut r, ReadKind::MdtFieldsAlternate)?;
                }
                READ_IMMEDIATE => self.send_fields(ReadKind::Immediate, None),
                // R-DS-7: SC30-3533-04 §15.20 says "the same as READ IMMEDIATE", which sends
                // no cursor or AID; RFC 1205 §5.3 says the same as Read MDT Fields Alternate
                // with AID X'00', as IBM's QsnReadMDTImmAlt API describes it. Parley follows
                // the RFC.
                READ_MDT_IMMEDIATE_ALTERNATE => self.send_fields(
                    ReadKind::ImmediateAlternate,
                    Some(super::Aid::from_code(0x00)),
                ),
                READ_SCREEN_IMMEDIATE => {
                    let buffer = self.screen.buffer().to_vec();
                    self.send_record(0, Opcode::NoOperation, &buffer);
                }
                SAVE_SCREEN => self.save_screen(),
                SAVE_PARTIAL_SCREEN => {
                    // Flag, top row, left column, depth, width: the 5494 ignores them and
                    // saves the whole screen, and so does Parley.
                    r.take(5, "Save Partial Screen")?;
                    partial_save = Some(self.keep_screen());
                }
                RESTORE_SCREEN => self.restore_screen(&mut r)?,
                RESTORE_PARTIAL_SCREEN => self.restore_partial_screen(&mut r)?,
                ROLL => self.roll(&mut r)?,
                WRITE_STRUCTURED_FIELD => self.write_structured_field(&mut r)?,
                other => return Err(Error::unsupported(format!("command X'{other:02X}'"))),
            }
        }
        if let Some(id) = partial_save {
            self.send_partial_save(id);
        }
        Ok(())
    }

    fn clear_unit(&mut self, size: ScreenSize) {
        self.screen.clear(size);
        self.lock();
        self.error = None;
        self.pending_read = None;
        self.insert_cursor = None;
        self.error_row = None;
        self.saved_error_line = None;
    }

    fn read_address(&self, r: &mut Reader<'_>, order: &str) -> Result<usize> {
        let row = r.byte(order)?;
        let column = r.byte(order)?;
        self.screen.address(row, column).ok_or_else(|| {
            Error::data(format!(
                "{order} to row {row} column {column}, outside the screen"
            ))
        })
    }

    fn write_to_display(&mut self, r: &mut Reader<'_>) -> Result<()> {
        let cc1 = r.byte("the Write To Display control characters")?;
        let cc2 = r.byte("the Write To Display control characters")?;
        self.apply_cc1(cc1);
        let mut address = self.screen.cursor;
        while let Some(b) = r.next_order() {
            match b {
                SOH => self.start_of_header(r)?,
                RA => {
                    let to = self.read_address(r, "Repeat to Address")?;
                    let byte = r.byte("Repeat to Address")?;
                    if to < address {
                        return Err(Error::data("Repeat to Address goes backwards"));
                    }
                    self.screen.fill(address, to, byte);
                    address = self.screen.next(to);
                }
                EA => {
                    let to = self.read_address(r, "Erase to Address")?;
                    let len = r.byte("Erase to Address")?;
                    if !(2..=5).contains(&len) {
                        return Err(Error::data(format!("Erase to Address with length {len}")));
                    }
                    r.take(usize::from(len) - 1, "Erase to Address")?;
                    if to < address {
                        return Err(Error::data("Erase to Address goes backwards"));
                    }
                    self.screen.fill(address, to, 0);
                    address = self.screen.next(to);
                }
                TD => {
                    let len = usize::from(r.u16("Transparent Data")?);
                    for &byte in r.take(len, "Transparent Data")? {
                        self.screen.put(address, byte);
                        address = self.screen.next(address);
                    }
                }
                SBA => address = self.read_address(r, "Set Buffer Address")?,
                // Extended attributes (colours, ideographic text): not shown yet.
                WEA => {
                    r.take(2, "Write Extended Attribute")?;
                }
                IC => self.insert_cursor = Some(self.read_address(r, "Insert Cursor")?),
                MC => self.screen.cursor = self.read_address(r, "Move Cursor")?,
                // Windows, menus and scroll bars: only sent to displays that report GUI support
                // in the query reply, which Parley does not.
                WDSF => {
                    let len = usize::from(r.u16("Write to Display Structured Field")?);
                    r.take(len.saturating_sub(2), "Write to Display Structured Field")?;
                }
                SF => self.start_of_field(r, &mut address)?,
                _ => {
                    self.screen.put(address, b);
                    address = self.screen.next(address);
                }
            }
        }
        self.apply_cc2(cc2);
        Ok(())
    }

    /// Start of Header (5494 Functions Reference SC30-3533-02, "Start of Header Order"): clears
    /// the format table and may move the error line. The length byte counts the bytes after it;
    /// 0 or more than 7 is a parameter error. Byte 4 (here `header[3]`) is the error row: 0 or a
    /// row past the screen means the default, the last row. Byte 1 (cursor and screen-reverse
    /// flags), byte 3 (resequencing) and bytes 5-7 (command keys that return no data, IDEA-4)
    /// are not used yet.
    fn start_of_header(&mut self, r: &mut Reader<'_>) -> Result<()> {
        let len = r.byte("Start of Header")?;
        if len == 0 || len > 7 {
            return Err(Error::data(format!("Start of Header with length {len}")));
        }
        let header = r.take(usize::from(len), "Start of Header")?;
        self.screen.clear_fields();
        // Bytes 5-7 (after the length): the command key switches, only when all are there.
        if let [_, _, _, _, a, b, c] = *header {
            self.screen.set_key_switches([a, b, c]);
        }
        let rows = self.screen.size().rows();
        self.error_row = header
            .get(3)
            .filter(|&&row| row != 0 && u16::from(row) <= rows)
            .map(|&row| usize::from(row - 1));
        Ok(())
    }

    /// Start of Field: `[FFW [FCW...]] attribute length`. Without a field format word the field
    /// is output only: its attribute is written but it does not go in the format table.
    fn start_of_field(&mut self, r: &mut Reader<'_>, address: &mut usize) -> Result<()> {
        let first = r.byte("Start of Field")?;
        let mut self_check = None;
        let (format, attribute) = if is_attribute(first) {
            (None, first)
        } else {
            if first & FFW_ID_MASK != FFW_ID {
                return Err(Error::data(format!(
                    "a field format word starting X'{first:02X}'"
                )));
            }
            let second = r.byte("Start of Field")?;
            // Field control words: of each type only the first counts (SC30-3533-04 §15.6.12.2).
            while r.peek().is_some_and(|b| !is_attribute(b)) {
                let fcw = r.take(2, "a field control word")?;
                let check = match fcw {
                    [0xB1, 0xA0] => Some(SelfCheck::Modulus10),
                    [0xB1, 0x40] => Some(SelfCheck::Modulus11),
                    _ => None,
                };
                self_check = self_check.or(check);
            }
            let attribute = r.byte("Start of Field")?;
            (
                Some(FieldFormat(u16::from_be_bytes([first, second]))),
                attribute,
            )
        };
        let len = usize::from(r.u16("Start of Field")?);
        self.screen.put(*address, attribute);
        let start = self.screen.next(*address);
        if let Some(format) = format {
            self.screen.add_field(Field {
                start,
                len,
                format,
                modified: format.modified(),
                self_check,
            });
        }
        *address = start;
        Ok(())
    }

    /// Control character 1: lock the keyboard, and reset MDT flags or null fields.
    fn apply_cc1(&mut self, cc1: u8) {
        let (reset, null) = match cc1 >> 5 {
            0 => return,
            1 => (Fields::None, Fields::None),
            2 => (Fields::NonBypass, Fields::None),
            3 => (Fields::All, Fields::None),
            4 => (Fields::None, Fields::NonBypassModified),
            5 => (Fields::NonBypass, Fields::NonBypass),
            6 => (Fields::NonBypass, Fields::NonBypassModified),
            _ => (Fields::All, Fields::NonBypass),
        };
        self.lock();
        let selected = |f: &Field, which| match which {
            Fields::None => false,
            Fields::NonBypass => f.is_input(),
            Fields::NonBypassModified => f.is_input() && f.modified,
            Fields::All => true,
        };
        for i in 0..self.screen.fields().len() {
            let Some(field) = self.screen.field(i).copied() else {
                continue;
            };
            if selected(&field, null) {
                self.screen.field_bytes_mut(&field).fill(0);
            }
            if selected(&field, reset)
                && let Some(f) = self.screen.field_mut(i)
            {
                f.modified = false;
            }
        }
    }

    /// Control character 2: message light, alarm, unlock.
    fn apply_cc2(&mut self, cc2: u8) {
        if cc2 & CC2_MESSAGE_LIGHT_OFF != 0 {
            self.message_waiting = false;
        }
        if cc2 & CC2_MESSAGE_LIGHT_ON != 0 {
            self.message_waiting = true;
        }
        if cc2 & CC2_ALARM != 0 {
            self.alarm = true;
        }
        if cc2 & CC2_UNLOCK != 0 {
            self.unlock();
        }
    }

    fn read_command(&mut self, r: &mut Reader<'_>, kind: ReadKind) -> Result<()> {
        let cc1 = r.byte("the read command's control characters")?;
        let cc2 = r.byte("the read command's control characters")?;
        self.apply_cc1(cc1);
        self.apply_cc2(cc2);
        self.pending_read = Some(kind);
        self.unlock();
        Ok(())
    }

    /// Writes a message on the error line and inhibits the keyboard until Reset.
    fn write_error_code(&mut self, r: &mut Reader<'_>) -> Result<()> {
        let row = self
            .error_row
            .unwrap_or(usize::from(self.screen.size().rows()) - 1);
        if self.saved_error_line.is_none() {
            self.saved_error_line = Some((row, self.screen.row_bytes(row).to_vec()));
        }
        let columns = usize::from(self.screen.size().columns());
        let mut address = row * columns;
        let end = address + columns;
        while let Some(b) = r.next_order() {
            if b == IC {
                self.screen.cursor = self.read_address(r, "Insert Cursor")?;
            } else if address < end {
                self.screen.put(address, b);
                address += 1;
            }
        }
        self.error = Some(super::OperatorError::Host);
        Ok(())
    }

    /// Roll: `control top bottom`; control bit 0 set rolls down, bits 3-7 are the line count.
    fn roll(&mut self, r: &mut Reader<'_>) -> Result<()> {
        let control = r.byte("Roll")?;
        let top = r.byte("Roll")?;
        let bottom = r.byte("Roll")?;
        if top == 0 || top > bottom || u16::from(bottom) > self.screen.size().rows() {
            return Err(Error::data(format!("Roll of rows {top} to {bottom}")));
        }
        let (top, bottom) = (usize::from(top - 1), usize::from(bottom - 1));
        let height = bottom - top + 1;
        let count = usize::from(control & ROLL_LINES).min(height);
        let mut rows: Vec<Vec<u8>> = (top..=bottom)
            .map(|row| self.screen.row_bytes(row).to_vec())
            .collect();
        if control & ROLL_DOWN != 0 {
            rows.rotate_right(count);
            rows[..count].iter_mut().for_each(|row| row.fill(0));
        } else {
            rows.rotate_left(count);
            rows[height - count..]
                .iter_mut()
                .for_each(|row| row.fill(0));
        }
        for (i, bytes) in rows.iter().enumerate() {
            self.screen.set_row_bytes(top + i, bytes);
        }
        Ok(())
    }

    /// Save Screen: the reply is restore data, which the host sends back later after a Restore
    /// Screen command. Parley keeps the screen itself and sends an id for it (D7).
    pub(super) fn save_screen(&mut self) {
        let id = self.keep_screen();
        let mut data = vec![ESC, RESTORE_SCREEN];
        data.extend(RESTORE_MAGIC);
        data.extend(id.to_be_bytes());
        self.send_record(0, Opcode::SaveScreen, &data);
    }

    /// Save Partial Screen's reply (R-DS-6, SC30-3533-04 §15.22, §15.24): ESC, Restore Partial
    /// Screen and a length counting itself, as the host sends it back. The manual names no
    /// opcode; RFC 1205's Save Screen opcode is used, as tn5250 does.
    fn send_partial_save(&mut self, id: u16) {
        let mut data = vec![ESC, RESTORE_PARTIAL_SCREEN, 0x00, 0x06];
        data.extend(RESTORE_MAGIC);
        data.extend(id.to_be_bytes());
        self.send_record(0, Opcode::SaveScreen, &data);
    }

    /// Keeps a copy of the screen and its state; returns its id.
    fn keep_screen(&mut self) -> u16 {
        let id = self.next_save_id;
        self.next_save_id = self.next_save_id.wrapping_add(1);
        self.saved_screens.push(SavedScreen {
            id,
            screen: self.screen.clone(),
            pending_read: self.pending_read,
            locked: self.locked,
            error_row: self.error_row,
        });
        if self.saved_screens.len() > MAX_SAVED_SCREENS {
            self.saved_screens.remove(0);
        }
        id
    }

    fn restore_screen(&mut self, r: &mut Reader<'_>) -> Result<()> {
        let data = r.take(4, "Restore Screen")?;
        self.restore(data)
    }

    /// Restore Partial Screen (R-DS-6, SC30-3533-04 §15.24): a length counting itself, then
    /// the data Save Partial Screen sent. Commands may follow it.
    fn restore_partial_screen(&mut self, r: &mut Reader<'_>) -> Result<()> {
        let len = usize::from(r.u16("Restore Partial Screen")?);
        let data = r.take(len.saturating_sub(2), "Restore Partial Screen")?;
        self.restore(data)
    }

    /// Puts back the screen saved under the id in `data` (D7): everything but the message
    /// waiting light, which the manual says is lost.
    fn restore(&mut self, data: &[u8]) -> Result<()> {
        if data.len() != 4 || data[..2] != RESTORE_MAGIC {
            return Err(Error::data("restore data that this session did not save"));
        }
        let id = u16::from_be_bytes([data[2], data[3]]);
        let saved = self
            .saved_screens
            .iter()
            .find(|s| s.id == id)
            .cloned()
            .ok_or_else(|| Error::data(format!("a restore of saved screen {id}, which is gone")))?;
        self.screen = saved.screen;
        self.pending_read = saved.pending_read;
        self.locked = saved.locked;
        self.error_row = saved.error_row;
        self.error = None;
        self.insert_cursor = None;
        self.saved_error_line = None;
        Ok(())
    }

    /// Write Structured Field: only the 5250 Query is answered; the others are for features
    /// Parley does not report in its query reply.
    fn write_structured_field(&mut self, r: &mut Reader<'_>) -> Result<()> {
        let len = usize::from(r.u16("Write Structured Field")?);
        let body = r.take(len.saturating_sub(2), "Write Structured Field")?;
        if let [0xD9, 0x70, ..] = body {
            self.send_query_reply();
        }
        Ok(())
    }
}
