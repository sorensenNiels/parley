//! Parley's look, shared with Tortuga (D12): fonts, colours, the global egui style, and small
//! building blocks (section labels, hints, cards, filled buttons, switches, segmented choices).
//! Every colour has a light and a dark variant; egui follows the system theme unless the
//! settings say otherwise. The 5250 screen keeps its own colours (`terminal::color`).
//!
//! [`install`] runs once per `egui::Context` before drawing (the first frame of
//! [`crate::ParleyUi::ui`]).

use std::sync::Arc;

use egui::epaint::Shadow;
use egui::{
    Color32, Context, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Frame, Margin,
    Response, RichText, Role, Stroke, TextStyle, Theme, Ui, Visuals, WidgetInfo,
};
use serde::{Deserialize, Serialize};

use crate::fonts;

/// Named font family for headings and emphasis (Inter `SemiBold`).
pub const SEMIBOLD: &str = "semibold";
/// The icon font's name among the fonts (R-UI-13, D17).
const PHOSPHOR: &str = "Phosphor";

/// Width of a scroll bar while the pointer is on it (floating bars are thinner otherwise).
const SCROLL_BAR_WIDTH: f32 = 6.0;

/// Corner radius of inputs and buttons.
const RADIUS: u8 = 6;

/// How much room there is around controls.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Density {
    #[default]
    Comfortable,
    Compact,
}

/// Parley's colours for the current light/dark mode.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    /// Main content background.
    pub bg: Color32,
    /// Menu bar, toolbar, status bar and side list background.
    pub chrome: Color32,
    /// Inputs, secondary buttons.
    pub surface: Color32,
    pub surface_hover: Color32,
    pub border: Color32,
    pub text: Color32,
    pub muted: Color32,
    /// Accent for text, lines and outlines.
    pub accent: Color32,
    /// Accent as a fill behind white text (primary buttons).
    pub accent_fill: Color32,
    pub ok: Color32,
    pub fail: Color32,
    pub warning_text: Color32,
    /// Code colours, as Tortuga's (D12): amber (numbers), blue (keywords), purple (names).
    pub redirect: Color32,
    pub blue: Color32,
    pub purple: Color32,
}

const LIGHT: Palette = Palette {
    bg: Color32::from_rgb(0xff, 0xff, 0xff),
    chrome: Color32::from_rgb(0xf6, 0xf7, 0xf9),
    surface: Color32::from_rgb(0xf3, 0xf4, 0xf6),
    surface_hover: Color32::from_rgb(0xe8, 0xea, 0xee),
    border: Color32::from_rgb(0xe2, 0xe5, 0xea),
    text: Color32::from_rgb(0x11, 0x18, 0x27),
    muted: Color32::from_rgb(0x6b, 0x72, 0x80),
    accent: Color32::from_rgb(0x0d, 0x94, 0x88),
    accent_fill: Color32::from_rgb(0x0d, 0x94, 0x88),
    ok: Color32::from_rgb(0x05, 0x96, 0x69),
    fail: Color32::from_rgb(0xdc, 0x26, 0x26),
    warning_text: Color32::from_rgb(0x92, 0x40, 0x0e),
    redirect: Color32::from_rgb(0xd9, 0x77, 0x06),
    blue: Color32::from_rgb(0x25, 0x63, 0xeb),
    purple: Color32::from_rgb(0x93, 0x33, 0xea),
};

const DARK: Palette = Palette {
    bg: Color32::from_rgb(0x13, 0x15, 0x1a),
    chrome: Color32::from_rgb(0x0d, 0x0f, 0x13),
    surface: Color32::from_rgb(0x1c, 0x1f, 0x26),
    surface_hover: Color32::from_rgb(0x25, 0x29, 0x32),
    border: Color32::from_rgb(0x2a, 0x2f, 0x3a),
    text: Color32::from_rgb(0xe7, 0xe9, 0xee),
    muted: Color32::from_rgb(0x8b, 0x93, 0xa7),
    accent: Color32::from_rgb(0x2d, 0xd4, 0xbf),
    accent_fill: Color32::from_rgb(0x0d, 0x94, 0x88),
    ok: Color32::from_rgb(0x34, 0xd3, 0x99),
    fail: Color32::from_rgb(0xf8, 0x71, 0x71),
    warning_text: Color32::from_rgb(0xfc, 0xd3, 0x4d),
    redirect: Color32::from_rgb(0xfb, 0xbf, 0x24),
    blue: Color32::from_rgb(0x60, 0xa5, 0xfa),
    purple: Color32::from_rgb(0xc0, 0x84, 0xfc),
};

pub fn palette(ui: &Ui) -> Palette {
    if ui.visuals().dark_mode { DARK } else { LIGHT }
}

// ---------------------------------------------------------------- setup

/// Installs Parley's fonts (with `terminal_font` for the screen) and light/dark styles.
/// Returns a note when the terminal font could not be loaded.
pub fn install(ctx: &Context, terminal_font: &str) -> Option<String> {
    let note = set_terminal_font(ctx, terminal_font);
    set_sizes(ctx, Density::Comfortable);
    ctx.all_styles_mut(|style| {
        fixed_spacing(&mut style.spacing);
        style.interaction.tooltip_delay = 0.3;
    });
    ctx.style_mut_of(Theme::Dark, |s| s.visuals = visuals(&DARK, true));
    ctx.style_mut_of(Theme::Light, |s| s.visuals = visuals(&LIGHT, false));
    note
}

/// Spacing that doesn't change with the density.
fn fixed_spacing(s: &mut egui::style::Spacing) {
    s.menu_margin = Margin::same(6);
    s.window_margin = Margin::same(20);
    s.combo_height = 320.0;
    s.scroll = egui::style::ScrollStyle {
        // Thin until hovered, then 6 px rather than egui's 10.
        bar_width: SCROLL_BAR_WIDTH,
        ..egui::style::ScrollStyle::floating()
    };
}

/// Text sizes, and the room around controls for `density`.
pub fn set_sizes(ctx: &Context, density: Density) {
    ctx.all_styles_mut(|style| {
        use TextStyle::{Body, Button, Heading, Monospace, Small};
        let semibold = FontFamily::Name(SEMIBOLD.into());
        style.text_styles = [
            (Small, FontId::proportional(11.5)),
            (Body, FontId::proportional(13.5)),
            (Button, FontId::proportional(13.5)),
            (Heading, FontId::new(17.0, semibold)),
            (Monospace, FontId::monospace(12.5)),
        ]
        .into();
        let s = &mut style.spacing;
        match density {
            Density::Comfortable => {
                s.item_spacing = egui::vec2(8.0, 6.0);
                s.button_padding = egui::vec2(10.0, 4.0);
                s.interact_size.y = 28.0;
            }
            Density::Compact => {
                s.item_spacing = egui::vec2(6.0, 3.0);
                s.button_padding = egui::vec2(7.0, 2.0);
                s.interact_size.y = 22.0;
            }
        }
    });
}

/// Uses `terminal_font` for the 5250 screen; returns a note when it is not installed and the
/// default stands in.
pub fn set_terminal_font(ctx: &Context, terminal_font: &str) -> Option<String> {
    let (definitions, note) = font_definitions(terminal_font);
    ctx.set_fonts(definitions);
    note
}

/// Inter for the UI, Inter `SemiBold` for headings, `JetBrains Mono` for monospace text, and
/// `terminal_font` (bundled or installed) for the terminal.
pub fn font_definitions(terminal_font: &str) -> (FontDefinitions, Option<String>) {
    let mut fonts = FontDefinitions::default();
    let mut add = |name: &str, bytes: &'static [u8]| {
        fonts
            .font_data
            .insert(name.to_owned(), Arc::new(FontData::from_static(bytes)));
    };
    add("Inter", include_bytes!("../assets/fonts/Inter-Regular.ttf"));
    add(
        "Inter-SemiBold",
        include_bytes!("../assets/fonts/Inter-SemiBold.ttf"),
    );
    add(
        fonts::JETBRAINS_MONO,
        include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf"),
    );
    add(
        fonts::PLEX_MONO,
        include_bytes!("../assets/fonts/IBMPlexMono-Regular.ttf"),
    );

    // The icons (R-UI-13, D17), in a family of their own: see `icons`.
    add(
        PHOSPHOR,
        include_bytes!("../assets/fonts/Phosphor-Regular.ttf"),
    );

    // Keep egui's bundled fonts after ours: they hold the symbols (⏵ ⚠ 🗙 …).
    let fallbacks = fonts.families[&FontFamily::Proportional].clone();
    let with = |first: &str| [vec![first.to_owned()], fallbacks.clone()].concat();
    fonts
        .families
        .insert(FontFamily::Proportional, with("Inter"));
    fonts
        .families
        .insert(FontFamily::Name(SEMIBOLD.into()), with("Inter-SemiBold"));
    let icons = FontFamily::Name(crate::icons::FAMILY.into());
    fonts.families.insert(icons, vec![PHOSPHOR.to_owned()]);
    let mono = fonts.families.entry(FontFamily::Monospace).or_default();
    mono.insert(0, fonts::JETBRAINS_MONO.to_owned());

    let note = fonts::add_terminal_family(&mut fonts, terminal_font);
    (fonts, note)
}

fn visuals(p: &Palette, dark: bool) -> Visuals {
    let mut v = if dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };
    let radius = CornerRadius::same(RADIUS);
    let border = Stroke::new(1.0, p.border);

    v.panel_fill = p.bg;
    v.window_fill = if dark { p.surface } else { p.bg };
    v.window_stroke = border;
    v.window_corner_radius = CornerRadius::same(12);
    v.menu_corner_radius = CornerRadius::same(8);
    let shadow_alpha = if dark { 140 } else { 40 };
    v.window_shadow = Shadow {
        offset: [0, 12],
        blur: 32,
        spread: 0,
        color: Color32::from_black_alpha(shadow_alpha),
    };
    v.popup_shadow = Shadow {
        offset: [0, 6],
        blur: 16,
        spread: 0,
        color: Color32::from_black_alpha(shadow_alpha),
    };
    v.extreme_bg_color = p.surface;
    v.text_edit_bg_color = Some(p.surface);
    v.code_bg_color = p.surface;
    v.faint_bg_color = p.chrome;
    v.hyperlink_color = p.accent;
    v.warn_fg_color = p.warning_text;
    v.error_fg_color = p.fail;
    v.selection.bg_fill = p.accent.gamma_multiply(if dark { 0.22 } else { 0.16 });
    v.selection.stroke = Stroke::new(1.0, p.accent);
    v.text_cursor.stroke = Stroke::new(2.0, p.accent);
    v.indent_has_left_vline = false;

    let w = &mut v.widgets;
    w.noninteractive.bg_fill = p.bg;
    w.noninteractive.weak_bg_fill = p.bg;
    w.noninteractive.bg_stroke = border;
    w.noninteractive.fg_stroke = Stroke::new(1.0, p.text);
    w.noninteractive.corner_radius = radius;

    w.inactive.bg_fill = p.surface;
    w.inactive.weak_bg_fill = p.surface;
    w.inactive.bg_stroke = border;
    w.inactive.fg_stroke = Stroke::new(1.0, p.text);
    w.inactive.corner_radius = radius;
    w.inactive.expansion = 0.0;

    w.hovered.bg_fill = p.surface_hover;
    w.hovered.weak_bg_fill = p.surface_hover;
    w.hovered.bg_stroke = Stroke::new(1.0, p.muted.gamma_multiply(0.6));
    w.hovered.fg_stroke = Stroke::new(1.0, p.text);
    w.hovered.corner_radius = radius;
    w.hovered.expansion = 0.0;

    w.active.bg_fill = p.surface_hover;
    w.active.weak_bg_fill = p.surface_hover;
    w.active.bg_stroke = Stroke::new(1.0, p.accent);
    w.active.fg_stroke = Stroke::new(1.0, p.text);
    w.active.corner_radius = radius;
    w.active.expansion = 0.0;

    w.open = w.active;
    w.open.bg_stroke = border;
    v
}

// ---------------------------------------------------------------- text

pub fn semibold(text: impl Into<String>) -> RichText {
    RichText::new(text).family(FontFamily::Name(SEMIBOLD.into()))
}

/// A row of buttons from the right, as tall as a button. A plain
/// `right_to_left(Align::Center)` layout takes all the height left to centre in, so a
/// dialog that was ever taller stays that tall.
pub fn button_row<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let size = egui::vec2(ui.available_width(), ui.spacing().interact_size.y);
    ui.allocate_ui_with_layout(size, egui::Layout::right_to_left(egui::Align::Center), add)
        .inner
}

/// Small upper-case label above a group (e.g. "CONNECTION").
pub fn section_label(ui: &mut Ui, text: &str) {
    let color = palette(ui).muted;
    ui.label(semibold(text.to_uppercase()).size(10.5).color(color));
}

/// Secondary explanatory text.
pub fn hint(ui: &mut Ui, text: impl Into<String>) {
    let color = palette(ui).muted;
    ui.label(RichText::new(text).small().color(color));
}

pub fn warning(ui: &mut Ui, text: impl Into<String>) {
    let color = palette(ui).warning_text;
    ui.label(RichText::new(text).small().color(color));
}

pub fn error(ui: &mut Ui, text: impl Into<String>) {
    let color = palette(ui).fail;
    ui.label(RichText::new(text).small().color(color));
}

// ---------------------------------------------------------------- frames

/// A bordered block for content (a saved session on the start screen).
pub fn card(ui: &Ui) -> Frame {
    let p = palette(ui);
    Frame::new()
        .fill(
            p.surface
                .gamma_multiply(if ui.visuals().dark_mode { 0.6 } else { 0.5 }),
        )
        .stroke(Stroke::new(1.0, p.border))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::same(10))
}

/// The menu bar, toolbar and status bar: the chrome colour, a border on the inside edge.
pub fn bar(ui: &Ui, margin: Margin) -> Frame {
    let p = palette(ui);
    Frame::new().fill(p.chrome).inner_margin(margin)
}

/// A rounded, tinted pill (the keyboard state, "Message waiting", "REC"), as Tortuga's.
pub fn badge(ui: &mut Ui, text: impl Into<String>, color: Color32) -> Response {
    badge_frame(color)
        .show(ui, |ui| {
            ui.add(egui::Label::new(semibold(text).size(12.0).color(color)).truncate())
        })
        .inner
}

/// A [`badge`] with an icon before its text (R-UI-13).
pub fn icon_badge(ui: &mut Ui, icon: &str, text: impl Into<String>, color: Color32) -> Response {
    badge_frame(color)
        .show(ui, |ui| {
            let icon = crate::icons::icon(icon).size(12.0).color(color);
            crate::icons::label(ui, icon, semibold(text).size(12.0).color(color))
        })
        .inner
}

fn badge_frame(color: Color32) -> Frame {
    Frame::new()
        .fill(color.gamma_multiply(0.14))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::symmetric(8, 2))
}

/// The keys of a shortcut as key caps, one per key: "Ctrl+Shift+F1" is three caps.
pub fn kbd(ui: &mut Ui, keys: &str) {
    let p = palette(ui);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 3.0;
        for key in key_parts(keys) {
            Frame::new()
                .fill(p.bg)
                .stroke(Stroke::new(1.0, p.border))
                .corner_radius(CornerRadius::same(4))
                .inner_margin(Margin::symmetric(5, 1))
                .show(ui, |ui| {
                    let text = RichText::new(key).monospace().size(11.0).color(p.muted);
                    ui.add(egui::Label::new(text).selectable(false));
                });
        }
    });
}

/// The keys of a shortcut as written ("Ctrl+Shift+F1", "Ctrl++"), one by one.
pub fn key_parts(keys: &str) -> Vec<&str> {
    if keys == "+" {
        return vec!["+"];
    }
    match keys.strip_suffix("++") {
        Some(rest) => rest.split('+').chain(["+"]).collect(),
        None => keys.split('+').collect(),
    }
}

/// A tab: its value, label and an optional badge such as a count.
pub type Tab<T, S> = (T, S, Option<String>);

/// Underlined tabs on a full-width rule, as Tortuga's (D12), that scroll sideways, then
/// `rest` (small buttons) on the same row, all centred on one line, with the rule under the
/// whole row so a tab's underline never reaches what comes below. `rest_width` is the room kept for `rest`. Returns the tab clicked, if another than
/// `current`, and what `rest` returned.
pub fn tab_strip<T: PartialEq + Copy, S: AsRef<str>, R>(
    ui: &mut Ui,
    id_salt: &str,
    (current, items): (T, &[Tab<T, S>]),
    rest_width: f32,
    rest: impl FnOnce(&mut Ui) -> R,
) -> (Option<T>, R) {
    let mut chosen = current;
    let mut underlines = Vec::new();
    let row = ui.horizontal(|ui| {
        let width = (ui.available_width() - rest_width).max(120.0);
        // The row's height, not 0: a centred row would put a 0-high block at its middle and the
        // tabs would hang below the buttons.
        let height = ui.spacing().interact_size.y;
        ui.allocate_ui(egui::vec2(width, height), |ui| {
            egui::ScrollArea::horizontal()
                .id_salt(id_salt)
                .show(ui, |ui| {
                    // Only what shows of a tab scrolled partly out of view is underlined.
                    let shown = ui.clip_rect().x_range();
                    underlines = ui
                        .horizontal(|ui| tab_buttons(ui, &mut chosen, items))
                        .inner
                        .into_iter()
                        .map(|(x, color)| (x.intersection(shown), color))
                        .filter(|(x, _)| x.span() > 0.0)
                        .collect();
                });
        });
        rest(ui)
    });
    tab_rule(ui, row.response.rect.bottom(), &underlines);
    ((chosen != current).then_some(chosen), row.inner)
}

/// One frameless button per tab, in a row; the underlines to draw (selected or hovered).
fn tab_buttons<T: PartialEq + Copy, S: AsRef<str>>(
    ui: &mut Ui,
    current: &mut T,
    items: &[Tab<T, S>],
) -> Vec<(egui::Rangef, Color32)> {
    let p = palette(ui);
    let mut underlines = Vec::new();
    ui.spacing_mut().item_spacing.x = 20.0;
    for (value, label, badge) in items {
        let selected = *current == *value;
        let color = if selected { p.text } else { p.muted };
        // One real button per tab, so clicks work everywhere (also inside dialogs).
        let label = label.as_ref();
        let mut atoms = vec![egui::Atom::from(semibold(label).color(color))];
        if let Some(b) = badge {
            let c = if selected { p.accent } else { p.muted };
            atoms.push(egui::Atom::from(semibold(b).size(10.5).color(c)));
        }
        let atoms: egui::Atoms<'_> = atoms.into_iter().collect();
        let response = ui.add(egui::Button::new(atoms).frame(false).gap(5.0));
        let name = match badge {
            Some(b) => format!("{label} ({b})"),
            None => label.to_owned(),
        };
        response.widget_info(|| WidgetInfo::selected(Role::Tab, true, selected, &name));
        if selected {
            underlines.push((response.rect.x_range(), p.accent));
        } else if response.hovered() {
            underlines.push((response.rect.x_range(), p.muted.gamma_multiply(0.5)));
        }
        if response.clicked() {
            *current = *value;
        }
        if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
    }
    underlines
}

/// The full-width rule a little under `bottom`, the tabs' underlines on it, and room below.
fn tab_rule(ui: &mut Ui, bottom: f32, underlines: &[(egui::Rangef, Color32)]) {
    let p = palette(ui);
    let y = bottom + 6.0;
    let painter = ui.painter();
    painter.hline(ui.max_rect().x_range(), y, Stroke::new(1.0, p.border));
    for (x, color) in underlines {
        painter.hline(*x, y - 0.5, Stroke::new(2.0, *color));
    }
    ui.add_space(12.0);
}

/// Adds a text field of exactly `width`. Inside a [`egui::Grid`] a plain
/// `desired_width` is capped by last frame's column width, so cells would never grow.
pub fn sized_edit(ui: &mut Ui, width: f32, edit: egui::TextEdit<'_>) -> Response {
    let height = ui.spacing().interact_size.y;
    ui.add_sized([width, height], edit.align(egui::Align2::LEFT_CENTER))
}

// ---------------------------------------------------------------- buttons

/// A filled button in the accent colour (Connect, Enter, dialog confirmations).
pub fn primary_button<'a>(ui: &Ui, text: impl Into<String>) -> egui::Button<'a> {
    filled_button(text, palette(ui).accent_fill)
}

/// A filled button in the danger colour (Delete).
pub fn danger_button<'a>(ui: &Ui, text: impl Into<String>) -> egui::Button<'a> {
    filled_button(text, palette(ui).fail)
}

fn filled_button<'a>(text: impl Into<String>, fill: Color32) -> egui::Button<'a> {
    egui::Button::new(semibold(text).color(Color32::WHITE))
        .fill(fill)
        .stroke(Stroke::NONE)
}

/// A small, muted icon button (close, dismiss).
pub fn icon_button<'a>(ui: &Ui, glyph: &str) -> egui::Button<'a> {
    ghost_button(RichText::new(glyph).size(11.0).color(palette(ui).muted))
}

/// A borderless button that only shows a background on hover (toolbar keys).
pub fn ghost_button<'a>(text: impl Into<egui::WidgetText>) -> egui::Button<'a> {
    egui::Button::new(text).frame_when_inactive(false)
}

// ---------------------------------------------------------------- choices

/// An on/off switch; a click flips `on`. Name it with `labelled_by`.
pub fn switch(ui: &mut Ui, on: &mut bool) -> Response {
    let height = ui.spacing().interact_size.y * 0.8;
    let (rect, mut response) =
        ui.allocate_exact_size(egui::vec2(height * 1.8, height), egui::Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    let enabled = ui.is_enabled();
    let state = *on;
    response.widget_info(|| WidgetInfo::selected(Role::CheckBox, enabled, state, ""));
    if ui.is_rect_visible(rect) {
        let p = palette(ui);
        let how_on = ui.ctx().animate_bool_responsive(response.id, *on);
        let off_fill = if response.hovered() {
            p.muted.gamma_multiply(0.6)
        } else {
            p.muted.gamma_multiply(0.4)
        };
        let fill = lerp_color(off_fill, p.accent_fill, how_on);
        let radius = rect.height() / 2.0;
        ui.painter().rect_filled(rect, radius, fill);
        let x = egui::lerp((rect.left() + radius)..=(rect.right() - radius), how_on);
        let knob = egui::pos2(x, rect.center().y);
        ui.painter()
            .circle_filled(knob, radius - 2.5, Color32::WHITE);
        if response.has_focus() {
            ui.painter().rect_stroke(
                rect.expand(2.0),
                radius + 2.0,
                Stroke::new(1.0, p.accent),
                egui::StrokeKind::Outside,
            );
        }
    }
    response
}

/// A [`switch`] with its name beside it; a click on the name flips it too. Returns the
/// switch's response, named by the text for screen readers.
pub fn switch_row(ui: &mut Ui, text: &str, on: &mut bool) -> Response {
    ui.horizontal(|ui| {
        let mut response = switch(ui, on);
        let label = ui.add(egui::Label::new(text).sense(egui::Sense::click()));
        if label.clicked() {
            *on = !*on;
            response.mark_changed();
        }
        response.labelled_by(label.id)
    })
    .inner
}

fn lerp_color(from: Color32, to: Color32, t: f32) -> Color32 {
    let mix = |a: u8, b: u8| {
        let v = f32::from(a) + (f32::from(b) - f32::from(a)) * t;
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "0..=255"
        )]
        let v = v.round().clamp(0.0, 255.0) as u8;
        v
    };
    Color32::from_rgba_premultiplied(
        mix(from.r(), to.r()),
        mix(from.g(), to.g()),
        mix(from.b(), to.b()),
        mix(from.a(), to.a()),
    )
}

/// One of a few choices as joined buttons, the chosen one raised. Returns whether the
/// choice changed.
pub fn segmented<T: PartialEq + Copy>(ui: &mut Ui, current: &mut T, options: &[(T, &str)]) -> bool {
    let p = palette(ui);
    let mut changed = false;
    Frame::new()
        .fill(p.surface)
        .stroke(Stroke::new(1.0, p.border))
        .corner_radius(CornerRadius::same(7))
        .inner_margin(Margin::same(2))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                for &(value, text) in options {
                    let chosen = *current == value;
                    let label = if chosen {
                        semibold(text).color(p.text)
                    } else {
                        RichText::new(text).color(p.muted)
                    };
                    let mut button = egui::Button::new(label).corner_radius(CornerRadius::same(5));
                    button = if chosen {
                        button.fill(p.bg).stroke(Stroke::new(1.0, p.border))
                    } else {
                        button.frame_when_inactive(false)
                    };
                    let response = ui.add(button);
                    response.widget_info(|| {
                        WidgetInfo::selected(Role::RadioButton, true, chosen, text)
                    });
                    if response.clicked() && !chosen {
                        *current = value;
                        changed = true;
                    }
                }
            });
        });
    changed
}

#[cfg(test)]
mod tests {
    /// Scroll bars float over the content and stay narrow when hovered.
    #[test]
    fn scroll_bars_are_thin_and_float() {
        let ctx = egui::Context::default();
        super::install(&ctx, crate::fonts::DEFAULT_TERMINAL_FONT);
        for theme in [egui::Theme::Light, egui::Theme::Dark] {
            let scroll = &ctx.style_of(theme).spacing.scroll;
            assert!(scroll.floating, "{theme:?}");
            assert!((scroll.bar_width - 6.0).abs() < f32::EPSILON, "{theme:?}");
        }
    }

    #[test]
    fn headings_use_inter_semibold_and_body_text_inter() {
        // D12: the same type as Tortuga.
        let (fonts, _) = super::font_definitions(crate::fonts::DEFAULT_TERMINAL_FONT);
        assert_eq!(fonts.families[&egui::FontFamily::Proportional][0], "Inter");
        let semibold = egui::FontFamily::Name(super::SEMIBOLD.into());
        assert_eq!(fonts.families[&semibold][0], "Inter-SemiBold");
        assert_eq!(
            fonts.families[&egui::FontFamily::Monospace][0],
            crate::fonts::JETBRAINS_MONO
        );
    }

    #[test]
    fn icons_have_a_family_of_their_own_and_stay_out_of_text_r_ui_13() {
        // D17: Inter has glyphs of its own at some of Phosphor's codepoints, and Phosphor
        // maps a–z to blanks, so neither may stand in for the other.
        let (fonts, _) = super::font_definitions(crate::fonts::DEFAULT_TERMINAL_FONT);
        let icons = egui::FontFamily::Name(crate::icons::FAMILY.into());
        assert_eq!(fonts.families[&icons], [super::PHOSPHOR]);
        let semibold = egui::FontFamily::Name(super::SEMIBOLD.into());
        for family in [egui::FontFamily::Proportional, semibold] {
            assert!(!fonts.families[&family].iter().any(|f| f == super::PHOSPHOR));
        }
    }

    #[test]
    fn a_shortcut_splits_into_one_key_cap_per_key() {
        // R-UI-17
        use super::key_parts;
        assert_eq!(key_parts("Ctrl+Shift+F1"), ["Ctrl", "Shift", "F1"]);
        assert_eq!(key_parts("Ctrl++"), ["Ctrl", "+"]);
        assert_eq!(key_parts("+"), ["+"]);
        assert_eq!(key_parts("Esc"), ["Esc"]);
        assert_eq!(key_parts("Ctrl+Page Up"), ["Ctrl", "Page Up"]);
    }
}
