//! The settings dialog's frame, shared by both programs (R-UI-18, D12): the list of pages on
//! the left, the page on the right as groups of rows, and Restore defaults, Cancel and Save.
//! Laid out as Tortuga's.

use egui::{Align, CornerRadius, Frame, Id, Layout, Margin, RichText, Stroke, Ui, vec2};

use crate::{dialog, theme};

/// Width of the list of pages.
const NAV_WIDTH: f32 = 168.0;

/// A page of a program's settings dialog.
pub trait SettingsPage:
    Copy + Eq + std::hash::Hash + std::fmt::Debug + Send + Sync + 'static
{
    /// Its name in the list.
    fn name(self) -> &'static str;
}

/// The pages under their headings, in the order the list shows them.
pub type Sections<P> = &'static [(&'static str, &'static [P])];

/// How the dialog closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Closed {
    Keep,
    /// Put the original settings back.
    Revert,
}

/// What was clicked at the foot of the dialog (or its close button).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Clicked {
    /// Save: keep the settings as they are now.
    Keep,
    /// Cancel or the close button: put the original settings back.
    Revert,
    /// Restore defaults, for the page shown.
    RestoreDefaults,
}

/// Draws the settings dialog on `page` of `sections`; `content` draws a page.
pub fn frame<P: SettingsPage>(
    ctx: &egui::Context,
    page: &mut P,
    sections: Sections<P>,
    content: impl FnOnce(&mut Ui, P),
) -> Option<Clicked> {
    let (close, clicked) = dialog::dialog(ctx, "settings", "Settings", 760.0, |ui| {
        // One size for every page, so the dialog doesn't jump when the page changes.
        let height = (ui.ctx().content_rect().height() - 210.0).clamp(260.0, 500.0);
        let (rect, _) =
            ui.allocate_exact_size(vec2(ui.available_width(), height), egui::Sense::hover());
        let nav_rect = egui::Rect::from_min_size(rect.min, vec2(NAV_WIDTH, height));
        let page_rect =
            egui::Rect::from_min_max(egui::pos2(nav_rect.right() + 16.0, rect.top()), rect.max);
        let mut nav_ui = ui.new_child(egui::UiBuilder::new().max_rect(nav_rect));
        nav(&mut nav_ui, page, sections, height);
        let mut page_ui = ui.new_child(egui::UiBuilder::new().max_rect(page_rect));
        // Nothing too wide for the page paints outside the dialog.
        page_ui.set_clip_rect(page_rect.expand2(vec2(0.0, 4.0)));
        let shown = *page;
        egui::ScrollArea::vertical()
            .id_salt(("settings-page", shown))
            .auto_shrink([false, false])
            .show(&mut page_ui, |ui| {
                ui.add_space(4.0);
                content(ui, shown);
            });
        ui.add_space(16.0);
        ui.horizontal(foot).inner
    });
    if close {
        Some(Clicked::Revert)
    } else {
        clicked
    }
}

/// Restore defaults on the left; Cancel and Save on the right.
fn foot(ui: &mut Ui) -> Option<Clicked> {
    let restore = ui
        .button("Restore defaults")
        .on_hover_text("Puts this page's settings back to their defaults.")
        .clicked();
    let save = theme::primary_button(ui, "Save");
    let (cancel, save) = theme::button_row(ui, |ui| {
        let save = ui.add(save).clicked();
        (ui.button("Cancel").clicked(), save)
    });
    match (restore, cancel, save) {
        (_, _, true) => Some(Clicked::Keep),
        (_, true, false) => Some(Clicked::Revert),
        (true, false, false) => Some(Clicked::RestoreDefaults),
        (false, false, false) => None,
    }
}

/// The list of pages, `height` tall; a click shows another page.
fn nav<P: SettingsPage>(ui: &mut Ui, page: &mut P, sections: Sections<P>, height: f32) {
    let p = theme::palette(ui);
    Frame::new()
        .fill(p.chrome)
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::same(8))
        .show(ui, |ui| {
            ui.set_width(NAV_WIDTH - 16.0);
            ui.set_height(height - 16.0);
            ui.spacing_mut().item_spacing.y = 2.0;
            for (i, (heading, pages)) in sections.iter().enumerate() {
                if i > 0 {
                    ui.add_space(10.0);
                }
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    theme::section_label(ui, heading);
                });
                ui.add_space(2.0);
                for &value in *pages {
                    nav_item(ui, page, value);
                }
            }
        });
}

fn nav_item<P: SettingsPage>(ui: &mut Ui, page: &mut P, value: P) {
    let p = theme::palette(ui);
    let chosen = *page == value;
    let name = value.name();
    let text = if chosen {
        theme::semibold(name).color(p.text)
    } else {
        RichText::new(name).color(p.muted)
    };
    let mut button = egui::Button::new((text, egui::Atom::grow()))
        .min_size(vec2(ui.available_width(), 28.0))
        .corner_radius(CornerRadius::same(6));
    button = if chosen {
        button.fill(p.bg).stroke(Stroke::new(1.0, p.border))
    } else {
        button.frame_when_inactive(false)
    };
    let response = ui.add(button);
    response.widget_info(|| egui::WidgetInfo::selected(egui::Role::Tab, true, chosen, name));
    if chosen {
        let r = response.rect;
        let bar = egui::Rect::from_min_size(
            egui::pos2(r.left() + 1.0, r.top() + 7.0),
            vec2(3.0, r.height() - 14.0),
        );
        ui.painter().rect_filled(bar, 1.5, p.accent);
    }
    if response.clicked() {
        *page = value;
    }
}

/// A titled group of rows on a card; `note` is a short line under the title and a longer
/// one on hover, as Tortuga's.
pub fn group<R>(
    ui: &mut Ui,
    title: &str,
    note: Option<(&str, &str)>,
    add: impl FnOnce(&mut Rows<'_>) -> R,
) -> R {
    let p = theme::palette(ui);
    theme::section_label(ui, title);
    if let Some((short, long)) = note {
        ui.add(egui::Label::new(RichText::new(short).small().color(p.muted)).wrap())
            .on_hover_text(long);
    }
    ui.add_space(4.0);
    let inner = Frame::new()
        .fill(p.bg)
        .stroke(Stroke::new(1.0, p.border))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::symmetric(14, 0))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            let label_width = (ui.available_width() * 0.42).clamp(140.0, 230.0);
            add(&mut Rows {
                ui,
                first: true,
                label_width,
            })
        })
        .inner;
    ui.add_space(18.0);
    inner
}

/// The rows of a [`group`].
pub struct Rows<'a> {
    ui: &'a mut Ui,
    first: bool,
    label_width: f32,
}

impl std::fmt::Debug for Rows<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Rows")
            .field("first", &self.first)
            .finish_non_exhaustive()
    }
}

impl Rows<'_> {
    /// A setting: its name and a short line about it on the left, `control` beside them.
    /// `control` gets the name's id, to name its widget with `labelled_by`.
    pub fn row<R>(
        &mut self,
        name: &str,
        about: Option<&str>,
        control: impl FnOnce(&mut Ui, Id) -> R,
    ) -> R {
        self.divider();
        self.ui.add_space(10.0);
        let width = self.label_width;
        let inner = self
            .ui
            .horizontal_top(|ui| {
                // The name lines up with the text of a button or field beside it.
                let text = ui.text_style_height(&egui::TextStyle::Body);
                let pad = ((ui.spacing().interact_size.y - text) / 2.0).max(0.0);
                let name_id = ui
                    .allocate_ui_with_layout(vec2(width, 0.0), Layout::top_down(Align::Min), |ui| {
                        ui.set_width(width);
                        ui.add_space(pad);
                        let id = ui.label(name).id;
                        if let Some(about) = about {
                            theme::hint(ui, about);
                        }
                        id
                    })
                    .inner;
                ui.add_space(12.0);
                ui.vertical(|ui| control(ui, name_id)).inner
            })
            .inner;
        self.ui.add_space(10.0);
        inner
    }

    /// A setting that is on or off: its name, and a switch named by it.
    pub fn switch(&mut self, name: &str, about: Option<&str>, on: &mut bool) {
        self.row(name, about, |ui, name_id| {
            theme::switch(ui, on).labelled_by(name_id);
        });
    }

    /// A fact, not a setting: its name and `value` as text side by side, on a lower row.
    pub fn fact(&mut self, name: &str, value: impl FnOnce(&mut Ui)) {
        self.divider();
        self.ui.add_space(7.0);
        let width = self.label_width;
        let muted = theme::palette(self.ui).muted;
        self.ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(vec2(width, 0.0), Layout::top_down(Align::Min), |ui| {
                ui.set_width(width);
                ui.label(RichText::new(name).color(muted));
            });
            ui.add_space(12.0);
            ui.vertical(value);
        });
        self.ui.add_space(7.0);
    }

    /// A row across the whole card, for what isn't a setting (a preview, a note).
    pub fn full_row<R>(&mut self, add: impl FnOnce(&mut Ui) -> R) -> R {
        self.divider();
        self.ui.add_space(10.0);
        let inner = add(self.ui);
        self.ui.add_space(10.0);
        inner
    }

    /// A line between rows; none above the first.
    fn divider(&mut self) {
        if std::mem::take(&mut self.first) {
            return;
        }
        let p = theme::palette(self.ui);
        let rect = self.ui.available_rect_before_wrap();
        let y = self.ui.cursor().top();
        self.ui
            .painter()
            .hline(rect.x_range(), y, Stroke::new(1.0, p.border));
    }
}
