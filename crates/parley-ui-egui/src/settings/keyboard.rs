//! Settings ▸ Keyboard (R-KEY-2): the keyboard maps. The built-in Default map is shown as it
//! is; the operator copies it to a map of their own and changes that. Each function is a row
//! with its keys: click one and press a new key to change it, 🗙 removes it, + adds one. A key
//! another function has is moved only once the operator says so. Laid out as Tortuga's
//! Settings ▸ Script editor keys.

use egui::{ComboBox, Event, EventFilter, RichText, TextEdit, Ui};

use super::{Settings, group};
use crate::keymap::{Chord, CtrlAlone, DEFAULT_MAP, Function, KeyMap, groups};
use crate::theme;

/// Where a pressed key goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Slot {
    /// In place of this key of the function.
    Replace(Chord),
    /// As one more key.
    Add,
}

/// What the page says under a function after a key was pressed for it.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Note {
    /// The key cannot be bound, and why.
    Refused(String),
    /// The key does `other` now: it moves only if the operator confirms.
    Taken {
        chord: Chord,
        other: Function,
        slot: Slot,
    },
}

/// A name being typed for a copy of the map shown, or for the map shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Naming {
    Copy,
    Rename,
}

/// The page's own state.
#[derive(Debug, Default)]
pub(super) struct Page {
    /// The name of the map shown.
    shown: String,
    /// The macros a key can run; `None` while the Macros add-on is off.
    macros: Option<Vec<String>>,
    /// The SQL add-on is on: its function can have a key (R-SQL-6).
    sql: bool,
    /// The function and slot waiting for a key.
    recording: Option<(Function, Slot)>,
    ctrl_alone: CtrlAlone,
    note: Option<(Function, Note)>,
    naming: Option<(Naming, String)>,
    naming_error: Option<String>,
    /// Maps renamed while the dialog was open, in order: (old name, new name).
    renames: Vec<(String, String)>,
}

impl Page {
    pub(super) fn new(shown: &str, macros: Option<Vec<String>>, sql: bool) -> Self {
        Self {
            shown: shown.to_owned(),
            macros,
            sql,
            ..Self::default()
        }
    }

    pub(super) fn take_renames(&mut self) -> Vec<(String, String)> {
        std::mem::take(&mut self.renames)
    }

    /// "Restore defaults": the map shown gets the Default map's keys.
    pub(super) fn restore(&mut self, settings: &mut Settings) {
        if let Some(map) = self.own_map(settings) {
            map.bindings.clone_from(&KeyMap::built_in().bindings);
        }
        self.recording = None;
        self.note = None;
    }

    fn own_map<'a>(&self, settings: &'a mut Settings) -> Option<&'a mut KeyMap> {
        settings
            .keyboard_maps
            .iter_mut()
            .find(|m| m.name == self.shown)
    }

    pub(super) fn show(&mut self, ui: &mut Ui, settings: &mut Settings) {
        self.take_pressed_key(ui, settings);
        let note = (
            "The Default map cannot be changed: copy it to make your own.",
            "A session uses the map chosen in its session dialog (Default unless chosen). \
             Keys that type characters need Ctrl; Ctrl+C and Ctrl+V always copy and paste.",
        );
        group(ui, "Keyboard map", Some(note), |rows| {
            rows.row("Map", None, |ui, name| self.map_choice(ui, settings, name));
            if self.naming.is_some() {
                rows.full_row(|ui| self.name_field(ui, settings));
            }
        });
        let map = KeyMap::find(&settings.keyboard_maps, &self.shown).clone();
        if map.name != self.shown {
            // The map shown was deleted (or never was): show Default.
            DEFAULT_MAP.clone_into(&mut self.shown);
        }
        let mut changed = map.clone();
        for (title, functions) in self.functions(&map) {
            group(ui, title, None, |rows| {
                for function in functions {
                    rows.row(&function.name(), None, |ui, _| {
                        self.keys_row(ui, &mut changed, &function);
                    });
                }
            });
        }
        if changed != map
            && let Some(own) = self.own_map(settings)
        {
            *own = changed;
        }
    }

    /// The functions under their headings; macros a key of the map runs are listed even
    /// when there is no such macro now, so the key can be removed.
    fn functions(&self, map: &KeyMap) -> Vec<(&'static str, Vec<Function>)> {
        let mut macros = self.macros.clone();
        if let Some(macros) = &mut macros {
            for (_, id) in &map.bindings {
                if let Some(Function::RunMacro(name)) = Function::from_id(id)
                    && !macros.contains(&name)
                {
                    macros.push(name);
                }
            }
        }
        groups(macros.as_deref(), self.sql)
    }

    /// The map shown, and Copy…, Rename… and Delete.
    fn map_choice(&mut self, ui: &mut Ui, settings: &mut Settings, name: egui::Id) {
        ui.horizontal_wrapped(|ui| {
            let before = self.shown.clone();
            ComboBox::from_id_salt("keyboard-map")
                .selected_text(&self.shown)
                .width(160.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.shown, DEFAULT_MAP.to_owned(), DEFAULT_MAP);
                    for map in &settings.keyboard_maps {
                        ui.selectable_value(&mut self.shown, map.name.clone(), &map.name);
                    }
                })
                .response
                .labelled_by(name);
            if self.shown != before {
                self.leave();
            }
            let own = self.shown != DEFAULT_MAP;
            if ui
                .button("Copy…")
                .on_hover_text("A new map with these keys, to change.")
                .clicked()
            {
                self.start_naming(Naming::Copy, format!("{} copy", self.shown));
            }
            if ui.add_enabled(own, egui::Button::new("Rename…")).clicked() {
                self.start_naming(Naming::Rename, self.shown.clone());
            }
            let delete = theme::danger_button(ui, "Delete map");
            if ui
                .add_enabled(own, delete)
                .on_hover_text("Sessions that use this map get Default.")
                .clicked()
            {
                settings.keyboard_maps.retain(|m| m.name != self.shown);
                DEFAULT_MAP.clone_into(&mut self.shown);
                self.leave();
            }
        });
    }

    fn start_naming(&mut self, naming: Naming, text: String) {
        self.leave();
        self.naming = Some((naming, text));
    }

    /// Stops recording and forgets the notes and the name being typed.
    fn leave(&mut self) {
        self.recording = None;
        self.note = None;
        self.naming = None;
        self.naming_error = None;
    }

    /// The name for a copy or a rename, with its confirm button and Cancel.
    fn name_field(&mut self, ui: &mut Ui, settings: &mut Settings) {
        let Some((naming, text)) = &mut self.naming else {
            return;
        };
        let naming = *naming;
        let (confirm, cancel) = ui
            .horizontal(|ui| {
                let label = ui.label("Name");
                let edit =
                    theme::sized_edit(ui, 200.0, TextEdit::singleline(text)).labelled_by(label.id);
                let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                let verb = match naming {
                    Naming::Copy => "Create",
                    Naming::Rename => "Rename",
                };
                let confirm = ui.add(theme::primary_button(ui, verb)).clicked() || enter;
                (confirm, ui.button("Cancel").clicked())
            })
            .inner;
        if let Some(error) = &self.naming_error {
            theme::error(ui, error);
        }
        if cancel {
            self.leave();
        } else if confirm {
            self.finish_naming(settings, naming);
        }
    }

    fn finish_naming(&mut self, settings: &mut Settings, naming: Naming) {
        let typed = self
            .naming
            .as_ref()
            .map_or("", |(_, t)| t.trim())
            .to_owned();
        let keep = (naming == Naming::Rename).then_some(self.shown.as_str());
        if let Err(e) = check_map_name(&typed, &settings.keyboard_maps, keep) {
            self.naming_error = Some(e);
            return;
        }
        match naming {
            Naming::Copy => {
                let mut copy = KeyMap::find(&settings.keyboard_maps, &self.shown).clone();
                copy.name.clone_from(&typed);
                settings.keyboard_maps.push(copy);
            }
            Naming::Rename => {
                if let Some(map) = self.own_map(settings) {
                    map.name.clone_from(&typed);
                }
                self.renames.push((self.shown.clone(), typed.clone()));
            }
        }
        self.shown = typed;
        self.leave();
    }

    /// The keys of `function` in `map`: the system's own key first (fixed), then each key,
    /// which in a map of the operator's can be clicked to change and has a 🗙, and +.
    fn keys_row(&mut self, ui: &mut Ui, map: &mut KeyMap, function: &Function) {
        let own = !map.is_built_in();
        let mac = ui.ctx().os() == egui::os::OperatingSystem::Mac;
        ui.horizontal_wrapped(|ui| {
            if let Some(key) = function.system_key(mac) {
                ui.label(RichText::new(key).monospace())
                    .on_hover_text("The system's key: it always works.");
            }
            for chord in map.chords_for(function) {
                if !own {
                    ui.label(RichText::new(chord.label()).monospace());
                    continue;
                }
                self.key_button(ui, function, Slot::Replace(chord), &chord.label());
                if ui
                    .add(theme::icon_button(ui, "🗙"))
                    .on_hover_text("Remove this key")
                    .clicked()
                {
                    map.unbind(chord);
                    self.note = None;
                }
            }
            if own {
                self.key_button(ui, function, Slot::Add, "+");
            }
        });
        self.note_row(ui, map, function);
    }

    /// A key, or + for one more: click it, then press the key. While waiting it says so and
    /// keeps the keyboard, so Tab and the arrows can be bound too.
    fn key_button(&mut self, ui: &mut Ui, function: &Function, slot: Slot, text: &str) {
        let waiting = self.recording.as_ref() == Some(&(function.clone(), slot));
        if key_button(ui, waiting, slot, text) {
            self.note = None;
            self.recording = if waiting {
                None
            } else {
                Some((function.clone(), slot))
            };
        }
    }

    /// Why a key was refused, or the question whether to move a key another function has.
    fn note_row(&mut self, ui: &mut Ui, map: &mut KeyMap, function: &Function) {
        let Some((_, note)) = self.note.as_ref().filter(|(f, _)| f == function) else {
            return;
        };
        match note.clone() {
            Note::Refused(text) => theme::error(ui, text),
            Note::Taken { chord, other, slot } => {
                theme::warning(ui, format!("{} is {} now.", chord.label(), other.name()));
                ui.horizontal(|ui| {
                    if ui
                        .button(format!("Use it for {}", function.name()))
                        .clicked()
                    {
                        place(map, function, slot, chord);
                        self.note = None;
                    }
                    if ui.add(theme::ghost_button("Keep it")).clicked() {
                        self.note = None;
                    }
                });
            }
        }
    }

    /// Takes the key pressed for the function waiting for one, before anything else sees it.
    fn take_pressed_key(&mut self, ui: &Ui, settings: &mut Settings) {
        let Some((function, slot)) = self.recording.clone() else {
            return;
        };
        let Some(pressed) = self.take_key(ui) else {
            return;
        };
        self.recording = None;
        let note = match pressed {
            Pressed::Chord(chord) => match self.own_map(settings) {
                Some(map) => record(map, &function, slot, chord),
                None => None,
            },
            Pressed::Clipboard => Some(Note::Refused(
                "Ctrl+C, Ctrl+V and Ctrl+X are the system's copy, paste and cut keys. \
                 Choose another key."
                    .to_owned(),
            )),
        };
        self.note = note.map(|n| (function, n));
    }

    /// The key pressed this frame, taken from the input so nothing else acts on it; Ctrl
    /// pressed and let go on its own counts.
    fn take_key(&mut self, ui: &Ui) -> Option<Pressed> {
        let (pressed, taken, modifiers) = take_key(ui, Chord::press);
        let alone = self.ctrl_alone.released(taken, modifiers);
        pressed.or_else(|| alone.then_some(Pressed::Chord(Chord::CtrlAlone)))
    }
}

/// A key, or + for one more (`slot`): click it, then press the key. While `waiting` it says so
/// and keeps the keyboard, so Tab and the arrows can be pressed too. Returns whether it was
/// clicked.
pub(super) fn key_button(ui: &mut Ui, waiting: bool, slot: Slot, text: &str) -> bool {
    let shown = if waiting { "Press a key…" } else { text };
    let button = egui::Button::new(RichText::new(shown).monospace()).selected(waiting);
    let hover = match slot {
        Slot::Replace(_) => "Click, then press the new key",
        Slot::Add => "Add a key: click, then press it",
    };
    let response = ui.add(button).on_hover_text(hover);
    if waiting {
        response.request_focus();
        ui.memory_mut(|m| {
            m.set_focus_lock_filter(
                response.id,
                EventFilter {
                    tab: true,
                    horizontal_arrows: true,
                    vertical_arrows: true,
                    escape: true,
                },
            );
        });
    }
    response.clicked()
}

/// The key pressed this frame as `make` reads it, taken from the input so nothing else acts on
/// it; also whether anything was taken, and the modifiers held.
pub(super) fn take_key(
    ui: &Ui,
    make: fn(egui::Key, egui::Modifiers) -> Chord,
) -> (Option<Pressed>, bool, egui::Modifiers) {
    let (taken, modifiers) = ui.input_mut(|i| {
        let mut taken = Vec::new();
        i.events.retain(|e| match e {
            Event::Key { .. } | Event::Text(_) | Event::Copy | Event::Cut | Event::Paste(_) => {
                taken.push(e.clone());
                false
            }
            _ => true,
        });
        (taken, i.modifiers)
    });
    let pressed = taken.iter().find_map(|e| match e {
        Event::Key {
            key,
            pressed: true,
            modifiers,
            ..
        } => Some(Pressed::Chord(make(*key, *modifiers))),
        Event::Copy | Event::Cut | Event::Paste(_) => Some(Pressed::Clipboard),
        _ => None,
    });
    (pressed, !taken.is_empty(), modifiers)
}

/// A key pressed while a function waits for one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Pressed {
    Chord(Chord),
    /// The system's copy, paste or cut key, which egui never shows as a key.
    Clipboard,
}

/// Gives `function` the key `chord` in `slot` unless it cannot have it; returns what to say:
/// why it was refused, or that another function has it (then nothing changes yet).
fn record(map: &mut KeyMap, function: &Function, slot: Slot, chord: Chord) -> Option<Note> {
    if let Some(why) = chord.refusal() {
        return Some(Note::Refused(why));
    }
    match map.function_for(chord) {
        Some(f) if f == *function => None,
        Some(other) => Some(Note::Taken { chord, other, slot }),
        None => {
            place(map, function, slot, chord);
            None
        }
    }
}

/// Binds `chord` to `function` in `slot`, taking it from any other function.
fn place(map: &mut KeyMap, function: &Function, slot: Slot, chord: Chord) {
    if let Slot::Replace(old) = slot {
        map.unbind(old);
    }
    map.bind(chord, function);
}

/// A map's name: not empty, not Default's, and not another map's. `keep` is the map being
/// renamed, which may keep its name.
fn check_map_name(name: &str, maps: &[KeyMap], keep: Option<&str>) -> Result<(), String> {
    if name.is_empty() {
        return Err("Type a name.".to_owned());
    }
    if name.eq_ignore_ascii_case(DEFAULT_MAP) {
        return Err("Default is the built-in map's name.".to_owned());
    }
    if Some(name) != keep && maps.iter().any(|m| m.name == name) {
        return Err(format!("There is already a map called \"{name}\"."));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use parley_core::{Aid, Key};

    fn chord(text: &str) -> Chord {
        Chord::parse(text).expect(text)
    }

    fn own_map() -> KeyMap {
        KeyMap {
            name: "Mine".to_owned(),
            ..KeyMap::default()
        }
    }

    #[test]
    fn a_free_key_is_bound_at_once_r_key_2() {
        let mut map = own_map();
        let help = Function::Key(Key::Aid(Aid::HELP));
        assert_eq!(record(&mut map, &help, Slot::Add, chord("Ctrl+H")), None);
        assert_eq!(map.chords_for(&help), [chord("Ctrl+H")]);
        assert_eq!(
            record(
                &mut map,
                &help,
                Slot::Replace(chord("Ctrl+H")),
                chord("F13")
            ),
            Some(Note::Taken {
                chord: chord("F13"),
                other: Function::Key(Key::Aid(Aid::function(13).expect("F13"))),
                slot: Slot::Replace(chord("Ctrl+H")),
            })
        );
        assert_eq!(
            map.chords_for(&help),
            [chord("Ctrl+H")],
            "unchanged until confirmed"
        );
    }

    #[test]
    fn a_key_another_function_has_moves_only_when_confirmed_r_key_2() {
        let mut map = own_map();
        let help = Function::Key(Key::Aid(Aid::HELP));
        let note = record(&mut map, &help, Slot::Add, chord("Ctrl+R"));
        let Some(Note::Taken {
            chord: c,
            other,
            slot,
        }) = note
        else {
            panic!("expected a question, got {note:?}");
        };
        assert_eq!(other, Function::Key(Key::Reset));
        place(&mut map, &help, slot, c);
        assert_eq!(map.function_for(chord("Ctrl+R")), Some(help));
        assert_eq!(
            map.chords_for(&Function::Key(Key::Reset)),
            [Chord::CtrlAlone]
        );
    }

    #[test]
    fn a_key_that_types_is_refused_r_key_2() {
        let mut map = own_map();
        let before = map.clone();
        let note = record(&mut map, &Function::Paste, Slot::Add, chord("Shift+A"));
        assert!(matches!(note, Some(Note::Refused(_))), "{note:?}");
        assert_eq!(map, before);
    }

    #[test]
    fn map_names_must_be_new_and_not_default_r_key_2() {
        let maps = [own_map()];
        assert!(check_map_name("", &maps, None).is_err());
        assert!(check_map_name("default", &maps, None).is_err());
        assert!(check_map_name("Mine", &maps, None).is_err());
        assert_eq!(check_map_name("Mine", &maps, Some("Mine")), Ok(()));
        assert_eq!(check_map_name("Ærø", &maps, None), Ok(()));
    }
}
