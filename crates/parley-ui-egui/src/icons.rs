//! The icons Parley uses (R-UI-13, D17): codepoints of the bundled Phosphor icon font
//! (`assets/fonts/Phosphor-Regular.ttf`, MIT licence). They are drawn in a font family of their
//! own ([`FAMILY`]), never inside text: Inter has glyphs of its own at some of the same
//! private-use codepoints (U+E136 is a circled ">"), and Phosphor maps a–z to blanks.
//!
//! Add a new icon here and to [`ALL`] before using it; a test checks that the font has each.

use egui::{Button, FontFamily, Response, RichText, Role, Ui, WidgetInfo, WidgetText};

/// The font family that holds only the icon font.
pub const FAMILY: &str = "icons";

pub const ARROW_DOWN: &str = "\u{E03E}";
pub const ARROW_UP: &str = "\u{E08E}";
pub const ARROWS_CLOCKWISE: &str = "\u{E094}";
pub const BELL: &str = "\u{E0CE}";
pub const CARET_DOWN: &str = "\u{E136}";
pub const CHECK_CIRCLE: &str = "\u{E184}";
pub const CLIPBOARD: &str = "\u{E196}";
pub const COPY: &str = "\u{E1CA}";
pub const DATABASE: &str = "\u{E1DE}";
pub const ENVELOPE: &str = "\u{E218}";
pub const ERASER: &str = "\u{E21E}";
pub const EXPORT: &str = "\u{EAF0}";
pub const GEAR: &str = "\u{E272}";
pub const HOURGLASS: &str = "\u{E2B8}";
pub const INFO: &str = "\u{E2CE}";
pub const KEYBOARD: &str = "\u{E2D8}";
pub const LIGHTNING: &str = "\u{E2DE}";
pub const LIST: &str = "\u{E2F2}";
pub const LOCK: &str = "\u{E308}";
pub const MAGNIFYING_GLASS: &str = "\u{E30C}";
pub const PENCIL: &str = "\u{E3B4}";
pub const PLAY: &str = "\u{E3D0}";
pub const PLUGS_CONNECTED: &str = "\u{EB5A}";
pub const PLUS: &str = "\u{E3D4}";
pub const POWER: &str = "\u{E3DA}";
pub const PRINTER: &str = "\u{E3DC}";
pub const QUESTION: &str = "\u{E3E8}";
pub const RECORD: &str = "\u{E3EE}";
pub const RESET: &str = "\u{E038}";
pub const SIGN_OUT: &str = "\u{E42A}";
pub const SQUARES_FOUR: &str = "\u{E464}";
pub const STOP: &str = "\u{E46C}";
pub const TERMINAL: &str = "\u{EAE8}";

/// Every icon above, for the test that the font has them.
#[cfg(test)]
pub const ALL: [&str; 33] = [
    ARROW_DOWN,
    ARROW_UP,
    ARROWS_CLOCKWISE,
    BELL,
    CARET_DOWN,
    CHECK_CIRCLE,
    CLIPBOARD,
    COPY,
    DATABASE,
    ENVELOPE,
    ERASER,
    EXPORT,
    GEAR,
    HOURGLASS,
    INFO,
    KEYBOARD,
    LIGHTNING,
    LIST,
    LOCK,
    MAGNIFYING_GLASS,
    PENCIL,
    PLAY,
    PLUGS_CONNECTED,
    PLUS,
    POWER,
    PRINTER,
    QUESTION,
    RECORD,
    RESET,
    SIGN_OUT,
    SQUARES_FOUR,
    STOP,
    TERMINAL,
];

/// `icon` as text in the icon font, to colour or size like any [`RichText`].
pub fn icon(icon: &str) -> RichText {
    RichText::new(icon).family(FontFamily::Name(FAMILY.into()))
}

/// A button with `icon` before `text`.
pub fn button<'a>(icon: &str, text: impl Into<WidgetText>) -> Button<'a> {
    Button::new((self::icon(icon), text.into()))
}

/// `icon` then `text` as one label of one line's height, both in `text`'s colour unless
/// `icon` has its own.
pub fn label(ui: &mut Ui, icon: RichText, text: RichText) -> Response {
    let style = std::sync::Arc::clone(ui.style());
    let mut job = egui::text::LayoutJob::default();
    let font = egui::FontSelection::Default;
    icon.append_to(&mut job, &style, font.clone(), egui::Align::Center);
    RichText::new("  ").append_to(&mut job, &style, font.clone(), egui::Align::Center);
    text.append_to(&mut job, &style, font, egui::Align::Center);
    ui.add(egui::Label::new(job).selectable(false).truncate())
}

/// Adds `button`, named `name` for screen readers and tests rather than by its icon's
/// codepoint.
pub fn add_named(ui: &mut Ui, enabled: bool, button: Button<'_>, name: &str) -> Response {
    let response = ui.add_enabled(enabled, button);
    response.widget_info(|| WidgetInfo::labeled(Role::Button, enabled, name));
    response
}
