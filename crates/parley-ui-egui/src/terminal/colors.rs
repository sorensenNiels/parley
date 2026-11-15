//! The 5250 screen's colour schemes (R-UI-23): Classic, Light and the operator's own. The one
//! place outside the 5250 screen's own drawing where colours are written down (CLAUDE.md).

use egui::Color32;
use parley_core::Color;
use serde::{Deserialize, Serialize};

/// A scheme the settings or a session chooses.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Scheme {
    /// The 5250 colours, a little softened, on black.
    #[default]
    Classic,
    /// The 5250 colours darkened to read on a light background.
    Light,
    /// The operator's own colours ([`CustomColors`]).
    Custom,
}

impl Scheme {
    pub const ALL: [(Self, &'static str); 3] = [
        (Self::Classic, "Classic"),
        (Self::Light, "Light"),
        (Self::Custom, "Custom"),
    ];

    pub fn name(self) -> &'static str {
        Self::ALL
            .iter()
            .find(|(s, _)| *s == self)
            .map_or("", |(_, n)| n)
    }
}

/// The seven 5250 colours in the order the settings list them.
pub const COLORS: [(Color, &str); 7] = [
    (Color::Green, "Green"),
    (Color::White, "White"),
    (Color::Red, "Red"),
    (Color::Turquoise, "Turquoise"),
    (Color::Yellow, "Yellow"),
    (Color::Pink, "Pink"),
    (Color::Blue, "Blue"),
];

/// The operator's own scheme, as stored: RGB triples, the background first. Starts as Classic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CustomColors {
    pub background: [u8; 3],
    /// In the order of [`COLORS`].
    pub colors: [[u8; 3]; 7],
}

impl Default for CustomColors {
    fn default() -> Self {
        let rgb = |c: Color32| [c.r(), c.g(), c.b()];
        let classic = Palette::classic();
        Self {
            background: rgb(classic.background),
            colors: COLORS.map(|(c, _)| rgb(classic.color(c))),
        }
    }
}

/// The colours the screen is drawn in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    pub background: Color32,
    /// In the order of [`COLORS`].
    colors: [Color32; 7],
}

impl Default for Palette {
    fn default() -> Self {
        Self::classic()
    }
}

impl Palette {
    pub const fn classic() -> Self {
        Self {
            background: Color32::BLACK,
            colors: [
                Color32::from_rgb(0x4A, 0xE2, 0x4A),
                Color32::from_rgb(0xF2, 0xF2, 0xF2),
                Color32::from_rgb(0xFF, 0x55, 0x55),
                Color32::from_rgb(0x40, 0xE0, 0xD0),
                Color32::from_rgb(0xFF, 0xF0, 0x5A),
                Color32::from_rgb(0xFF, 0x80, 0xFF),
                Color32::from_rgb(0x6C, 0x9C, 0xFF),
            ],
        }
    }

    /// Dark enough on the light background for normal text (WCAG 4.5:1 or better).
    pub const fn light() -> Self {
        Self {
            background: Color32::from_rgb(0xFA, 0xFA, 0xF7),
            colors: [
                Color32::from_rgb(0x1B, 0x6E, 0x1B),
                Color32::from_rgb(0x1F, 0x1F, 0x1F),
                Color32::from_rgb(0xB7, 0x1C, 0x1C),
                Color32::from_rgb(0x00, 0x6A, 0x73),
                Color32::from_rgb(0x7A, 0x5A, 0x00),
                Color32::from_rgb(0x9C, 0x12, 0x63),
                Color32::from_rgb(0x15, 0x55, 0xB0),
            ],
        }
    }

    pub fn custom(custom: &CustomColors) -> Self {
        let rgb = |[r, g, b]: [u8; 3]| Color32::from_rgb(r, g, b);
        Self {
            background: rgb(custom.background),
            colors: custom.colors.map(rgb),
        }
    }

    pub fn of(scheme: Scheme, custom: &CustomColors) -> Self {
        match scheme {
            Scheme::Classic => Self::classic(),
            Scheme::Light => Self::light(),
            Scheme::Custom => Self::custom(custom),
        }
    }

    pub fn color(&self, c: Color) -> Color32 {
        let i = COLORS.iter().position(|(k, _)| *k == c).unwrap_or(0);
        self.colors.get(i).copied().unwrap_or(Color32::GRAY)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The WCAG contrast ratio of two colours.
    fn contrast(a: Color32, b: Color32) -> f32 {
        let luminance = |c: Color32| {
            let channel = |v: u8| {
                let v = f32::from(v) / 255.0;
                if v <= 0.039_28 {
                    v / 12.92
                } else {
                    ((v + 0.055) / 1.055).powf(2.4)
                }
            };
            0.0722f32.mul_add(
                channel(c.b()),
                0.2126f32.mul_add(channel(c.r()), 0.7152 * channel(c.g())),
            )
        };
        let (l1, l2) = (luminance(a), luminance(b));
        (l1.max(l2) + 0.05) / (l1.min(l2) + 0.05)
    }

    #[test]
    fn every_colour_of_the_light_scheme_reads_on_its_background_r_ui_23() {
        let light = Palette::light();
        for (c, name) in COLORS {
            let ratio = contrast(light.color(c), light.background);
            assert!(ratio >= 4.5, "{name}: {ratio:.2}");
        }
    }

    #[test]
    fn the_custom_colours_start_as_classic_and_are_used_as_chosen_r_ui_23() {
        let mut custom = CustomColors::default();
        assert_eq!(Palette::custom(&custom), Palette::classic());
        custom.colors[2] = [1, 2, 3];
        custom.background = [9, 9, 9];
        let palette = Palette::of(Scheme::Custom, &custom);
        assert_eq!(palette.color(Color::Red), Color32::from_rgb(1, 2, 3));
        assert_eq!(palette.background, Color32::from_rgb(9, 9, 9));
    }

    #[test]
    fn each_scheme_gives_its_own_palette_r_ui_23() {
        let custom = CustomColors::default();
        assert_eq!(Palette::of(Scheme::Classic, &custom), Palette::classic());
        assert_eq!(Palette::of(Scheme::Light, &custom), Palette::light());
        assert_eq!(Scheme::Light.name(), "Light");
    }
}
