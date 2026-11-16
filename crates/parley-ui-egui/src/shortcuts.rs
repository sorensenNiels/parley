//! The keyboard shortcut sheet (R-UI-17): the keys of the map in use, under the same headings
//! as Settings ▸ Keyboard, as key caps, with a filter. As Tortuga's shortcut sheet, opened by
//! the `shortcuts` function (Ctrl+/ in the Default map) since F1 goes to the host here.

use egui::{Context, Event, RichText, TextEdit};

use crate::keymap::{Chord, Function, KeyMap};
use crate::theme;
use crate::{dialog, icons};

/// One heading of the sheet and its lines: what a function is called and its keys.
pub type Group = (&'static str, Vec<(String, Vec<String>)>);

/// The sheet while it is open.
#[derive(Debug, Default)]
pub struct Sheet {
    filter: String,
    /// The list's height last frame (see `dialog::fitting_scroll_area`).
    height: f32,
    /// The filter field has had the focus once.
    focused: bool,
}

/// What became of the sheet this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Open,
    Close,
    /// "Change keys…": Settings on the Keyboard page.
    ChangeKeys,
}

/// Width of the column with what a key does.
const NAME_WIDTH: f32 = 250.0;

impl Sheet {
    /// `groups` from [`groups`]; `close_keys` close it again as Esc does.
    pub fn show(&mut self, ctx: &Context, groups: &[Group], close_keys: &[Chord]) -> Outcome {
        // Not on the first frame: the key that opened the sheet is still in this frame's input.
        if self.focused && pressed(ctx, close_keys) {
            return Outcome::Close;
        }
        let filter = self.filter.to_lowercase();
        let (closed, outcome) =
            dialog::dialog(ctx, "shortcuts", "Keyboard shortcuts", 520.0, |ui| {
                let field = ui
                    .horizontal(|ui| {
                        let muted = theme::palette(ui).muted;
                        ui.label(icons::icon(icons::MAGNIFYING_GLASS).color(muted));
                        let field = TextEdit::singleline(&mut self.filter).hint_text("Filter");
                        let height = ui.spacing().interact_size.y;
                        ui.add_sized([ui.available_width(), height], field)
                    })
                    .inner;
                if !self.focused {
                    field.request_focus();
                    self.focused = true;
                }
                ui.add_space(8.0);
                dialog::fitting_scroll_area(ui, "shortcuts", 220.0, &mut self.height, |ui| {
                    let mut any = false;
                    for (heading, lines) in groups {
                        let shown: Vec<_> =
                            lines.iter().filter(|line| matches(line, &filter)).collect();
                        if shown.is_empty() {
                            continue;
                        }
                        any = true;
                        ui.add_space(6.0);
                        theme::section_label(ui, heading);
                        for (name, keys) in shown {
                            line(ui, name, keys);
                        }
                    }
                    if !any {
                        theme::hint(ui, "No shortcut matches the filter.");
                    }
                });
                ui.add_space(12.0);
                theme::button_row(ui, |ui| {
                    if ui.add(theme::primary_button(ui, "Close")).clicked() {
                        Outcome::Close
                    } else if ui.link("Change keys…").clicked() {
                        Outcome::ChangeKeys
                    } else {
                        Outcome::Open
                    }
                })
            });
        if closed { Outcome::Close } else { outcome }
    }
}

/// The functions with keys in `map`, under the headings Settings uses; `macros` as for
/// [`crate::keymap::groups`].
pub fn groups(map: &KeyMap, macros: Option<&[String]>, sql: bool, mac: bool) -> Vec<Group> {
    crate::keymap::groups(macros, sql)
        .into_iter()
        .map(|(heading, functions)| {
            let lines = functions
                .iter()
                .filter_map(|f: &Function| {
                    let keys = map.keys_for(f, mac);
                    (!keys.is_empty()).then(|| (f.name(), keys))
                })
                .collect();
            (heading, lines)
        })
        .filter(|(_, lines): &Group| !lines.is_empty())
        .collect()
}

/// Whether a line's name or one of its keys holds `filter` (already lower case).
fn matches((name, keys): &(String, Vec<String>), filter: &str) -> bool {
    filter.is_empty()
        || name.to_lowercase().contains(filter)
        || keys.iter().any(|k| k.to_lowercase().contains(filter))
}

/// What a key does, then its keys as key caps.
fn line(ui: &mut egui::Ui, name: &str, keys: &[String]) {
    ui.horizontal(|ui| {
        let size = egui::vec2(NAME_WIDTH, ui.spacing().interact_size.y);
        ui.allocate_ui_with_layout(
            size,
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.add(egui::Label::new(RichText::new(name)).truncate());
            },
        );
        for keys in keys {
            theme::kbd(ui, keys);
        }
    });
}

/// Esc, or one of `close_keys`, was pressed this frame.
fn pressed(ctx: &Context, close_keys: &[Chord]) -> bool {
    ctx.input(|i| {
        i.events.iter().any(|event| match event {
            Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } => *key == egui::Key::Escape || close_keys.contains(&Chord::press(*key, *modifiers)),
            _ => false,
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sheet_lists_the_default_maps_keys_under_settings_headings_r_ui_17() {
        let groups = groups(KeyMap::built_in(), None, false, false);
        let headings: Vec<_> = groups.iter().map(|(h, _)| *h).collect();
        assert_eq!(
            headings,
            ["Send to the host", "Fields and cursor", "Parley 5250"]
        );
        let parley = &groups[2].1;
        let sheet = parley.iter().find(|(name, _)| name == "Keyboard shortcuts");
        assert_eq!(
            sheet.map(|(_, keys)| keys.as_slice()),
            Some(&["Ctrl+/".to_owned()][..])
        );
    }

    #[test]
    fn the_filter_matches_names_and_keys_ignoring_case_r_ui_17() {
        let line = ("Field Exit".to_owned(), vec!["Shift+Enter".to_owned()]);
        assert!(matches(&line, ""));
        assert!(matches(&line, "exit"));
        assert!(matches(&line, "shift"));
        assert!(!matches(&line, "æøå"));
    }
}
