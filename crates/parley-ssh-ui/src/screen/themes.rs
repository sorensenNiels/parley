//! The terminal themes (R-SSH-20, D49): Parley's own and well-known schemes under permissive licences (all MIT),
//! with the colours their authors publish for terminals. They colour the screen only (D49).

use serde::{Deserialize, Serialize};

use super::Palette;

/// A theme for the screen, kept by name in the settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TerminalTheme {
    ParleyDark,
    ParleyLight,
    SolarizedDark,
    SolarizedLight,
    Nord,
    Dracula,
    GruvboxDark,
    GruvboxLight,
    CatppuccinMocha,
    CatppuccinLatte,
    TokyoNight,
    TokyoNightDay,
}

impl TerminalTheme {
    /// Every theme, dark ones first, as the gallery shows them.
    pub const ALL: [Self; 12] = [
        Self::ParleyDark,
        Self::SolarizedDark,
        Self::Nord,
        Self::Dracula,
        Self::GruvboxDark,
        Self::CatppuccinMocha,
        Self::TokyoNight,
        Self::ParleyLight,
        Self::SolarizedLight,
        Self::GruvboxLight,
        Self::CatppuccinLatte,
        Self::TokyoNightDay,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::ParleyDark => "Parley Dark",
            Self::ParleyLight => "Parley Light",
            Self::SolarizedDark => "Solarized Dark",
            Self::SolarizedLight => "Solarized Light",
            Self::Nord => "Nord",
            Self::Dracula => "Dracula",
            Self::GruvboxDark => "Gruvbox Dark",
            Self::GruvboxLight => "Gruvbox Light",
            Self::CatppuccinMocha => "Catppuccin Mocha",
            Self::CatppuccinLatte => "Catppuccin Latte",
            Self::TokyoNight => "Tokyo Night",
            Self::TokyoNightDay => "Tokyo Night Day",
        }
    }

    /// Whether the theme is made for a dark background.
    pub const fn is_dark(self) -> bool {
        match self {
            Self::ParleyDark
            | Self::SolarizedDark
            | Self::Nord
            | Self::Dracula
            | Self::GruvboxDark
            | Self::CatppuccinMocha
            | Self::TokyoNight => true,
            Self::ParleyLight
            | Self::SolarizedLight
            | Self::GruvboxLight
            | Self::CatppuccinLatte
            | Self::TokyoNightDay => false,
        }
    }

    #[expect(
        clippy::mistyped_literal_suffixes,
        reason = "colours are written 0xRR_GG_BB, and a blue of 0x16 or 0x64 is not a suffix"
    )]
    pub const fn palette(self) -> Palette {
        match self {
            Self::ParleyDark => Palette::dark(),
            Self::ParleyLight => Palette::light(),
            Self::SolarizedDark => {
                Palette::from_hex([0x00_2B_36, 0x83_94_96, 0x07_36_42, 0x93_A1_A1], SOLARIZED)
            }
            Self::SolarizedLight => {
                Palette::from_hex([0xFD_F6_E3, 0x65_7B_83, 0xEE_E8_D5, 0x58_6E_75], SOLARIZED)
            }
            Self::Nord => Palette::from_hex(
                [0x2E_34_40, 0xD8_DE_E9, 0x43_4C_5E, 0xD8_DE_E9],
                [
                    0x3B_42_52, 0xBF_61_6A, 0xA3_BE_8C, 0xEB_CB_8B, 0x81_A1_C1, 0xB4_8E_AD,
                    0x88_C0_D0, 0xE5_E9_F0, 0x4C_56_6A, 0xBF_61_6A, 0xA3_BE_8C, 0xEB_CB_8B,
                    0x81_A1_C1, 0xB4_8E_AD, 0x8F_BC_BB, 0xEC_EF_F4,
                ],
            ),
            Self::Dracula => Palette::from_hex(
                [0x28_2A_36, 0xF8_F8_F2, 0x44_47_5A, 0xF8_F8_F2],
                [
                    0x21_22_2C, 0xFF_55_55, 0x50_FA_7B, 0xF1_FA_8C, 0xBD_93_F9, 0xFF_79_C6,
                    0x8B_E9_FD, 0xF8_F8_F2, 0x62_72_A4, 0xFF_6E_6E, 0x69_FF_94, 0xFF_FF_A5,
                    0xD6_AC_FF, 0xFF_92_DF, 0xA4_FF_FF, 0xFF_FF_FF,
                ],
            ),
            Self::GruvboxDark => Palette::from_hex(
                [0x28_28_28, 0xEB_DB_B2, 0x50_49_45, 0xEB_DB_B2],
                [
                    0x28_28_28, 0xCC_24_1D, 0x98_97_1A, 0xD7_99_21, 0x45_85_88, 0xB1_62_86,
                    0x68_9D_6A, 0xA8_99_84, 0x92_83_74, 0xFB_49_34, 0xB8_BB_26, 0xFA_BD_2F,
                    0x83_A5_98, 0xD3_86_9B, 0x8E_C0_7C, 0xEB_DB_B2,
                ],
            ),
            Self::GruvboxLight => Palette::from_hex(
                [0xFB_F1_C7, 0x3C_38_36, 0xD5_C4_A1, 0x3C_38_36],
                [
                    0xFB_F1_C7, 0xCC_24_1D, 0x98_97_1A, 0xD7_99_21, 0x45_85_88, 0xB1_62_86,
                    0x68_9D_6A, 0x7C_6F_64, 0x92_83_74, 0x9D_00_06, 0x79_74_0E, 0xB5_76_14,
                    0x07_66_78, 0x8F_3F_71, 0x42_7B_58, 0x3C_38_36,
                ],
            ),
            Self::CatppuccinMocha => Palette::from_hex(
                [0x1E_1E_2E, 0xCD_D6_F4, 0x58_5B_70, 0xF5_E0_DC],
                [
                    0x45_47_5A, 0xF3_8B_A8, 0xA6_E3_A1, 0xF9_E2_AF, 0x89_B4_FA, 0xF5_C2_E7,
                    0x94_E2_D5, 0xBA_C2_DE, 0x58_5B_70, 0xF3_8B_A8, 0xA6_E3_A1, 0xF9_E2_AF,
                    0x89_B4_FA, 0xF5_C2_E7, 0x94_E2_D5, 0xA6_AD_C8,
                ],
            ),
            Self::CatppuccinLatte => Palette::from_hex(
                [0xEF_F1_F5, 0x4C_4F_69, 0xAC_B0_BE, 0xDC_8A_78],
                [
                    0x5C_5F_77, 0xD2_0F_39, 0x40_A0_2B, 0xDF_8E_1D, 0x1E_66_F5, 0xEA_76_CB,
                    0x17_92_99, 0xAC_B0_BE, 0x6C_6F_85, 0xD2_0F_39, 0x40_A0_2B, 0xDF_8E_1D,
                    0x1E_66_F5, 0xEA_76_CB, 0x17_92_99, 0xBC_C0_CC,
                ],
            ),
            Self::TokyoNight => Palette::from_hex(
                [0x1A_1B_26, 0xC0_CA_F5, 0x28_34_57, 0xC0_CA_F5],
                [
                    0x15_16_1E, 0xF7_76_8E, 0x9E_CE_6A, 0xE0_AF_68, 0x7A_A2_F7, 0xBB_9A_F7,
                    0x7D_CF_FF, 0xA9_B1_D6, 0x41_48_68, 0xF7_76_8E, 0x9E_CE_6A, 0xE0_AF_68,
                    0x7A_A2_F7, 0xBB_9A_F7, 0x7D_CF_FF, 0xC0_CA_F5,
                ],
            ),
            Self::TokyoNightDay => Palette::from_hex(
                [0xE1_E2_E7, 0x37_60_BF, 0xB7_C1_E3, 0x37_60_BF],
                [
                    0xE9_E9_ED, 0xF5_2A_65, 0x58_75_39, 0x8C_6C_3E, 0x2E_7D_E9, 0x98_54_F1,
                    0x00_71_97, 0x61_72_B0, 0xA1_A6_C5, 0xF5_2A_65, 0x58_75_39, 0x8C_6C_3E,
                    0x2E_7D_E9, 0x98_54_F1, 0x00_71_97, 0x37_60_BF,
                ],
            ),
        }
    }
}

/// Solarized's 16 colours, the same on dark and light (Ethan Schoonover's terminal mapping).
#[expect(
    clippy::mistyped_literal_suffixes,
    reason = "colours are written 0xRR_GG_BB, and a blue of 0x16 is not a suffix"
)]
const SOLARIZED: [u32; 16] = [
    0x07_36_42, 0xDC_32_2F, 0x85_99_00, 0xB5_89_00, 0x26_8B_D2, 0xD3_36_82, 0x2A_A1_98, 0xEE_E8_D5,
    0x00_2B_36, 0xCB_4B_16, 0x58_6E_75, 0x65_7B_83, 0x83_94_96, 0x6C_71_C4, 0x93_A1_A1, 0xFD_F6_E3,
];

#[cfg(test)]
mod tests {
    use super::*;

    /// WCAG's relative luminance.
    fn luminance(c: egui::Color32) -> f64 {
        let channel = |v: u8| {
            let v = f64::from(v) / 255.0;
            if v <= 0.039_28 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.0722_f64.mul_add(
            channel(c.b()),
            0.2126_f64.mul_add(channel(c.r()), 0.7152 * channel(c.g())),
        )
    }

    fn contrast(a: egui::Color32, b: egui::Color32) -> f64 {
        let (x, y) = (luminance(a), luminance(b));
        (x.max(y) + 0.05) / (x.min(y) + 0.05)
    }

    #[test]
    fn every_theme_is_listed_once_and_says_whether_it_is_dark_r_ssh_20() {
        let names: std::collections::HashSet<_> =
            TerminalTheme::ALL.iter().map(|t| t.name()).collect();
        assert_eq!(names.len(), TerminalTheme::ALL.len());
        for theme in TerminalTheme::ALL {
            let p = theme.palette();
            let dark = luminance(p.background) < luminance(p.foreground);
            assert_eq!(dark, theme.is_dark(), "{}", theme.name());
        }
    }

    #[test]
    fn every_themes_text_and_cursor_can_be_read_on_its_background_r_ssh_20() {
        for theme in TerminalTheme::ALL {
            let p = theme.palette();
            // 4.0, not WCAG's 4.5: Solarized Light's own text colour is 4.1:1, and the themes keep their authors'
            // colours (D49).
            assert!(
                contrast(p.foreground, p.background) >= 4.0,
                "{} text",
                theme.name()
            );
            // A filled block is found at less: Catppuccin Latte's own cursor is 2.3:1.
            assert!(
                contrast(p.cursor, p.background) >= 2.0,
                "{} cursor",
                theme.name()
            );
            assert_ne!(p.selection, p.background, "{} selection", theme.name());
        }
    }

    #[test]
    fn a_theme_is_kept_by_name_r_ssh_20() {
        let text = serde_json::to_string(&TerminalTheme::TokyoNightDay).expect("serialises");
        assert_eq!(text, "\"TokyoNightDay\"");
    }
}
