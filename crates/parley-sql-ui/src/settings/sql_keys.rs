//! Settings ▸ Shortcuts in Parley SQL (R-SQL-60): every action of the SQL window with its
//! keys. Click a key and press a new one to change it, 🗙 removes it, + adds one, Reset gives
//! back the action's own keys; a key another action has moves only once the user says so.
//! Laid out as Settings ▸ Keyboard in Parley, whose key button it shares.

use egui::{RichText, TextEdit, Ui};

use parley_ui_egui::keymap::Chord;
use parley_ui_egui::settings::group;
use parley_ui_egui::settings::key_capture::{Pressed, Slot, key_button, take_key};
use parley_ui_egui::theme;

use super::Settings;
use crate::keys::{self, FIXED, SqlAction, SqlKeys};

/// The headings, in order.
const GROUPS: [&str; 5] = ["Run", "Scripts", "Find", "Editing", "Zoom"];

/// What the page says under an action after a key was pressed for it.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Note {
    /// The key cannot be a shortcut, and why.
    Refused(String),
    /// Another action has the key: it moves only if the user confirms.
    Taken {
        chord: Chord,
        other: SqlAction,
        slot: Slot,
    },
}

/// The page's own state.
#[derive(Debug, Default)]
pub(super) struct Page {
    /// The action and slot waiting for a key.
    recording: Option<(SqlAction, Slot)>,
    note: Option<(SqlAction, Note)>,
    /// Shows only the actions whose name or keys contain it.
    filter: String,
}

impl Page {
    /// "Restore defaults": every action gets its own keys.
    pub(super) fn restore(&mut self, settings: &mut Settings) {
        settings.sql_keys.clear();
        self.recording = None;
        self.note = None;
    }

    pub(super) fn show(&mut self, ui: &mut Ui, settings: &mut Settings) {
        self.take_pressed_key(ui, settings);
        let note = (
            "Click a key and press the new one; + adds a key, Reset gives back the action's own.",
            "Keys without Ctrl or Alt are for typing, except F1–F24 and Esc. \
             Parley's SQL add-on keeps the usual keys.",
        );
        group(ui, "Shortcuts", Some(note), |rows| {
            rows.row("Search", None, |ui, name| {
                theme::sized_edit(
                    ui,
                    220.0,
                    TextEdit::singleline(&mut self.filter).hint_text("An action or a key"),
                )
                .labelled_by(name);
            });
        });
        let in_use = SqlKeys::new(&settings.sql_keys);
        for title in GROUPS {
            let actions: Vec<SqlAction> = SqlAction::ALL
                .into_iter()
                .filter(|a| a.group() == title && self.shows(&in_use, *a))
                .collect();
            if actions.is_empty() {
                continue;
            }
            group(ui, title, None, |rows| {
                for action in actions {
                    rows.row(action.name(), None, |ui, _| {
                        self.keys_row(ui, settings, &in_use, action);
                    });
                }
            });
        }
        let fixed: Vec<_> = FIXED
            .iter()
            .filter(|(keys, what)| self.matches(what) || self.matches(keys))
            .collect();
        if !fixed.is_empty() {
            group(ui, "Keys that cannot change", None, |rows| {
                for (keys, what) in fixed {
                    rows.fact(what, |ui| {
                        ui.label(RichText::new(*keys).monospace());
                    });
                }
            });
        }
    }

    /// Whether `text` contains the filter, ignoring case.
    fn matches(&self, text: &str) -> bool {
        let filter = self.filter.trim().to_lowercase();
        filter.is_empty() || text.to_lowercase().contains(&filter)
    }

    fn shows(&self, in_use: &SqlKeys, action: SqlAction) -> bool {
        self.matches(action.name())
            || in_use
                .chords_for(action)
                .iter()
                .any(|c| self.matches(&c.label()))
    }

    /// The keys of `action`: each can be clicked to change and has a 🗙; then + and Reset.
    fn keys_row(
        &mut self,
        ui: &mut Ui,
        settings: &mut Settings,
        in_use: &SqlKeys,
        action: SqlAction,
    ) {
        let chords = in_use.chords_for(action);
        ui.horizontal_wrapped(|ui| {
            for chord in &chords {
                let slot = Slot::Replace(*chord);
                self.key_button(ui, action, slot, &chord.label());
                if ui
                    .add(theme::icon_button(ui, "🗙"))
                    .on_hover_text("Remove this key")
                    .clicked()
                {
                    let left: Vec<Chord> = chords.iter().copied().filter(|c| c != chord).collect();
                    keys::set(&mut settings.sql_keys, action, &left);
                    self.note = None;
                }
            }
            self.key_button(ui, action, Slot::Add, "+");
            if chords != action.default_chords()
                && ui
                    .add(theme::ghost_button("Reset"))
                    .on_hover_text("Give back this action's own keys")
                    .clicked()
            {
                settings.sql_keys.remove(action.id());
                self.note = None;
            }
        });
        if chords.is_empty() {
            theme::hint(ui, "No key: use the button or menu.");
        }
        self.note_row(ui, settings, action);
    }

    fn key_button(&mut self, ui: &mut Ui, action: SqlAction, slot: Slot, text: &str) {
        let waiting = self.recording == Some((action, slot));
        if key_button(ui, waiting, slot, text) {
            self.note = None;
            self.recording = if waiting { None } else { Some((action, slot)) };
        }
    }

    /// Why a key was refused, or the question whether to move a key another action has.
    fn note_row(&mut self, ui: &mut Ui, settings: &mut Settings, action: SqlAction) {
        let Some((_, note)) = self.note.as_ref().filter(|(a, _)| *a == action) else {
            return;
        };
        match note.clone() {
            Note::Refused(text) => theme::error(ui, text),
            Note::Taken { chord, other, slot } => {
                theme::warning(ui, format!("{} is {} now.", chord.label(), other.name()));
                ui.horizontal(|ui| {
                    if ui.button(format!("Use it for {}", action.name())).clicked() {
                        place(settings, action, slot, chord);
                        self.note = None;
                    }
                    if ui.add(theme::ghost_button("Keep it")).clicked() {
                        self.note = None;
                    }
                });
            }
        }
    }

    /// Takes the key pressed for the action waiting for one, before anything else sees it.
    fn take_pressed_key(&mut self, ui: &Ui, settings: &mut Settings) {
        let Some((action, slot)) = self.recording else {
            return;
        };
        let (Some(pressed), _, _) = take_key(ui, Chord::exact) else {
            return;
        };
        self.recording = None;
        let note = match pressed {
            Pressed::Chord(chord) => record(settings, action, slot, chord),
            Pressed::Clipboard => Some(Note::Refused(
                "Ctrl+C, Ctrl+V and Ctrl+X copy, paste and cut. Choose another key.".to_owned(),
            )),
        };
        self.note = note.map(|n| (action, n));
    }
}

/// Gives `action` the key `chord` in `slot` unless it cannot have it; returns what to say: why
/// it was refused, or that another action has it (then nothing changes yet).
fn record(settings: &mut Settings, action: SqlAction, slot: Slot, chord: Chord) -> Option<Note> {
    if let Some(why) = keys::refusal(chord) {
        return Some(Note::Refused(why));
    }
    match SqlKeys::new(&settings.sql_keys).action_for(chord) {
        Some(a) if a == action => None,
        Some(other) => Some(Note::Taken { chord, other, slot }),
        None => {
            place(settings, action, slot, chord);
            None
        }
    }
}

/// Gives `action` the key `chord` in `slot`, taking it from any other action.
fn place(settings: &mut Settings, action: SqlAction, slot: Slot, chord: Chord) {
    let in_use = SqlKeys::new(&settings.sql_keys);
    if let Some(other) = in_use.action_for(chord).filter(|a| *a != action) {
        let left: Vec<Chord> = in_use
            .chords_for(other)
            .into_iter()
            .filter(|c| *c != chord)
            .collect();
        keys::set(&mut settings.sql_keys, other, &left);
    }
    let mut chords = SqlKeys::new(&settings.sql_keys).chords_for(action);
    match slot {
        Slot::Replace(old) => {
            if let Some(at) = chords.iter().position(|c| *c == old) {
                chords[at] = chord;
            }
        }
        Slot::Add => chords.push(chord),
    }
    keys::set(&mut settings.sql_keys, action, &chords);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chord(text: &str) -> Chord {
        Chord::parse_exact(text).expect(text)
    }

    fn keys_of(settings: &Settings, action: SqlAction) -> Vec<Chord> {
        SqlKeys::new(&settings.sql_keys).chords_for(action)
    }

    #[test]
    fn a_free_key_replaces_or_joins_at_once_r_sql_60() {
        let mut settings = Settings::default();
        let find = SqlAction::Find;
        let slot = Slot::Replace(chord("Ctrl+F"));
        assert_eq!(record(&mut settings, find, slot, chord("Ctrl+K")), None);
        assert_eq!(keys_of(&settings, find), [chord("Ctrl+K")]);
        assert_eq!(record(&mut settings, find, Slot::Add, chord("F3")), None);
        assert_eq!(keys_of(&settings, find), [chord("Ctrl+K"), chord("F3")]);
    }

    #[test]
    fn a_key_another_action_has_moves_only_when_confirmed_r_sql_60() {
        let mut settings = Settings::default();
        let taken = record(&mut settings, SqlAction::Find, Slot::Add, chord("Ctrl+S"));
        assert_eq!(
            taken,
            Some(Note::Taken {
                chord: chord("Ctrl+S"),
                other: SqlAction::Save,
                slot: Slot::Add
            })
        );
        assert_eq!(keys_of(&settings, SqlAction::Save), [chord("Ctrl+S")]);
        place(&mut settings, SqlAction::Find, Slot::Add, chord("Ctrl+S"));
        assert_eq!(keys_of(&settings, SqlAction::Save), []);
        assert_eq!(
            keys_of(&settings, SqlAction::Find),
            [chord("Ctrl+F"), chord("Ctrl+S")]
        );
    }

    #[test]
    fn a_typing_key_is_refused_and_nothing_changes_r_sql_60() {
        let mut settings = Settings::default();
        let note = record(&mut settings, SqlAction::Run, Slot::Add, chord("Enter"));
        assert!(matches!(note, Some(Note::Refused(_))));
        assert!(settings.sql_keys.is_empty());
    }

    /// The page shows, changes a key by click and press, and keeps its Save button in place.
    #[test]
    fn the_shortcuts_page_changes_a_key_by_click_and_press_r_sql_60() {
        use egui_kittest::kittest::Queryable;

        let mut harness = egui_kittest::Harness::builder()
            .with_size(egui::vec2(760.0, 1600.0))
            .build_ui_state(
                |ui, (page, settings): &mut (Page, Settings)| {
                    parley_ui_egui::test_support::install_fonts(ui);
                    page.show(ui, settings);
                },
                (Page::default(), Settings::default()),
            );
        harness.run();
        harness.get_by_label("Ctrl+G").click();
        harness.run();
        harness.key_press_modifiers(
            egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
            egui::Key::G,
        );
        harness.run_steps(2);
        let settings = &harness.state().1;
        assert_eq!(
            keys_of(settings, SqlAction::GoToLine),
            [chord("Ctrl+Shift+G")]
        );
        harness.get_by_label("Ctrl+Shift+G");
        harness.get_by_label("Tab, Shift+Tab");
    }
}
