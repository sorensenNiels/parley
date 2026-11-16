//! The screen's faces and fallbacks (R-SSH-20, D49): the terminal font chosen in the settings with its bold, italic
//! and bold italic faces (`JetBrains Mono`'s are bundled, an installed font's own are used when it has them), and
//! Nerd Font symbols and emoji (monochrome) as fallbacks for every face. The fonts are under the SIL Open Font
//! License and MIT (`assets/fonts/*-LICENSE.txt`).

use std::sync::Arc;

use egui::{FontData, FontDefinitions, FontFamily, FontId};
use parley_ui_egui::fonts::{self as shared, JETBRAINS_MONO};

const NERD_SYMBOLS: &str = "Symbols Nerd Font Mono";
const EMOJI: &str = "Noto Emoji (monochrome)";

/// The three faces besides the regular one, with their family names and the bundled `JetBrains Mono` files.
const FACES: [(Face, &str, &[u8]); 3] = [
    (
        Face::Bold,
        "ssh-terminal-bold",
        include_bytes!("../assets/fonts/JetBrainsMono-Bold.ttf"),
    ),
    (
        Face::Italic,
        "ssh-terminal-italic",
        include_bytes!("../assets/fonts/JetBrainsMono-Italic.ttf"),
    ),
    (
        Face::BoldItalic,
        "ssh-terminal-bold-italic",
        include_bytes!("../assets/fonts/JetBrainsMono-BoldItalic.ttf"),
    ),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Face {
    Bold,
    Italic,
    BoldItalic,
}

impl Face {
    const fn is_bold(self) -> bool {
        matches!(self, Self::Bold | Self::BoldItalic)
    }

    const fn is_italic(self) -> bool {
        matches!(self, Self::Italic | Self::BoldItalic)
    }
}

/// Sets egui's fonts up for `terminal_font` with the faces and fallbacks. Returns a note when that font is not
/// installed and `JetBrains Mono` stands in.
pub fn install(ctx: &egui::Context, terminal_font: &str) -> Option<String> {
    let (definitions, note) = definitions(terminal_font);
    ctx.set_fonts(definitions);
    note
}

/// Parley's fonts with the screen's faces and fallbacks added.
fn definitions(terminal_font: &str) -> (FontDefinitions, Option<String>) {
    let (mut fonts, note) = parley_ui_egui::theme::font_definitions(terminal_font);
    let bundled = note.is_some() || terminal_font == JETBRAINS_MONO;
    let mut add = |name: &str, data: FontData| {
        fonts.font_data.insert(name.to_owned(), Arc::new(data));
    };
    add(
        NERD_SYMBOLS,
        FontData::from_static(include_bytes!(
            "../assets/fonts/SymbolsNerdFontMono-Regular.ttf"
        )),
    );
    add(
        EMOJI,
        FontData::from_static(include_bytes!("../assets/fonts/NotoEmoji-Regular.ttf")),
    );
    let family = shared::terminal_family();
    let mut regular = fonts.families.get(&family).cloned().unwrap_or_default();
    // After the chosen font, before the general fallbacks: symbols and emoji look the same everywhere.
    let at = regular.len().min(1);
    regular.splice(at..at, [NERD_SYMBOLS.to_owned(), EMOJI.to_owned()]);
    for (face, name, bytes) in FACES {
        let data = if bundled {
            Some(FontData::from_static(bytes))
        } else {
            shared::system_face(terminal_font, face.is_bold(), face.is_italic())
        };
        if let Some(data) = data {
            fonts.font_data.insert(name.to_owned(), Arc::new(data));
            let list = std::iter::once(name.to_owned())
                .chain(regular.iter().cloned())
                .collect();
            fonts.families.insert(FontFamily::Name(name.into()), list);
        }
    }
    fonts.families.insert(family, regular);
    (fonts, note)
}

/// The fonts a frame draws with, at one size; a face the font lacks is `None` and drawn from the regular one.
#[derive(Debug, Clone)]
pub struct Faces {
    pub regular: FontId,
    pub bold: Option<FontId>,
    pub italic: Option<FontId>,
    pub bold_italic: Option<FontId>,
}

impl Faces {
    pub fn at(ctx: &egui::Context, size: f32) -> Self {
        let face = |i: usize| {
            let family = FontFamily::Name(FACES[i].1.into());
            ctx.fonts(|f| f.definitions().families.contains_key(&family))
                .then(|| FontId::new(size, family))
        };
        Self {
            regular: shared::terminal_font(ctx, size),
            bold: face(0),
            italic: face(1),
            bold_italic: face(2),
        }
    }

    /// The font for a cell, and whether bold must be faked by drawing twice.
    pub fn of(&self, bold: bool, italic: bool) -> (&FontId, bool) {
        let chosen = match (bold, italic) {
            (false, false) => None,
            (true, false) => self.bold.as_ref(),
            (false, true) => self.italic.as_ref(),
            (true, true) => self.bold_italic.as_ref().or(self.bold.as_ref()),
        };
        match chosen {
            Some(font) => (font, false),
            None => (&self.regular, bold),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bundled_font_gets_its_faces_and_every_face_the_fallbacks_r_ssh_20() {
        let (fonts, note) = definitions(JETBRAINS_MONO);
        assert_eq!(note, None);
        let regular = &fonts.families[&shared::terminal_family()];
        assert_eq!(&regular[..3], [JETBRAINS_MONO, NERD_SYMBOLS, EMOJI]);
        for (_, name, _) in FACES {
            let list = &fonts.families[&FontFamily::Name(name.into())];
            assert_eq!(list[0], name);
            assert!(list.contains(&NERD_SYMBOLS.to_owned()) && list.contains(&EMOJI.to_owned()));
        }
    }

    #[test]
    fn a_missing_font_falls_back_to_the_bundled_one_with_its_faces_r_ssh_20() {
        let (fonts, note) = definitions("No Such Font 12345");
        assert!(note.is_some());
        assert!(
            fonts
                .families
                .contains_key(&FontFamily::Name(FACES[0].1.into()))
        );
    }

    #[test]
    fn the_fallbacks_hold_nerd_symbols_and_emoji_r_ssh_20() {
        use skrifa::MetadataProvider;
        let has = |bytes: &[u8], c: char| {
            skrifa::FontRef::new(bytes).is_ok_and(|f| f.charmap().map(c).is_some())
        };
        // U+E0B0 is powerline's arrow, U+F31B a distribution's logo; U+1F680 a rocket.
        let nerd = include_bytes!("../assets/fonts/SymbolsNerdFontMono-Regular.ttf");
        assert!(has(nerd, '\u{E0B0}') && has(nerd, '\u{F31B}'));
        assert!(has(
            include_bytes!("../assets/fonts/NotoEmoji-Regular.ttf"),
            '\u{1F680}'
        ));
    }

    #[test]
    fn a_face_the_font_lacks_is_faked_from_the_regular_one_r_ssh_20() {
        let regular = FontId::monospace(14.0);
        let faces = Faces {
            regular: regular.clone(),
            bold: None,
            italic: Some(FontId::proportional(14.0)),
            bold_italic: None,
        };
        assert_eq!(faces.of(true, false), (&regular, true));
        assert!(!faces.of(false, true).1, "a real italic face");
        assert_eq!(faces.of(true, true), (&regular, true));
    }
}
