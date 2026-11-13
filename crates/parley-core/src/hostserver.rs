//! The IBM i host servers' records (R-SQL-1, D19), sans-I/O as the 5250 engine (D4): what the
//! sign-on server and the database server send and take.
//!
//! ```text
//! 0-3    record length, including this header
//! 4-5    header id (0)
//! 6-7    server id (X'E009' sign-on, X'E004' database)
//! 8-11   CS instance
//! 12-15  correlation id
//! 16-17  template length
//! 18-19  request or reply id
//! 20-    template, then parameters: length (4, including itself), code point (2), data
//! ```
//!
//! IBM publishes no reference for these records. The layout above is the one in the replies
//! `make probe-sql` recorded from two IBM i systems (R-NET-8; the fixtures in the tests), and
//! every reading is bounds-checked: a record that does not hold together is an error.

pub mod portmap;
pub mod signon;
pub mod substitute;

use crate::error::{Error, Result};

/// Bytes in a record's header.
pub const HEADER_LEN: usize = 20;

/// The largest record Parley takes: far above any reply it asks for, small enough that a
/// corrupt length cannot make it wait for or hold gigabytes.
pub const MAX_RECORD_LEN: usize = 64 * 1024 * 1024;

/// Bytes in a parameter's length and code point.
const PARAM_HEADER_LEN: usize = 6;

/// Which host server a record is for or from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerId {
    /// The sign-on server (`as-signon`, QZSOSIGN).
    SignOn,
    /// The database server (`as-database`, QZDASOINIT).
    Database,
}

impl ServerId {
    const fn code(self) -> u16 {
        match self {
            Self::SignOn => 0xE009,
            Self::Database => 0xE004,
        }
    }

    fn from_code(code: u16) -> Result<Self> {
        match code {
            0xE009 => Ok(Self::SignOn),
            0xE004 => Ok(Self::Database),
            other => Err(Error::host_server(format!(
                "a record from server {other:#06X}, which Parley does not talk to"
            ))),
        }
    }
}

/// One parameter: a code point and its data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Param {
    pub code_point: u16,
    pub data: Vec<u8>,
}

impl Param {
    pub fn new(code_point: u16, data: impl Into<Vec<u8>>) -> Self {
        Self {
            code_point,
            data: data.into(),
        }
    }
}

/// A record without its lengths, which [`Record::encode`] works out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    pub server: ServerId,
    pub cs_instance: u32,
    pub correlation: u32,
    /// The request id in a request, the reply id in a reply.
    pub id: u16,
    pub template: Vec<u8>,
    pub params: Vec<Param>,
}

impl Record {
    /// A request to `server` with no template.
    pub const fn request(server: ServerId, id: u16, params: Vec<Param>) -> Self {
        Self {
            server,
            cs_instance: 0,
            correlation: 0,
            id,
            template: Vec::new(),
            params,
        }
    }

    /// The first parameter with `code_point`.
    pub fn param(&self, code_point: u16) -> Option<&[u8]> {
        self.params
            .iter()
            .find(|p| p.code_point == code_point)
            .map(|p| p.data.as_slice())
    }

    /// The record as sent. Fails only when a part is too long for its length field.
    pub fn encode(&self) -> Result<Vec<u8>> {
        let too_long = |what: &str| Error::host_server(format!("{what} is too long to send"));
        let template_len =
            u16::try_from(self.template.len()).map_err(|_| too_long("a template"))?;
        let mut out = Vec::with_capacity(HEADER_LEN + self.template.len());
        out.extend_from_slice(&[0; 4]); // the length, filled in below
        out.extend_from_slice(&0u16.to_be_bytes()); // header id
        out.extend_from_slice(&self.server.code().to_be_bytes());
        out.extend_from_slice(&self.cs_instance.to_be_bytes());
        out.extend_from_slice(&self.correlation.to_be_bytes());
        out.extend_from_slice(&template_len.to_be_bytes());
        out.extend_from_slice(&self.id.to_be_bytes());
        out.extend_from_slice(&self.template);
        for param in &self.params {
            let length = u32::try_from(PARAM_HEADER_LEN + param.data.len())
                .map_err(|_| too_long("a parameter"))?;
            out.extend_from_slice(&length.to_be_bytes());
            out.extend_from_slice(&param.code_point.to_be_bytes());
            out.extend_from_slice(&param.data);
        }
        let length = u32::try_from(out.len()).map_err(|_| too_long("a record"))?;
        out.get_mut(..4)
            .expect("the header was written first")
            .copy_from_slice(&length.to_be_bytes());
        Ok(out)
    }

    /// Reads one whole record, as [`record_length`] delimits it.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        let header = bytes
            .get(..HEADER_LEN)
            .ok_or_else(|| Error::host_server(format!("a record of {} bytes", bytes.len())))?;
        let length = u32_at(header, 0)? as usize;
        if length != bytes.len() {
            return Err(Error::host_server(format!(
                "a record that says it has {length} bytes but has {}",
                bytes.len()
            )));
        }
        let server = ServerId::from_code(u16_at(header, 6)?)?;
        let template_end = HEADER_LEN + usize::from(u16_at(header, 16)?);
        let template = bytes
            .get(HEADER_LEN..template_end)
            .ok_or_else(|| Error::host_server("a template longer than its record".to_owned()))?;
        Ok(Self {
            server,
            cs_instance: u32_at(header, 8)?,
            correlation: u32_at(header, 12)?,
            id: u16_at(header, 18)?,
            template: template.to_vec(),
            params: decode_params(bytes.get(template_end..).unwrap_or_default())?,
        })
    }
}

/// The parameters after the template: length, code point, data, to the end of the record.
fn decode_params(mut bytes: &[u8]) -> Result<Vec<Param>> {
    let mut params = Vec::new();
    while !bytes.is_empty() {
        let length = u32_at(bytes, 0)? as usize;
        let code_point = u16_at(bytes, 4)?;
        if length < PARAM_HEADER_LEN {
            return Err(Error::host_server(format!(
                "parameter {code_point:#06X} with a length of {length}"
            )));
        }
        let data = bytes.get(PARAM_HEADER_LEN..length).ok_or_else(|| {
            Error::host_server(format!(
                "parameter {code_point:#06X} running past the end of its record"
            ))
        })?;
        params.push(Param::new(code_point, data));
        bytes = bytes.get(length..).unwrap_or_default();
    }
    Ok(params)
}

/// The length of the record that starts `buffer`, once its first 4 bytes are there; an error
/// when that length cannot be a record.
pub fn record_length(buffer: &[u8]) -> Result<Option<usize>> {
    let Some(bytes) = buffer.get(..4) else {
        return Ok(None);
    };
    let length = u32_at(bytes, 0)? as usize;
    if !(HEADER_LEN..=MAX_RECORD_LEN).contains(&length) {
        return Err(Error::host_server(format!(
            "a record that says it has {length} bytes"
        )));
    }
    Ok(Some(length))
}

pub(crate) fn u16_at(bytes: &[u8], at: usize) -> Result<u16> {
    bytes
        .get(at..at + 2)
        .and_then(|b| b.try_into().ok())
        .map(u16::from_be_bytes)
        .ok_or_else(|| Error::host_server(format!("a record that ends before byte {}", at + 2)))
}

pub(crate) fn u32_at(bytes: &[u8], at: usize) -> Result<u32> {
    bytes
        .get(at..at + 4)
        .and_then(|b| b.try_into().ok())
        .map(u32::from_be_bytes)
        .ok_or_else(|| Error::host_server(format!("a record that ends before byte {}", at + 4)))
}

#[cfg(test)]
pub(crate) mod fixtures {
    /// PUB400's sign-on server answering an exchange of attributes (IBM i 7.5, password
    /// level 3), recorded by `make probe-sql` (R-NET-8).
    pub const PUB400_EXCHANGE_ATTRIBUTES: [u8; 94] = [
        0x00, 0x00, 0x00, 0x5e, 0x00, 0x00, 0xe0, 0x09, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x04, 0xf0, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0a, 0x11, 0x01,
        0x00, 0x07, 0x05, 0x00, 0x00, 0x00, 0x00, 0x08, 0x11, 0x02, 0x00, 0x0f, 0x00, 0x00, 0x00,
        0x0e, 0x11, 0x03, 0x25, 0x32, 0xdc, 0x13, 0x51, 0x5d, 0x34, 0x49, 0x00, 0x00, 0x00, 0x07,
        0x11, 0x19, 0x03, 0x00, 0x00, 0x00, 0x1f, 0x11, 0x1f, 0x00, 0x00, 0x00, 0x00, 0xf7, 0xf2,
        0xf2, 0xf9, 0xf0, 0xf3, 0x61, 0xd8, 0xe4, 0xe2, 0xc5, 0xd9, 0x61, 0xd8, 0xe9, 0xe2, 0xd6,
        0xe2, 0xc9, 0xc7, 0xd5,
    ];

    /// The owner's IBM i answering the same (IBM i 7.3, password level 0).
    pub const OWNER_EXCHANGE_ATTRIBUTES: [u8; 94] = [
        0x00, 0x00, 0x00, 0x5e, 0x00, 0x00, 0xe0, 0x09, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x04, 0xf0, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0a, 0x11, 0x01,
        0x00, 0x07, 0x03, 0x00, 0x00, 0x00, 0x00, 0x08, 0x11, 0x02, 0x00, 0x0a, 0x00, 0x00, 0x00,
        0x0e, 0x11, 0x03, 0xa1, 0x04, 0xba, 0x38, 0xf8, 0xd5, 0x7a, 0x5d, 0x00, 0x00, 0x00, 0x07,
        0x11, 0x19, 0x00, 0x00, 0x00, 0x00, 0x1f, 0x11, 0x1f, 0x00, 0x00, 0x00, 0x00, 0xf7, 0xf2,
        0xf0, 0xf2, 0xf7, 0xf4, 0x61, 0xd8, 0xe4, 0xe2, 0xc5, 0xd9, 0x61, 0xd8, 0xe9, 0xe2, 0xd6,
        0xe2, 0xc9, 0xc7, 0xd5,
    ];
    /// The DES sign-on request the owner's IBM i took (R-NET-9), with the substitute zeroed as
    /// recorded and the user replaced by PARLEY.
    pub const OWNER_SIGN_ON_REQUEST: [u8; 61] = [
        0x00, 0x00, 0x00, 0x3d, 0x00, 0x00, 0xe0, 0x09, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x01, 0x70, 0x04, 0x01, 0x00, 0x00, 0x00, 0x0a, 0x11, 0x13, 0x00, 0x00, 0x34,
        0xb0, 0x00, 0x00, 0x00, 0x0e, 0x11, 0x05, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x10, 0x11, 0x04, 0xd7, 0xc1, 0xd9, 0xd3, 0xc5, 0xe8, 0x40, 0x40, 0x40,
        0x40,
    ];

    /// The owner's IBM i taking a DES sign-on (return code 0, server CCSID 277), recorded by
    /// `make probe-sql … --sign-on` (R-NET-9); parameter 0x110B, not yet understood, is zeroed.
    pub const OWNER_SIGN_ON_REPLY: [u8; 113] = [
        0x00, 0x00, 0x00, 0x71, 0x00, 0x00, 0xe0, 0x09, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x04, 0xf0, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0e, 0x11, 0x06,
        0x07, 0xea, 0x0b, 0x0d, 0x09, 0x2b, 0x07, 0x00, 0x00, 0x00, 0x00, 0x0e, 0x11, 0x07, 0x07,
        0xea, 0x0b, 0x0d, 0x07, 0x25, 0x23, 0x00, 0x00, 0x00, 0x00, 0x0e, 0x11, 0x08, 0x07, 0xea,
        0x0c, 0x11, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x08, 0x11, 0x09, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x08, 0x11, 0x0a, 0x00, 0x03, 0x00, 0x00, 0x00, 0x07, 0x11, 0x0e, 0xf3, 0x00,
        0x00, 0x00, 0x0e, 0x11, 0x0b, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x0a, 0x11, 0x14, 0x00, 0x00, 0x01, 0x15,
    ];
}

#[cfg(test)]
mod tests {
    use super::fixtures::{OWNER_EXCHANGE_ATTRIBUTES, PUB400_EXCHANGE_ATTRIBUTES};
    use super::*;

    #[test]
    fn a_recorded_sign_on_reply_reads_into_its_parameters_r_sql_1() {
        for (fixture, release, level) in [
            (PUB400_EXCHANGE_ATTRIBUTES, [0, 7, 5, 0], 3),
            (OWNER_EXCHANGE_ATTRIBUTES, [0, 7, 3, 0], 0),
        ] {
            let record = Record::decode(&fixture).expect("a valid record");
            assert_eq!(record.server, ServerId::SignOn);
            assert_eq!(record.id, 0xF003);
            assert_eq!(record.template, [0, 0, 0, 0], "return code 0");
            let codes: Vec<u16> = record.params.iter().map(|p| p.code_point).collect();
            assert_eq!(codes, [0x1101, 0x1102, 0x1103, 0x1119, 0x111F]);
            assert_eq!(record.param(0x1101), Some(&release[..]));
            assert_eq!(record.param(0x1119), Some(&[level][..]));
            assert_eq!(record.param(0x1103).map(<[u8]>::len), Some(8), "the seed");
        }
    }

    #[test]
    fn a_record_encodes_and_decodes_to_itself_r_sql_1() {
        let record = Record {
            server: ServerId::Database,
            cs_instance: 7,
            correlation: 42,
            id: 0x1803,
            template: vec![0x80, 0, 0, 0],
            params: vec![
                Param::new(0x3807, "æøå".as_bytes()),
                Param::new(0x3801, Vec::new()),
            ],
        };
        let bytes = record.encode().expect("short enough");
        assert_eq!(
            u32_at(&bytes, 0).ok(),
            Some(u32::try_from(bytes.len()).expect("small"))
        );
        assert_eq!(Record::decode(&bytes).expect("valid"), record);
    }

    #[test]
    fn a_request_without_a_template_is_header_then_parameters_r_sql_1() {
        let request = Record::request(
            ServerId::SignOn,
            0x7003,
            vec![Param::new(0x1101, [0, 0, 0, 1])],
        );
        assert_eq!(
            request.encode().expect("short"),
            [
                0, 0, 0, 30, 0, 0, 0xE0, 0x09, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x70,
                0x03, // header
                0, 0, 0, 10, 0x11, 0x01, 0, 0, 0, 1, // client version
            ]
        );
    }

    #[test]
    fn broken_records_are_errors_not_panics_r_sql_1() {
        let good = PUB400_EXCHANGE_ATTRIBUTES;
        let mut wrong_length = good;
        wrong_length[3] = 0x5F;
        let mut past_end = good.to_vec();
        past_end.truncate(good.len() - 3);
        past_end[3] = u8::try_from(past_end.len()).expect("small");
        let mut long_template = good;
        long_template[17] = 0xFF;
        let mut short_param = good;
        short_param[27] = 0x02; // the first parameter says 2 bytes
        let mut unknown_server = good;
        unknown_server[7] = 0x01;
        for (name, bytes) in [
            ("empty", &[][..]),
            ("header only in part", &good[..12]),
            ("length not the record's", &wrong_length[..]),
            ("parameter past the end", &past_end[..]),
            ("template past the end", &long_template[..]),
            ("parameter shorter than its header", &short_param[..]),
            ("unknown server", &unknown_server[..]),
        ] {
            assert!(Record::decode(bytes).is_err(), "{name}");
        }
    }

    #[test]
    fn the_length_of_a_record_is_known_from_four_bytes_r_sql_1() {
        assert_eq!(record_length(&[0, 0]).ok(), Some(None));
        assert_eq!(record_length(&[0, 0, 0, 0x5e, 1]).ok(), Some(Some(94)));
        assert!(
            record_length(&[0, 0, 0, 3]).is_err(),
            "shorter than a header"
        );
        assert!(
            record_length(&[0xFF, 0xFF, 0xFF, 0xFF]).is_err(),
            "absurdly long"
        );
    }
}
