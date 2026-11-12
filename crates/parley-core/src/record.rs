//! The 5250 record header (RFC 1205 section 3): a GDS header in front of every data stream.
//!
//! ```text
//! 0-1  record length, including these two bytes
//! 2-3  record type X'12A0' (GDS)
//! 4-5  reserved
//! 6    variable header length (4)
//! 7-8  flags
//! 9    opcode
//! 10-  data
//! ```

use crate::error::{Error, Result};

const RECORD_TYPE: [u8; 2] = [0x12, 0xA0];
const HEADER_LEN: usize = 10;

/// Flags in byte 7 of the header.
pub mod flags {
    /// The data stream had an error (a negative response).
    pub const ERROR: u8 = 0x80;
    /// The operator pressed Attention.
    pub const ATTENTION: u8 = 0x40;
    /// The operator pressed System Request.
    pub const SYSTEM_REQUEST: u8 = 0x04;
}

/// What the host asks the display to do with a record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Opcode {
    NoOperation,
    Invite,
    OutputOnly,
    PutGet,
    SaveScreen,
    RestoreScreen,
    ReadImmediate,
    ReadScreen,
    CancelInvite,
    MessageLightOn,
    MessageLightOff,
}

impl Opcode {
    pub const fn code(self) -> u8 {
        match self {
            Self::NoOperation => 0x00,
            Self::Invite => 0x01,
            Self::OutputOnly => 0x02,
            Self::PutGet => 0x03,
            Self::SaveScreen => 0x04,
            Self::RestoreScreen => 0x05,
            Self::ReadImmediate => 0x06,
            Self::ReadScreen => 0x08,
            Self::CancelInvite => 0x0A,
            Self::MessageLightOn => 0x0B,
            Self::MessageLightOff => 0x0C,
        }
    }

    fn from_code(code: u8) -> Option<Self> {
        Some(match code {
            0x00 => Self::NoOperation,
            0x01 => Self::Invite,
            0x02 => Self::OutputOnly,
            0x03 => Self::PutGet,
            0x04 => Self::SaveScreen,
            0x05 => Self::RestoreScreen,
            0x06 => Self::ReadImmediate,
            0x08 => Self::ReadScreen,
            0x0A => Self::CancelInvite,
            0x0B => Self::MessageLightOn,
            0x0C => Self::MessageLightOff,
            _ => return None,
        })
    }
}

/// A record from the host, borrowing its data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Record<'a> {
    pub flags: u8,
    pub opcode: Opcode,
    pub data: &'a [u8],
}

pub fn parse(bytes: &[u8]) -> Result<Record<'_>> {
    if bytes.len() < HEADER_LEN {
        return Err(Error::data(format!(
            "a {}-byte record is too short",
            bytes.len()
        )));
    }
    if bytes.get(2..4) != Some(RECORD_TYPE.as_slice()) {
        return Err(Error::data("the record type is not X'12A0'"));
    }
    let variable_len = usize::from(bytes[6]);
    let data_start = 6 + variable_len;
    if variable_len < 4 || data_start > bytes.len() {
        return Err(Error::data("the record header length is wrong"));
    }
    let opcode = Opcode::from_code(bytes[9])
        .ok_or_else(|| Error::data(format!("unknown record opcode X'{:02X}'", bytes[9])))?;
    Ok(Record {
        flags: bytes[7],
        opcode,
        data: &bytes[data_start..],
    })
}

pub fn build(flags: u8, opcode: Opcode, data: &[u8]) -> Vec<u8> {
    // Records hold one screen's worth of data at most, far below 64 KiB.
    let len = u16::try_from(HEADER_LEN + data.len()).unwrap_or(u16::MAX);
    let mut out = Vec::with_capacity(HEADER_LEN + data.len());
    out.extend(len.to_be_bytes());
    out.extend(RECORD_TYPE);
    out.extend([0, 0, 4, flags, 0, opcode.code()]);
    out.extend(data);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_built_record_parses_back() {
        let bytes = build(flags::ATTENTION, Opcode::PutGet, &[1, 2, 3]);
        assert_eq!(bytes[..10], [0, 13, 0x12, 0xA0, 0, 0, 4, 0x40, 0, 3]);
        let record = parse(&bytes).expect("valid record");
        assert_eq!(
            record,
            Record {
                flags: 0x40,
                opcode: Opcode::PutGet,
                data: &[1, 2, 3]
            }
        );
    }

    #[test]
    fn short_and_foreign_records_are_errors() {
        assert!(parse(&[0, 4, 0x12, 0xA0]).is_err());
        assert!(parse(&[0, 10, 0x12, 0xA1, 0, 0, 4, 0, 0, 3]).is_err());
        assert!(parse(&[0, 10, 0x12, 0xA0, 0, 0, 4, 0, 0, 0x07]).is_err());
        assert!(parse(&[0, 10, 0x12, 0xA0, 0, 0, 9, 0, 0, 3]).is_err());
    }
}
