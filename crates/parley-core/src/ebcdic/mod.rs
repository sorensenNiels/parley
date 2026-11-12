//! EBCDIC code pages: the host's bytes to Unicode and back (R-EBC-1).

mod tables;

use serde::{Deserialize, Serialize};

/// The code page (CCSID) a session uses for text. It must match the host job's CCSID,
/// which Parley asks for when it connects (`CODEPAGE`/`CHARSET`, RFC 4777).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum CodePage {
    #[default]
    Ccsid37,
    Ccsid277,
    Ccsid1140,
    Ccsid1142,
}

impl CodePage {
    pub const ALL: [Self; 4] = [
        Self::Ccsid37,
        Self::Ccsid277,
        Self::Ccsid1140,
        Self::Ccsid1142,
    ];

    pub const fn ccsid(self) -> u16 {
        match self {
            Self::Ccsid37 => 37,
            Self::Ccsid277 => 277,
            Self::Ccsid1140 => 1140,
            Self::Ccsid1142 => 1142,
        }
    }

    /// A name for menus, e.g. "277 Danish, Norwegian".
    pub const fn label(self) -> &'static str {
        match self {
            Self::Ccsid37 => "37 USA, Canada",
            Self::Ccsid277 => "277 Danish, Norwegian",
            Self::Ccsid1140 => "1140 USA, Canada (euro)",
            Self::Ccsid1142 => "1142 Danish, Norwegian (euro)",
        }
    }

    /// The IBM i character set (graphic character set ID) the code page belongs to.
    pub(crate) const fn character_set(self) -> u16 {
        match self {
            Self::Ccsid37 | Self::Ccsid277 => 697,
            Self::Ccsid1140 | Self::Ccsid1142 => 695,
        }
    }

    /// The IBM i keyboard type for the code page (the `KBDTYPE` of the device description).
    pub(crate) const fn keyboard_type(self) -> &'static str {
        match self {
            Self::Ccsid37 | Self::Ccsid1140 => "USB",
            Self::Ccsid277 | Self::Ccsid1142 => "DMB",
        }
    }

    const fn table(self) -> &'static [char; 192] {
        match self {
            Self::Ccsid37 => &tables::CCSID_37,
            Self::Ccsid277 => &tables::CCSID_277,
            Self::Ccsid1140 => &tables::CCSID_1140,
            Self::Ccsid1142 => &tables::CCSID_1142,
        }
    }

    /// The character an EBCDIC byte shows. Bytes below X'40' (nulls, orders and attributes)
    /// and control characters show as a space.
    pub fn to_char(self, byte: u8) -> char {
        byte.checked_sub(0x40)
            .and_then(|i| self.table().get(usize::from(i)))
            .copied()
            .unwrap_or(' ')
    }

    /// The EBCDIC byte for a character, if the code page has it.
    pub fn to_byte(self, c: char) -> Option<u8> {
        if c == ' ' {
            return Some(0x40);
        }
        let index = self.table().iter().position(|&t| t == c)?;
        u8::try_from(index + 0x40).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letters_digits_and_space_are_where_ebcdic_puts_them() {
        for page in CodePage::ALL {
            assert_eq!(page.to_char(0xC1), 'A');
            assert_eq!(page.to_char(0x81), 'a');
            assert_eq!(page.to_char(0xF0), '0');
            assert_eq!(page.to_byte(' '), Some(0x40));
            assert_eq!(page.to_byte('Z'), Some(0xE9));
        }
    }

    #[test]
    fn danish_letters_round_trip_in_ccsid_277() {
        // R-EBC-1
        let page = CodePage::Ccsid277;
        for (byte, c) in [
            (0x7B, 'Æ'),
            (0x7C, 'Ø'),
            (0x5B, 'Å'),
            (0xC0, 'æ'),
            (0x6A, 'ø'),
            (0xD0, 'å'),
        ] {
            assert_eq!(page.to_char(byte), c);
            assert_eq!(page.to_byte(c), Some(byte));
        }
    }

    #[test]
    fn the_euro_variants_have_the_euro_sign() {
        assert_eq!(CodePage::Ccsid1140.to_char(0x9F), '€');
        assert_eq!(CodePage::Ccsid1142.to_char(0x5A), '€');
        assert_eq!(CodePage::Ccsid37.to_byte('€'), None);
    }

    #[test]
    fn bytes_below_x40_show_as_spaces() {
        for byte in 0..0x40 {
            assert_eq!(CodePage::Ccsid37.to_char(byte), ' ');
        }
    }

    #[test]
    fn every_printable_character_round_trips() {
        for page in CodePage::ALL {
            for byte in 0x41..=0xFE_u8 {
                let c = page.to_char(byte);
                if c != ' ' {
                    assert_eq!(page.to_byte(c), Some(byte), "{page:?} {byte:#04x} {c:?}");
                }
            }
        }
    }
}
