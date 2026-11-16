//! The SSH screen's colours (R-SSH-8): the 16 ANSI colours of Parley's dark and light terminal schemes, the
//! xterm 256-colour cube and greys, and 24-bit colours. With the 5250 schemes (`terminal::colors`), the one
//! place colours are written down (CLAUDE.md).

use egui::Color32;
use parley_ssh_core::term::Color;

/// The colours the screen is drawn in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    pub background: Color32,
    pub foreground: Color32,
    pub selection: Color32,
    /// Black, red, green, yellow, blue, magenta, cyan, white; then the bright ones.
    ansi: [Color32; 16],
}

const fn rgb(value: u32) -> Color32 {
    let [_, r, g, b] = value.to_be_bytes();
    Color32::from_rgb(r, g, b)
}

impl Palette {
    /// On black, as Parley 5250's Classic scheme.
    pub const fn dark() -> Self {
        Self {
            background: Color32::BLACK,
            foreground: rgb(0xD8_D8_D8),
            selection: rgb(0x3A_5A_8C),
            ansi: [
                rgb(0x00_00_00),
                rgb(0xE0_4B_4B),
                rgb(0x4A_C2_4A),
                rgb(0xD9_C0_43),
                rgb(0x5C_8C_E6),
                rgb(0xC0_6C_D8),
                rgb(0x40_C8_C8),
                rgb(0xD0_D0_D0),
                rgb(0x6E_6E_6E),
                rgb(0xFF_6E_6E),
                rgb(0x6E_EE_6E),
                rgb(0xFF_F0_5A),
                rgb(0x82_AA_FF),
                rgb(0xFF_8C_FF),
                rgb(0x6E_F0_E6),
                rgb(0xFF_FF_FF),
            ],
        }
    }

    /// On Parley 5250's Light background, dark enough to read (WCAG 4.5:1 for the normal colours).
    pub const fn light() -> Self {
        Self {
            background: rgb(0xFA_FA_F7),
            foreground: rgb(0x1F_1F_1F),
            selection: rgb(0xB4_D2_F5),
            ansi: [
                rgb(0x1F_1F_1F),
                rgb(0xB7_1C_1C),
                rgb(0x1B_6E_1B),
                rgb(0x7A_5A_00),
                rgb(0x15_55_B0),
                rgb(0x9C_12_63),
                rgb(0x00_6A_73),
                rgb(0x6A_6A_6A),
                rgb(0x55_55_55),
                rgb(0xD3_2F_2F),
                rgb(0x2E_8B_2E),
                rgb(0x9A_74_00),
                rgb(0x2D_6F_D2),
                rgb(0xB8_2E_8A),
                rgb(0x00_86_8F),
                rgb(0x00_00_00),
            ],
        }
    }

    /// The scheme for the UI's theme.
    pub const fn for_theme(dark: bool) -> Self {
        if dark { Self::dark() } else { Self::light() }
    }

    /// A text colour; `bright` turns the eight normal colours into their bright ones (bold as bright).
    pub fn foreground_of(&self, color: Color, bright: bool) -> Color32 {
        match color {
            Color::Default => self.foreground,
            Color::Indexed(i) if bright && i < 8 => self.indexed(i + 8),
            other => self.of(other).unwrap_or(self.foreground),
        }
    }

    pub fn background_of(&self, color: Color) -> Color32 {
        self.of(color).unwrap_or(self.background)
    }

    fn of(&self, color: Color) -> Option<Color32> {
        match color {
            Color::Default => None,
            Color::Indexed(i) => Some(self.indexed(i)),
            Color::Rgb(r, g, b) => Some(Color32::from_rgb(r, g, b)),
        }
    }

    /// xterm's 256 colours: 16 from the scheme, a 6×6×6 cube, then 24 greys.
    fn indexed(&self, i: u8) -> Color32 {
        match i {
            0..=15 => self.ansi[usize::from(i)],
            16..=231 => {
                let i = i - 16;
                let level = |n: u8| if n == 0 { 0 } else { 55 + n * 40 };
                Color32::from_rgb(level(i / 36), level(i / 6 % 6), level(i % 6))
            }
            232..=255 => {
                let grey = 8 + (i - 232) * 10;
                Color32::from_rgb(grey, grey, grey)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_256_colours_are_xterms_r_ssh_6() {
        let p = Palette::dark();
        assert_eq!(
            p.background_of(Color::Indexed(16)),
            Color32::from_rgb(0, 0, 0)
        );
        assert_eq!(
            p.background_of(Color::Indexed(196)),
            Color32::from_rgb(255, 0, 0)
        );
        assert_eq!(
            p.background_of(Color::Indexed(231)),
            Color32::from_rgb(255, 255, 255)
        );
        assert_eq!(
            p.background_of(Color::Indexed(232)),
            Color32::from_rgb(8, 8, 8)
        );
        assert_eq!(
            p.background_of(Color::Indexed(255)),
            Color32::from_rgb(238, 238, 238)
        );
        assert_eq!(
            p.background_of(Color::Rgb(1, 2, 3)),
            Color32::from_rgb(1, 2, 3)
        );
    }

    #[test]
    fn default_colours_and_bold_as_bright() {
        let p = Palette::light();
        assert_eq!(p.foreground_of(Color::Default, true), p.foreground);
        assert_eq!(p.background_of(Color::Default), p.background);
        assert_eq!(
            p.foreground_of(Color::Indexed(1), true),
            p.foreground_of(Color::Indexed(9), false)
        );
        assert_eq!(
            p.foreground_of(Color::Indexed(100), true),
            p.foreground_of(Color::Indexed(100), false)
        );
    }
}
