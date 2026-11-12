//! Keyboard macros in the UI (R-KEY-3, R-KEY-4): where they are kept ([`MacroStore`]), the
//! list the menus show, and the dialogs to name a recording, give a password and manage them.
//! Recording and playing are in `parley_core` ([`parley_core::Session`]).

use egui::{Button, ScrollArea, TextEdit, Ui};
use parley_core::macros::Macro;

use crate::{dialog, theme};

/// Characters a macro name may not hold: it is a file name on every OS.
const FORBIDDEN: &[char] = &['/', '\\', ':', '*', '?', '"', '<', '>', '|'];
const MAX_NAME: usize = 64;

/// One saved macro, or why its file cannot be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredMacro {
    pub name: String,
    pub contents: Result<Macro, String>,
}

impl StoredMacro {
    /// Whether the macro can be used in the saved session called `session`.
    pub fn applies_to(&self, session: Option<&str>) -> bool {
        match &self.contents {
            Ok(m) => m.session.is_none() || m.session.as_deref() == session,
            Err(_) => true,
        }
    }
}

/// Where macros are kept: one text file each in the app (D14), memory in tests. Errors are
/// display text.
pub trait MacroStore: std::fmt::Debug {
    /// Every macro, sorted by name.
    fn list(&self) -> Result<Vec<StoredMacro>, String>;
    /// Saves a macro, replacing one of the same name.
    fn save(&self, name: &str, contents: &Macro) -> Result<(), String>;
    fn delete(&self, name: &str) -> Result<(), String>;
    /// Renames a macro; an existing `new` name is an error.
    fn rename(&self, old: &str, new: &str) -> Result<(), String>;
    /// The folder the files are in, to show and open; `None` if there is none.
    fn folder(&self) -> Option<String>;
}

/// A store that keeps nothing, for a UI built without one.
#[derive(Debug)]
pub struct NoMacroStore;

impl MacroStore for NoMacroStore {
    fn list(&self) -> Result<Vec<StoredMacro>, String> {
        Ok(Vec::new())
    }

    fn save(&self, _: &str, _: &Macro) -> Result<(), String> {
        Err("Parley has no place to keep macros.".to_owned())
    }

    fn delete(&self, _: &str) -> Result<(), String> {
        Ok(())
    }

    fn rename(&self, _: &str, _: &str) -> Result<(), String> {
        Err("Parley has no place to keep macros.".to_owned())
    }

    fn folder(&self) -> Option<String> {
        None
    }
}

/// A trimmed macro name, or why it cannot be one.
pub fn check_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Give the macro a name.".to_owned());
    }
    if name.chars().count() > MAX_NAME {
        return Err(format!("A macro name has at most {MAX_NAME} characters."));
    }
    if name.starts_with('.')
        || name
            .chars()
            .any(|c| FORBIDDEN.contains(&c) || c.is_control())
    {
        return Err("A macro name cannot start with . or hold / \\ : * ? \" < > |".to_owned());
    }
    Ok(name.to_owned())
}

/// The store and the macros in it, read again only when they change.
#[derive(Debug)]
pub struct Macros {
    store: Box<dyn MacroStore>,
    list: Vec<StoredMacro>,
    /// Why the list could not be read.
    problem: Option<String>,
}

impl Macros {
    pub fn new(store: Box<dyn MacroStore>) -> Self {
        let mut macros = Self {
            store,
            list: Vec::new(),
            problem: None,
        };
        macros.reload();
        macros
    }

    /// The macros in `store`, not read yet: the Macros add-on is off (R-ADD-2).
    pub fn unloaded(store: Box<dyn MacroStore>) -> Self {
        Self {
            store,
            list: Vec::new(),
            problem: None,
        }
    }

    /// Forgets the macros read, as when the Macros add-on is turned off (R-ADD-2).
    pub fn unload(&mut self) {
        self.list.clear();
        self.problem = None;
    }

    pub fn reload(&mut self) {
        match self.store.list() {
            Ok(list) => {
                self.list = list;
                self.problem = None;
            }
            Err(e) => {
                self.list.clear();
                self.problem = Some(format!("Could not read the macros: {e}"));
            }
        }
    }

    pub fn list(&self) -> &[StoredMacro] {
        &self.list
    }

    pub fn find(&self, name: &str) -> Option<&StoredMacro> {
        self.list.iter().find(|m| m.name == name)
    }

    /// The names of the macros for the saved session `session`.
    pub fn names_for(&self, session: Option<&str>) -> Vec<String> {
        self.list
            .iter()
            .filter(|m| m.applies_to(session))
            .map(|m| m.name.clone())
            .collect()
    }

    pub fn save(&mut self, name: &str, contents: &Macro) -> Result<(), String> {
        let result = self.store.save(name, contents);
        self.reload();
        result
    }

    /// Moves the macros of a renamed session along with it.
    pub fn rename_session(&mut self, old: &str, new: &str) -> Result<(), String> {
        let moving: Vec<(String, Macro)> = self
            .list
            .iter()
            .filter_map(|m| match &m.contents {
                Ok(contents) if contents.session.as_deref() == Some(old) => {
                    Some((m.name.clone(), contents.clone()))
                }
                _ => None,
            })
            .collect();
        let mut result = Ok(());
        for (name, mut contents) in moving {
            contents.session = Some(new.to_owned());
            if let Err(e) = self.store.save(&name, &contents) {
                result = Err(e);
            }
        }
        self.reload();
        result
    }
}

/// Names a finished recording (R-KEY-3).
#[derive(Debug)]
pub struct SaveDialog {
    recorded: Macro,
    /// The saved session the recording was made in, if any.
    session: Option<String>,
    name: String,
    only_this_session: bool,
    message: Option<String>,
}

impl SaveDialog {
    pub fn new(recorded: Macro, session: Option<String>) -> Self {
        Self {
            recorded,
            session,
            name: String::new(),
            only_this_session: false,
            message: None,
        }
    }

    /// Draws the dialog; true once it is done (saved or cancelled).
    pub fn show(&mut self, ctx: &egui::Context, macros: &mut Macros) -> bool {
        let (close, done) = dialog::dialog(ctx, "save_macro", "Save macro", 440.0, |ui| {
            let steps = self.recorded.steps.len();
            theme::hint(ui, format!("The recording has {steps} steps."));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let label = ui.label("Name");
                theme::sized_edit(
                    ui,
                    300.0,
                    TextEdit::singleline(&mut self.name).hint_text("e.g. Active jobs"),
                )
                .labelled_by(label.id);
            });
            if let Some(session) = &self.session {
                ui.add_space(6.0);
                theme::switch_row(
                    ui,
                    &format!("Only for the session \"{session}\""),
                    &mut self.only_this_session,
                );
            }
            if let Some(message) = &self.message {
                ui.add_space(6.0);
                theme::warning(ui, message);
            }
            let (cancel, save) = dialog::actions(ui, theme::primary_button(ui, "Save"), true);
            cancel || save && self.save(macros)
        });
        close || done
    }

    fn save(&mut self, macros: &mut Macros) -> bool {
        let name = match check_name(&self.name) {
            Ok(name) => name,
            Err(e) => {
                self.message = Some(e);
                return false;
            }
        };
        if macros.find(&name).is_some() {
            self.message = Some(format!("There is already a macro called \"{name}\"."));
            return false;
        }
        let mut contents = self.recorded.clone();
        contents.session = self.session.clone().filter(|_| self.only_this_session);
        match macros.save(&name, &contents) {
            Ok(()) => true,
            Err(e) => {
                self.message = Some(format!("Could not save the macro: {e}"));
                false
            }
        }
    }
}

/// What the password dialog ended with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PasswordAnswer {
    Typed(String),
    Cancelled,
}

/// Asks for the password a macro's `password` step needs when none is saved (R-KEY-4).
#[derive(Debug)]
pub struct PasswordDialog {
    macro_name: String,
    password: String,
}

impl PasswordDialog {
    pub fn new(macro_name: &str) -> Self {
        Self {
            macro_name: macro_name.to_owned(),
            password: String::new(),
        }
    }

    /// Draws the dialog; once closed, what it ended with.
    pub fn show(&mut self, ctx: &egui::Context) -> Option<PasswordAnswer> {
        let (close, answer) =
            dialog::dialog(ctx, "macro_password", "Macro password", 400.0, |ui| {
                theme::hint(
                    ui,
                    format!(
                        "The macro \"{}\" types the password here, and none is saved for \
                     this session.",
                        self.macro_name
                    ),
                );
                ui.add_space(8.0);
                let edit = ui.horizontal(|ui| {
                    let label = ui.label("Password");
                    theme::sized_edit(
                        ui,
                        220.0,
                        TextEdit::singleline(&mut self.password).password(true),
                    )
                    .labelled_by(label.id)
                });
                let entered =
                    edit.inner.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                let (cancel, ok) = dialog::actions(ui, theme::primary_button(ui, "Type it"), true);
                if cancel {
                    Some(PasswordAnswer::Cancelled)
                } else if ok || entered {
                    Some(PasswordAnswer::Typed(std::mem::take(&mut self.password)))
                } else {
                    None
                }
            });
        if close {
            Some(PasswordAnswer::Cancelled)
        } else {
            answer
        }
    }
}

/// Tools ▸ Macros…: the saved macros, to rename or delete, and their folder (R-KEY-4).
#[derive(Debug, Default)]
pub struct Manager {
    /// The macro being renamed, and the new name typed so far.
    renaming: Option<(String, String)>,
    message: Option<String>,
}

/// The list's height: fixed, so the dialog keeps its size (CLAUDE.md "Layout traps").
const LIST_HEIGHT: f32 = 260.0;

impl Manager {
    /// Draws the dialog; true once it is closed.
    pub fn show(&mut self, ctx: &egui::Context, macros: &mut Macros) -> bool {
        let (close, done) = dialog::dialog(ctx, "macros", "Macros", 600.0, |ui| {
            ScrollArea::vertical()
                .id_salt("macro_list")
                .auto_shrink([false, false])
                .max_height(LIST_HEIGHT)
                .min_scrolled_height(LIST_HEIGHT)
                .show(ui, |ui| self.rows(ui, macros));
            if let Some(problem) = &macros.problem {
                theme::error(ui, problem);
            }
            if let Some(message) = &self.message {
                ui.add_space(6.0);
                theme::warning(ui, message);
            }
            ui.add_space(10.0);
            match macros.store.folder() {
                Some(folder) => {
                    theme::hint(ui, format!("Each macro is a text file in {folder}"));
                    ui.horizontal(|ui| {
                        if ui.add(theme::ghost_button("Open folder")).clicked() {
                            ui.ctx()
                                .open_url(egui::OpenUrl::new_tab(format!("file://{folder}")));
                        }
                        if ui.add(theme::ghost_button("Copy path")).clicked() {
                            ui.ctx().copy_text(folder.clone());
                        }
                        if ui.add(theme::ghost_button("Read again")).clicked() {
                            macros.reload();
                        }
                    });
                }
                None => theme::hint(ui, "Parley has no place to keep macros."),
            }
            ui.add_space(16.0);
            theme::button_row(ui, |ui| ui.add(theme::primary_button(ui, "Done")).clicked())
        });
        close || done
    }

    fn rows(&mut self, ui: &mut Ui, macros: &mut Macros) {
        if macros.list().is_empty() {
            theme::hint(
                ui,
                "No macros yet: Record macro in the Tools menu records one.",
            );
            return;
        }
        let mut chosen = None;
        for stored in macros.list() {
            theme::card(ui).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(theme::semibold(&stored.name));
                        match &stored.contents {
                            Ok(m) => theme::hint(ui, scope(m)),
                            Err(e) => theme::error(ui, format!("Cannot be run: {e}")),
                        }
                    });
                    theme::button_row(ui, |ui| {
                        if ui.add(theme::danger_button(ui, "Delete")).clicked() {
                            chosen = Some(Row::Delete(stored.name.clone()));
                        }
                        if ui.add(theme::ghost_button("Rename")).clicked() {
                            chosen = Some(Row::Rename(stored.name.clone()));
                        }
                    });
                });
                if let Some((old, new)) = self.renaming.as_mut().filter(|(o, _)| *o == stored.name)
                {
                    ui.horizontal(|ui| {
                        let label = ui.label("New name");
                        theme::sized_edit(ui, 240.0, TextEdit::singleline(new))
                            .labelled_by(label.id);
                        if ui.add(Button::new("Save name")).clicked() {
                            chosen = Some(Row::Renamed(old.clone(), new.clone()));
                        }
                    });
                }
            });
            ui.add_space(6.0);
        }
        match chosen {
            Some(Row::Rename(name)) => self.renaming = Some((name.clone(), name)),
            Some(Row::Renamed(old, new)) => self.rename(macros, &old, &new),
            Some(Row::Delete(name)) => {
                self.message = Some(match macros.store.delete(&name) {
                    Ok(()) => format!("Deleted \"{name}\"."),
                    Err(e) => format!("Could not delete \"{name}\": {e}"),
                });
                macros.reload();
            }
            None => {}
        }
    }

    fn rename(&mut self, macros: &mut Macros, old: &str, new: &str) {
        let new = match check_name(new) {
            Ok(new) => new,
            Err(e) => return self.message = Some(e),
        };
        if new != old && macros.find(&new).is_some() {
            self.message = Some(format!("There is already a macro called \"{new}\"."));
            return;
        }
        self.message = match macros.store.rename(old, &new) {
            Ok(()) => {
                self.renaming = None;
                None
            }
            Err(e) => Some(format!("Could not rename \"{old}\": {e}")),
        };
        macros.reload();
    }
}

/// What a row of the manager asked for.
enum Row {
    Rename(String),
    Renamed(String, String),
    Delete(String),
}

fn scope(m: &Macro) -> String {
    let steps = m.steps.len();
    match &m.session {
        Some(session) => format!("{steps} steps · only for \"{session}\""),
        None => format!("{steps} steps · every session"),
    }
}

#[cfg(test)]
mod tests {
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;
    use parley_core::macros::Step;

    use super::*;
    use crate::test_support::{MemoryMacros, macros_with};

    fn recording() -> Macro {
        Macro {
            session: None,
            steps: vec![Step::Text("go".into()), Step::Wait],
        }
    }

    /// Shows `dialog` with `macros`; `show` returns true once the dialog is done.
    fn harness_for<D: 'static>(
        dialog: D,
        macros: Macros,
        show: fn(&mut D, &egui::Context, &mut Macros) -> bool,
    ) -> Harness<'static, (D, Macros, bool)> {
        Harness::builder()
            .with_size(egui::vec2(900.0, 700.0))
            .build_ui_state(
                move |ui, state: &mut (D, Macros, bool)| {
                    crate::theme::install(ui.ctx(), crate::fonts::DEFAULT_TERMINAL_FONT);
                    if !state.2 {
                        state.2 = show(&mut state.0, ui.ctx(), &mut state.1);
                    }
                },
                (dialog, macros, false),
            )
    }

    #[test]
    fn the_save_dialog_saves_the_recording_for_this_session_only_r_key_3() {
        let store = MemoryMacros::default();
        let dialog = SaveDialog::new(recording(), Some("PUB400".into()));
        let mut harness = harness_for(
            dialog,
            Macros::new(Box::new(store.clone())),
            SaveDialog::show,
        );
        harness.run();
        let before = harness.get_by_label("Save").rect();
        harness.run_steps(5);
        assert_eq!(
            harness.get_by_label("Save").rect(),
            before,
            "the dialog keeps its place"
        );
        harness.get_by_label("Save").click();
        harness.run();
        assert!(!harness.state().2, "a name is needed");
        harness.get_by_label("Name").focus();
        harness.run();
        harness.get_by_label("Name").type_text("Jobs");
        harness
            .get_by_label("Only for the session \"PUB400\"")
            .click();
        harness.run();
        harness.get_by_label("Save").click();
        harness.run();
        assert!(harness.state().2, "saved and closed");
        let saved = store.0.borrow()["Jobs"].clone();
        assert_eq!(saved.session.as_deref(), Some("PUB400"));
        assert_eq!(saved.steps, recording().steps);
    }

    #[test]
    fn the_save_dialog_refuses_a_name_in_use_r_key_3() {
        let dialog = SaveDialog::new(recording(), None);
        let mut harness = harness_for(
            dialog,
            Macros::new(Box::new(macros_with("Jobs"))),
            SaveDialog::show,
        );
        harness.run();
        harness.get_by_label("Name").focus();
        harness.run();
        harness.get_by_label("Name").type_text("Jobs");
        harness.get_by_label("Save").click();
        harness.run();
        assert!(!harness.state().2);
        harness.get_by_label("There is already a macro called \"Jobs\".");
    }

    #[test]
    fn the_manager_renames_and_deletes_and_keeps_its_place_r_key_4() {
        let store = macros_with("Jobs");
        let macros = Macros::new(Box::new(store.clone()));
        let mut harness = harness_for(Manager::default(), macros, Manager::show);
        harness.run();
        let before = harness.get_by_label("Done").rect();
        harness.run_steps(5);
        assert_eq!(
            harness.get_by_label("Done").rect(),
            before,
            "the dialog keeps its place"
        );
        harness.get_by_label("Rename").click();
        harness.run();
        harness.get_by_label("New name").focus();
        harness.run();
        harness.get_by_label("New name").type_text(" 2");
        harness.get_by_label("Save name").click();
        harness.run();
        assert!(store.0.borrow().contains_key("Jobs 2"));
        harness.get_by_label("Delete").click();
        harness.run();
        assert!(store.0.borrow().is_empty());
        harness.get_by_label("Done").click();
        harness.run();
        assert!(harness.state().2);
    }

    #[test]
    fn the_password_dialog_gives_what_was_typed_and_keeps_its_place_r_key_4() {
        let mut harness = Harness::builder()
            .with_size(egui::vec2(900.0, 700.0))
            .build_ui_state(
                |ui, state: &mut (PasswordDialog, Option<PasswordAnswer>)| {
                    crate::theme::install(ui.ctx(), crate::fonts::DEFAULT_TERMINAL_FONT);
                    if state.1.is_none() {
                        state.1 = state.0.show(ui.ctx());
                    }
                },
                (PasswordDialog::new("Jobs"), None),
            );
        harness.run();
        let before = harness.get_by_label("Type it").rect();
        harness.run_steps(5);
        assert_eq!(harness.get_by_label("Type it").rect(), before);
        harness.get_by_label("Password").focus();
        harness.run();
        harness.get_by_label("Password").type_text("æøå");
        harness.get_by_label("Type it").click();
        harness.run();
        assert_eq!(
            harness.state().1,
            Some(PasswordAnswer::Typed("æøå".to_owned()))
        );
    }

    #[test]
    fn macro_names_must_make_good_file_names_r_key_3() {
        assert_eq!(
            check_name("  Active jobs æøå "),
            Ok("Active jobs æøå".into())
        );
        for bad in [
            "", "  ", ".hidden", "a/b", "a\\b", "a:b", "what?", "a\"b", "a\nb",
        ] {
            assert!(check_name(bad).is_err(), "{bad:?}");
        }
        assert!(check_name(&"x".repeat(MAX_NAME + 1)).is_err());
    }
}
