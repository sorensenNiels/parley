//! EBCDIC code pages: the host's bytes to Unicode and back (R-EBC-1).

mod tables;

use serde::{Deserialize, Serialize};

/// The code page (CCSID) a session uses for text. It must match the host job's CCSID,
/// which Parley asks for when it connects (`CODEPAGE`/`CHARSET`, RFC 4777).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum CodePage {
    #[default]
    Ccsid37,
    Ccsid273,
    Ccsid277,
    Ccsid285,
    Ccsid297,
    Ccsid500,
    Ccsid871,
    Ccsid1140,
    Ccsid1141,
    Ccsid1142,
    Ccsid1146,
    Ccsid1147,
    Ccsid1148,
    Ccsid1149,
}

impl CodePage {
    pub const ALL: [Self; 14] = [
        Self::Ccsid37,
        Self::Ccsid273,
        Self::Ccsid277,
        Self::Ccsid285,
        Self::Ccsid297,
        Self::Ccsid500,
        Self::Ccsid871,
        Self::Ccsid1140,
        Self::Ccsid1141,
        Self::Ccsid1142,
        Self::Ccsid1146,
        Self::Ccsid1147,
        Self::Ccsid1148,
        Self::Ccsid1149,
    ];

    pub const fn ccsid(self) -> u16 {
        match self {
            Self::Ccsid37 => 37,
            Self::Ccsid273 => 273,
            Self::Ccsid277 => 277,
            Self::Ccsid285 => 285,
            Self::Ccsid297 => 297,
            Self::Ccsid500 => 500,
            Self::Ccsid871 => 871,
            Self::Ccsid1140 => 1140,
            Self::Ccsid1141 => 1141,
            Self::Ccsid1142 => 1142,
            Self::Ccsid1146 => 1146,
            Self::Ccsid1147 => 1147,
            Self::Ccsid1148 => 1148,
            Self::Ccsid1149 => 1149,
        }
    }

    /// A name for menus, e.g. "277 Danish, Norwegian".
    pub const fn label(self) -> &'static str {
        match self {
            Self::Ccsid37 => "37 USA, Canada",
            Self::Ccsid273 => "273 Germany, Austria",
            Self::Ccsid277 => "277 Danish, Norwegian",
            Self::Ccsid285 => "285 United Kingdom",
            Self::Ccsid297 => "297 France",
            Self::Ccsid500 => "500 International",
            Self::Ccsid871 => "871 Iceland",
            Self::Ccsid1140 => "1140 USA, Canada (euro)",
            Self::Ccsid1141 => "1141 Germany, Austria (euro)",
            Self::Ccsid1142 => "1142 Danish, Norwegian (euro)",
            Self::Ccsid1146 => "1146 United Kingdom (euro)",
            Self::Ccsid1147 => "1147 France (euro)",
            Self::Ccsid1148 => "1148 International (euro)",
            Self::Ccsid1149 => "1149 Iceland (euro)",
        }
    }

    /// The IBM i character set (graphic character set ID) the code page belongs to: 697 for the
    /// Latin-1 pages, 695 for their euro variants.
    pub(crate) const fn character_set(self) -> u16 {
        match self {
            Self::Ccsid37
            | Self::Ccsid273
            | Self::Ccsid277
            | Self::Ccsid285
            | Self::Ccsid297
            | Self::Ccsid500
            | Self::Ccsid871 => 697,
            Self::Ccsid1140
            | Self::Ccsid1141
            | Self::Ccsid1142
            | Self::Ccsid1146
            | Self::Ccsid1147
            | Self::Ccsid1148
            | Self::Ccsid1149 => 695,
        }
    }

    /// The IBM i keyboard type for the code page (the `KBDTYPE` of the device description). For
    /// 500, the international page, BLI (Belgium, multinational) is the usual choice (R-EBC-2).
    pub(crate) const fn keyboard_type(self) -> &'static str {
        match self {
            Self::Ccsid37 | Self::Ccsid1140 => "USB",
            Self::Ccsid273 | Self::Ccsid1141 => "AGB",
            Self::Ccsid277 | Self::Ccsid1142 => "DMB",
            Self::Ccsid285 | Self::Ccsid1146 => "UKB",
            Self::Ccsid297 | Self::Ccsid1147 => "FAB",
            Self::Ccsid500 | Self::Ccsid1148 => "BLI",
            Self::Ccsid871 | Self::Ccsid1149 => "ICB",
        }
    }

    const fn table(self) -> &'static [char; 192] {
        match self {
            Self::Ccsid37 => &tables::CCSID_37,
            Self::Ccsid273 => &tables::CCSID_273,
            Self::Ccsid277 => &tables::CCSID_277,
            Self::Ccsid285 => &tables::CCSID_285,
            Self::Ccsid297 => &tables::CCSID_297,
            Self::Ccsid500 => &tables::CCSID_500,
            Self::Ccsid871 => &tables::CCSID_871,
            Self::Ccsid1140 => &tables::CCSID_1140,
            Self::Ccsid1141 => &tables::CCSID_1141,
            Self::Ccsid1142 => &tables::CCSID_1142,
            Self::Ccsid1146 => &tables::CCSID_1146,
            Self::Ccsid1147 => &tables::CCSID_1147,
            Self::Ccsid1148 => &tables::CCSID_1148,
            Self::Ccsid1149 => &tables::CCSID_1149,
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
    fn the_new_code_pages_have_their_national_letters_r_ebc_2() {
        for (page, c) in [
            (CodePage::Ccsid273, 'ß'),
            (CodePage::Ccsid285, '£'),
            (CodePage::Ccsid297, 'é'),
            (CodePage::Ccsid500, 'ç'),
            (CodePage::Ccsid871, 'Þ'),
        ] {
            let byte = page.to_byte(c).expect("in the page");
            assert_eq!(page.to_char(byte), c, "{page:?}");
        }
        for page in [
            CodePage::Ccsid1141,
            CodePage::Ccsid1146,
            CodePage::Ccsid1147,
            CodePage::Ccsid1148,
            CodePage::Ccsid1149,
        ] {
            assert!(page.to_byte('€').is_some(), "{page:?}");
            assert_eq!(page.character_set(), 695);
        }
        assert_eq!(CodePage::Ccsid273.keyboard_type(), "AGB");
        assert_eq!(CodePage::Ccsid871.ccsid(), 871);
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
