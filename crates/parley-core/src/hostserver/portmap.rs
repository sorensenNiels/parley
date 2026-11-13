//! The port mapper (port 449, R-SQL-1): asked a service's name in ASCII, it answers `+` and
//! the port in 4 bytes, or `-` when it does not know the service. Seen in `make probe-sql`'s
//! replies from two IBM i systems (R-NET-8).

use crate::error::{Error, Result};

/// The port mapper's own port.
pub const PORT: u16 = 449;

/// A host server Parley looks up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Service {
    SignOn,
    Database,
}

impl Service {
    /// The name the port mapper knows it by; the TLS one ends in `-s`.
    pub const fn name(self, tls: bool) -> &'static str {
        match (self, tls) {
            (Self::SignOn, false) => "as-signon",
            (Self::SignOn, true) => "as-signon-s",
            (Self::Database, false) => "as-database",
            (Self::Database, true) => "as-database-s",
        }
    }

    /// Where it usually listens, for when the port mapper cannot be asked.
    pub const fn usual_port(self, tls: bool) -> u16 {
        match (self, tls) {
            (Self::SignOn, false) => 8476,
            (Self::SignOn, true) => 9476,
            (Self::Database, false) => 8471,
            (Self::Database, true) => 9471,
        }
    }
}

/// What to send the port mapper.
pub fn request(service: Service, tls: bool) -> &'static [u8] {
    service.name(tls).as_bytes()
}

/// The port mapper's answer: `Some(port)`, `None` when it does not know the service, or an error
/// for anything else. Call it once [`reply_complete`] says so or the connection has closed.
pub fn parse_reply(reply: &[u8]) -> Result<Option<u16>> {
    match reply.first() {
        Some(b'-') => Ok(None),
        Some(b'+') => {
            let port = reply
                .get(1..5)
                .and_then(|b| b.try_into().ok())
                .map(u32::from_be_bytes)
                .ok_or_else(|| {
                    Error::host_server("a short reply from the port mapper".to_owned())
                })?;
            u16::try_from(port)
                .ok()
                .filter(|&p| p != 0)
                .map(Some)
                .ok_or_else(|| Error::host_server(format!("the port mapper named port {port}")))
        }
        _ => Err(Error::host_server(format!(
            "an unexpected reply from the port mapper: {reply:02X?}"
        ))),
    }
}

/// Whether `reply` is all the port mapper will send.
pub fn reply_complete(reply: &[u8]) -> bool {
    reply.first() == Some(&b'-') || reply.len() >= 5
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_request_is_the_service_name_in_ascii_r_sql_1() {
        assert_eq!(request(Service::Database, true), b"as-database-s");
        assert_eq!(request(Service::SignOn, false), b"as-signon");
    }

    #[test]
    fn a_plus_and_four_bytes_name_the_port_r_sql_1() {
        // Both probed systems named 9471 for as-database-s and 8471 for as-database.
        assert_eq!(
            parse_reply(&[b'+', 0, 0, 0x24, 0xFF]).ok(),
            Some(Some(9471))
        );
        assert_eq!(
            parse_reply(&[b'+', 0, 0, 0x21, 0x17]).ok(),
            Some(Some(8471))
        );
        assert!(reply_complete(&[b'+', 0, 0, 0x24, 0xFF]));
    }

    #[test]
    fn a_minus_means_the_service_is_not_known_r_sql_1() {
        assert_eq!(parse_reply(b"-").ok(), Some(None));
        assert!(reply_complete(b"-"));
    }

    #[test]
    fn short_odd_and_impossible_replies_are_errors_r_sql_1() {
        assert!(!reply_complete(&[b'+', 0, 0]));
        for reply in [
            &[][..],
            &[b'+', 0, 0][..],
            &[b'?', 0, 0, 0, 1][..],
            &[b'+', 0, 1, 0, 0][..], // 65536
            &[b'+', 0, 0, 0, 0][..],
        ] {
            assert!(parse_reply(reply).is_err(), "{reply:02X?}");
        }
    }
}
