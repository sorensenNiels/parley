//! The terminal's font (R-UI-6): one of the bundled monospace fonts or one installed on the
//! system, chosen in the settings. The UI's own fonts are set up in [`crate::theme`]. The
//! bundled text fonts are under the SIL Open Font License, the icon font under MIT
//! (`assets/fonts/*-LICENSE.txt`).

use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use egui::{FontData, FontDefinitions, FontFamily};
use fontique::{
    Collection, CollectionOptions, FallbackKey, FamilyId, Script, SourceCache, SourceCacheOptions,
};
use skrifa::instance::{LocationRef, Size};
use skrifa::{FontRef, MetadataProvider};

/// Names the bundled monospace fonts go by, in the settings and in egui.
pub const JETBRAINS_MONO: &str = "JetBrains Mono";
pub const PLEX_MONO: &str = "IBM Plex Mono";
/// The font egui ships, always there as the terminal's fallback.
const HACK: &str = "Hack";
/// The terminal's default font: Tortuga's code font (D12).
pub const DEFAULT_TERMINAL_FONT: &str = JETBRAINS_MONO;
/// The monospace fonts Parley carries, offered first.
pub const BUNDLED: [&str; 3] = [JETBRAINS_MONO, PLEX_MONO, HACK];

const TERMINAL: &str = "terminal";

/// The family the terminal draws with.
pub fn terminal_family() -> FontFamily {
    FontFamily::Name(TERMINAL.into())
}

/// The terminal font at `size`; plain monospace before the theme sets the fonts up (in
/// tests that draw the terminal alone).
pub fn terminal_font(ctx: &egui::Context, size: f32) -> egui::FontId {
    let family = terminal_family();
    let known = ctx.fonts(|f| f.definitions().families.contains_key(&family));
    egui::FontId::new(size, if known { family } else { FontFamily::Monospace })
}

/// Adds the terminal family to `fonts` (which already hold the bundled fonts): `terminal_font`
/// first, then the monospace fonts for glyphs it lacks. Returns a note when that font is not
/// installed and the default stands in.
pub fn add_terminal_family(fonts: &mut FontDefinitions, terminal_font: &str) -> Option<String> {
    let mut note = None;
    let chosen = if fonts.font_data.contains_key(terminal_font) {
        terminal_font.to_owned()
    } else if let Some(data) = system_font(terminal_font) {
        let key = format!("system: {terminal_font}");
        fonts.font_data.insert(key.clone(), Arc::new(data));
        key
    } else {
        note = Some(format!(
            "The font \"{terminal_font}\" is not installed; using {DEFAULT_TERMINAL_FONT}."
        ));
        DEFAULT_TERMINAL_FONT.to_owned()
    };
    let mut terminal = vec![chosen];
    for name in fonts
        .families
        .get(&FontFamily::Monospace)
        .into_iter()
        .flatten()
    {
        if !terminal.contains(name) {
            terminal.push(name.clone());
        }
    }
    fonts.families.insert(terminal_family(), terminal);
    note
}

fn system_collection() -> (Collection, SourceCache) {
    let collection = Collection::new(CollectionOptions {
        shared: false,
        system_fonts: true,
    });
    (collection, SourceCache::new(SourceCacheOptions::default()))
}

/// The regular face of an installed family.
fn system_font(name: &str) -> Option<FontData> {
    let (mut collection, mut cache) = system_collection();
    let family = collection.family_by_name(name)?;
    let font = family.default_font()?;
    let blob = font.load(Some(&mut cache))?;
    let mut data = FontData::from_owned(blob.data().to_vec());
    data.index = font.index();
    Some(data)
}

/// The bold, italic or bold italic face of an installed family (Parley SSH, R-SSH-20); `None` when it has none.
pub fn system_face(name: &str, bold: bool, italic: bool) -> Option<FontData> {
    let (mut collection, mut cache) = system_collection();
    let family = collection.family_by_name(name)?;
    let font = family.fonts().iter().find(|f| {
        let heavy = f.weight().value() >= 600.0;
        let slanted = f.style() != fontique::FontStyle::Normal;
        heavy == bold && slanted == italic
    })?;
    let blob = font.load(Some(&mut cache))?;
    let mut data = FontData::from_owned(blob.data().to_vec());
    data.index = font.index();
    Some(data)
}

/// An installed font egui may fall back to, by its key in [`FontDefinitions::font_data`].
pub type Fallback = (String, Arc<FontData>);

/// Installed fonts for the scripts the bundled fonts lack: Japanese, Chinese and Korean
/// (R-UI-29). Looked for the first time they are wanted and kept, so setting the fonts up
/// again costs nothing; none where none is installed.
pub fn script_fallbacks() -> &'static [Fallback] {
    static FALLBACKS: OnceLock<Vec<Fallback>> = OnceLock::new();
    FALLBACKS.get_or_init(find_script_fallbacks)
}

/// The system's own fallback family for each of Han as Japanese (with kana), Han as Chinese
/// and Hangul, once each: one font that covers all three is added once.
fn find_script_fallbacks() -> Vec<Fallback> {
    let han = Script::from_bytes(*b"Hani");
    let keys: [FallbackKey; 3] = [
        (han, "ja").into(),
        (han, "zh").into(),
        Script::from_bytes(*b"Hang").into(),
    ];
    let (mut collection, mut cache) = system_collection();
    let mut found: Vec<Fallback> = Vec::new();
    for key in keys {
        let ids: Vec<FamilyId> = collection.fallback_families(key).collect();
        let Some(family) = ids.into_iter().find_map(|id| collection.family(id)) else {
            continue;
        };
        let name = format!("fallback: {}", family.name());
        if found.iter().any(|(known, _)| *known == name) {
            continue;
        }
        let Some(blob) = family
            .default_font()
            .and_then(|font| Some((font.load(Some(&mut cache))?, font.index())))
        else {
            continue;
        };
        let mut data = FontData::from_owned(blob.0.data().to_vec());
        data.index = blob.1;
        found.push((name, Arc::new(data)));
    }
    found
}

/// Puts `fallbacks` last in `families`, so they draw only the characters every other font in
/// them lacks (R-UI-29).
pub fn add_fallbacks(fonts: &mut FontDefinitions, families: &[FontFamily], fallbacks: &[Fallback]) {
    for (name, data) in fallbacks {
        fonts.font_data.insert(name.clone(), Arc::clone(data));
        for family in families {
            let list = fonts.families.entry(family.clone()).or_default();
            if !list.contains(name) {
                list.push(name.clone());
            }
        }
    }
}

/// True if the font's narrow and wide letters are equally wide: a grid needs that.
fn is_monospace(data: &[u8], index: u32) -> bool {
    let Ok(font) = FontRef::from_index(data, index) else {
        return false;
    };
    let charmap = font.charmap();
    let metrics = font.glyph_metrics(Size::unscaled(), LocationRef::default());
    let widths: Option<Vec<f32>> = ['i', 'M', 'W', '.']
        .into_iter()
        .map(|c| {
            charmap
                .map(c)
                .and_then(|glyph| metrics.advance_width(glyph))
        })
        .collect();
    widths.is_some_and(|w| w.windows(2).all(|pair| (pair[0] - pair[1]).abs() < 0.01))
}

/// The installed monospace families, sorted, without the bundled ones. Reads every font, so
/// it can take a second: run it off the UI thread.
pub fn installed_monospace() -> Vec<String> {
    let (mut collection, mut cache) = system_collection();
    let names: Vec<String> = collection.family_names().map(str::to_owned).collect();
    let mut found: Vec<String> = names
        .into_iter()
        .filter(|name| !name.starts_with('.') && !BUNDLED.contains(&name.as_str()))
        .filter(|name| {
            collection
                .family_by_name(name)
                .and_then(|family| family.default_font().cloned())
                .and_then(|font| Some((font.load(Some(&mut cache))?, font.index())))
                .is_some_and(|(blob, index)| is_monospace(blob.data(), index))
        })
        .collect();
    found.sort_by_key(|name| name.to_lowercase());
    found.dedup();
    found
}

/// The installed monospace fonts, looked for on a thread the first time they are wanted.
#[derive(Debug, Default, Clone)]
pub struct Installed(Arc<Mutex<Option<Vec<String>>>>);

impl Installed {
    /// Starts looking; `done` is called (to repaint) when the list is ready.
    pub fn search(done: impl FnOnce() + Send + 'static) -> Self {
        let installed = Self::default();
        let result = Arc::clone(&installed.0);
        std::thread::spawn(move || {
            let fonts = installed_monospace();
            *result.lock().unwrap_or_else(PoisonError::into_inner) = Some(fonts);
            done();
        });
        installed
    }

    /// The list, `None` while still looking.
    pub fn get(&self) -> Option<Vec<String>> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::font_definitions;

    #[test]
    fn the_bundled_terminal_fonts_are_monospace_and_inter_is_not() {
        // R-UI-6
        assert!(is_monospace(
            include_bytes!("../assets/fonts/IBMPlexMono-Regular.ttf"),
            0
        ));
        assert!(is_monospace(
            include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf"),
            0
        ));
        assert!(!is_monospace(
            include_bytes!("../assets/fonts/Inter-Regular.ttf"),
            0
        ));
        assert!(!is_monospace(b"not a font", 0));
    }

    #[test]
    fn a_missing_terminal_font_falls_back_to_the_default_with_a_note() {
        // R-UI-6
        let (fonts, note) = font_definitions("No Such Font 12345");
        assert!(note.is_some_and(|n| n.contains("No Such Font 12345")));
        let terminal = &fonts.families[&terminal_family()];
        assert_eq!(terminal[0], DEFAULT_TERMINAL_FONT);
        assert!(
            terminal.contains(&HACK.to_owned()),
            "Hack fills in missing glyphs"
        );
    }

    #[test]
    fn script_fallbacks_come_last_in_every_text_family_and_the_terminal_r_ui_29() {
        let mut fonts = FontDefinitions::default();
        let before = fonts.families[&FontFamily::Proportional].clone();
        let fake: Vec<Fallback> = vec![(
            "fallback: Fake CJK".to_owned(),
            Arc::new(FontData::from_static(include_bytes!(
                "../assets/fonts/Inter-Regular.ttf"
            ))),
        )];
        let families = [FontFamily::Proportional, FontFamily::Monospace];
        add_fallbacks(&mut fonts, &families, &fake);
        add_fallbacks(&mut fonts, &families, &fake);
        assert!(fonts.font_data.contains_key("fallback: Fake CJK"));
        let ui = &fonts.families[&FontFamily::Proportional];
        assert_eq!(
            ui[..before.len()],
            before[..],
            "the others keep their place"
        );
        assert_eq!(ui.last().map(String::as_str), Some("fallback: Fake CJK"));
        assert_eq!(ui.len(), before.len() + 1, "added once");
        add_terminal_family(&mut fonts, DEFAULT_TERMINAL_FONT);
        let terminal = &fonts.families[&terminal_family()];
        assert_eq!(
            terminal.last().map(String::as_str),
            Some("fallback: Fake CJK")
        );
    }

    #[test]
    fn looking_for_script_fallbacks_twice_reads_the_system_once_r_ui_29() {
        // Whatever is installed: the same fonts, kept, and each one once.
        let first = script_fallbacks();
        assert!(std::ptr::eq(first, script_fallbacks()));
        let mut names: Vec<&String> = first.iter().map(|(n, _)| n).collect();
        names.dedup();
        assert_eq!(names.len(), first.len());
    }

    #[test]
    fn a_bundled_terminal_font_heads_the_terminal_family() {
        let (fonts, note) = font_definitions(PLEX_MONO);
        assert_eq!(note, None);
        assert_eq!(fonts.families[&terminal_family()][0], PLEX_MONO);
    }
}
