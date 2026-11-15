//! Library groups (R-SQL-83, D33): the operator's named groups of libraries, such as
//! Development and Testing, kept per saved connection. A group only organises what the
//! browser shows; it never changes the job's library list or current schema. A library may be
//! in several groups.

use egui::{TextEdit, Ui};
use parley_core::sql::text;
use serde::{Deserialize, Serialize};

use super::Browser;
use crate::{dialog, theme};

/// A named group of libraries, by system name, in the order they were added.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct LibraryGroup {
    pub name: String,
    pub libraries: Vec<String>,
    /// Whether the group is shown open in the browser.
    pub open: bool,
}

impl Default for LibraryGroup {
    fn default() -> Self {
        Self {
            name: String::new(),
            libraries: Vec::new(),
            open: true,
        }
    }
}

/// The group the libraries pinned before groups become, the first time (R-SQL-83).
pub const PINNED: &str = "Pinned";

/// The groups for a connection that has none yet: the libraries it had pinned (R-SQL-34), as
/// one group, or none.
pub fn from_pinned(pinned: Vec<String>) -> Vec<LibraryGroup> {
    if pinned.is_empty() {
        return Vec::new();
    }
    vec![LibraryGroup {
        name: PINNED.to_owned(),
        libraries: pinned,
        open: true,
    }]
}

/// `typed` as a group's name: given, and not another group's (ignoring case). `except` is the
/// group being renamed, which may keep its own name.
fn group_name(
    groups: &[LibraryGroup],
    typed: &str,
    except: Option<usize>,
) -> Result<String, String> {
    let name = typed.trim();
    if name.is_empty() {
        return Err("Give the group a name.".to_owned());
    }
    let taken = groups
        .iter()
        .enumerate()
        .any(|(i, g)| Some(i) != except && g.name.to_lowercase() == name.to_lowercase());
    if taken {
        return Err(format!("There is a group called \"{name}\" already."));
    }
    Ok(name.to_owned())
}

/// What a name dialog is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Naming {
    NewGroup,
    Rename(usize),
    /// A library typed into this group.
    AddLibrary(usize),
}

/// A name dialog open: what for, what is typed, and why it was not taken.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Asking {
    pub naming: Naming,
    pub text: String,
    pub error: Option<String>,
}

/// Which way a group moves in the list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Direction {
    Up,
    Down,
}

impl Browser {
    /// Starts with the groups kept for this connection (R-SQL-83).
    pub fn with_groups(groups: Vec<LibraryGroup>, list_open: bool) -> Self {
        Self {
            groups,
            list_open,
            ..Self::default()
        }
    }

    /// The groups, to keep in the settings.
    pub fn library_groups(&self) -> &[LibraryGroup] {
        &self.groups
    }

    /// Whether the "Library list" group is open, to keep in the settings.
    pub const fn list_open(&self) -> bool {
        self.list_open
    }

    /// Asks for a new group's name.
    pub fn new_group(&mut self) {
        self.ask(Naming::NewGroup, String::new());
    }

    pub(super) fn ask(&mut self, naming: Naming, text: String) {
        self.asking = Some(Asking {
            naming,
            text,
            error: None,
        });
    }

    /// Takes what was typed in the name dialog; `Err` says why not.
    pub(super) fn answer(&mut self, naming: Naming, typed: &str) -> Result<(), String> {
        match naming {
            Naming::NewGroup => {
                let name = group_name(&self.groups, typed, None)?;
                self.groups.push(LibraryGroup {
                    name,
                    ..LibraryGroup::default()
                });
            }
            Naming::Rename(i) => {
                let name = group_name(&self.groups, typed, Some(i))?;
                if let Some(group) = self.groups.get_mut(i) {
                    group.name = name;
                }
            }
            Naming::AddLibrary(i) => {
                let library = text::system_name(typed)
                    .ok_or_else(|| format!("\"{}\" is not a library name.", typed.trim()))?;
                self.add_to(i, &library);
            }
        }
        Ok(())
    }

    /// Puts `library` in group `i`, once, shown open at once: added closed, it seemed to show
    /// nothing.
    pub(super) fn add_to(&mut self, i: usize, library: &str) {
        let Some(group) = self.groups.get_mut(i) else {
            return;
        };
        if !group.libraries.iter().any(|l| l == library) {
            group.libraries.push(library.to_owned());
        }
        group.open = true;
        self.just_added = Some((i, library.to_owned()));
    }

    pub(super) fn remove_from(&mut self, i: usize, library: &str) {
        if let Some(group) = self.groups.get_mut(i) {
            group.libraries.retain(|l| l != library);
        }
    }

    pub(super) fn move_group(&mut self, i: usize, direction: Direction) {
        let to = match direction {
            Direction::Up => i.checked_sub(1),
            Direction::Down => Some(i + 1),
        };
        if let Some(to) = to
            && to < self.groups.len()
            && i < self.groups.len()
        {
            self.groups.swap(i, to);
        }
    }

    /// The groups `library` is not in, by index and name, for "Add to group".
    pub(super) fn groups_without(&self, library: &str) -> Vec<(usize, String)> {
        self.groups
            .iter()
            .enumerate()
            .filter(|(_, g)| !g.libraries.iter().any(|l| l == library))
            .map(|(i, g)| (i, g.name.clone()))
            .collect()
    }

    /// A group's menu: from its menu button or a right-click on its name.
    pub(super) fn group_menu(&mut self, ui: &mut Ui, i: usize) {
        let last = self.groups.len().saturating_sub(1);
        if ui.button("Add library…").clicked() {
            self.ask(Naming::AddLibrary(i), String::new());
            ui.close();
        }
        if ui.button("Rename…").clicked() {
            let name = self
                .groups
                .get(i)
                .map(|g| g.name.clone())
                .unwrap_or_default();
            self.ask(Naming::Rename(i), name);
            ui.close();
        }
        ui.separator();
        if ui
            .add_enabled(i > 0, egui::Button::new("Move up"))
            .clicked()
        {
            self.move_group(i, Direction::Up);
            ui.close();
        }
        if ui
            .add_enabled(i < last, egui::Button::new("Move down"))
            .clicked()
        {
            self.move_group(i, Direction::Down);
            ui.close();
        }
        ui.separator();
        if ui.button("Delete…").clicked() {
            self.deleting = Some(i);
            ui.close();
        }
    }

    /// The name dialog and the question before deleting a group.
    pub fn dialogs(&mut self, ctx: &egui::Context) {
        self.name_dialog(ctx);
        self.delete_dialog(ctx);
    }

    fn name_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut asking) = self.asking.take() else {
            return;
        };
        let group = |i: usize| {
            self.groups
                .get(i)
                .map(|g| g.name.clone())
                .unwrap_or_default()
        };
        let (title, label, confirm, hint) = match asking.naming {
            Naming::NewGroup => (
                "New group".to_owned(),
                "Name",
                "Make",
                "For example Development or Testing.",
            ),
            Naming::Rename(i) => (format!("Rename {}", group(i)), "Name", "Rename", ""),
            Naming::AddLibrary(i) => (
                format!("Add a library to {}", group(i)),
                "Library",
                "Add",
                "Its system name, such as QGPL.",
            ),
        };
        let (close, (field, (cancel, go))) =
            dialog::dialog(ctx, "sql-library-group", &title, 400.0, |ui| {
                let field = ui
                    .horizontal(|ui| {
                        let name = ui.label(label);
                        theme::sized_edit(ui, 260.0, TextEdit::singleline(&mut asking.text))
                            .labelled_by(name.id)
                    })
                    .inner;
                if !hint.is_empty() {
                    theme::hint(ui, hint);
                }
                if let Some(error) = &asking.error {
                    theme::error(ui, error);
                }
                let ready = !asking.text.trim().is_empty();
                (
                    field,
                    dialog::actions(ui, theme::primary_button(ui, confirm), ready),
                )
            });
        if !field.has_focus() && !field.lost_focus() && asking.text.is_empty() {
            field.request_focus();
        }
        let entered = field.lost_focus() && ctx.input(|i| i.key_pressed(egui::Key::Enter));
        if close || cancel {
            return;
        }
        if go || entered {
            match self.answer(asking.naming, &asking.text) {
                Ok(()) => return,
                Err(why) => asking.error = Some(why),
            }
        }
        self.asking = Some(asking);
    }

    fn delete_dialog(&mut self, ctx: &egui::Context) {
        let Some(i) = self.deleting else {
            return;
        };
        let Some(name) = self.groups.get(i).map(|g| g.name.clone()) else {
            self.deleting = None;
            return;
        };
        let (close, (cancel, delete)) = dialog::dialog(
            ctx,
            "sql-library-group-delete",
            "Delete the group?",
            400.0,
            |ui| {
                ui.label(format!(
                    "The group {name} is removed from the list. Its libraries stay on the host."
                ));
                dialog::actions(ui, theme::danger_button(ui, "Delete"), true)
            },
        );
        if close || cancel || delete {
            self.deleting = None;
        }
        if delete {
            self.groups.remove(i);
            self.just_added = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn named(names: &[&str]) -> Browser {
        let groups = names
            .iter()
            .map(|n| LibraryGroup {
                name: (*n).to_owned(),
                ..LibraryGroup::default()
            })
            .collect();
        Browser::with_groups(groups, true)
    }

    /// R-SQL-83: the libraries pinned before become one group, "Pinned"; none, no group.
    #[test]
    fn pinned_libraries_become_a_group_called_pinned_r_sql_83() {
        assert_eq!(from_pinned(Vec::new()), []);
        let groups = from_pinned(vec!["QGPL".to_owned(), "KUNDELIB".to_owned()]);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].name, PINNED);
        assert_eq!(groups[0].libraries, ["QGPL", "KUNDELIB"]);
    }

    /// R-SQL-83: a group's name is given and unique within the connection, ignoring case; a
    /// renamed group may keep its own name.
    #[test]
    fn a_group_needs_a_name_of_its_own_r_sql_83() {
        let mut b = named(&["Development"]);
        assert!(b.answer(Naming::NewGroup, "  ").is_err());
        assert!(b.answer(Naming::NewGroup, "development").is_err());
        assert_eq!(b.answer(Naming::NewGroup, " Test æøå "), Ok(()));
        assert_eq!(b.library_groups()[1].name, "Test æøå");
        assert!(b.answer(Naming::Rename(1), "DEVELOPMENT").is_err());
        assert_eq!(b.answer(Naming::Rename(0), "Development"), Ok(()));
        assert_eq!(b.answer(Naming::Rename(0), "Udvikling"), Ok(()));
        assert_eq!(b.library_groups()[0].name, "Udvikling");
    }

    /// R-SQL-83: only a library name goes in, once per group; the same library may be in
    /// several groups, and is shown open where it was added.
    #[test]
    fn a_library_goes_in_a_group_once_and_may_be_in_several_r_sql_83() {
        let mut b = named(&["Development", "Testing"]);
        assert_eq!(b.answer(Naming::AddLibrary(0), " utils "), Ok(()));
        assert_eq!(b.answer(Naming::AddLibrary(0), "UTILS"), Ok(()));
        assert_eq!(b.answer(Naming::AddLibrary(1), "UTILS"), Ok(()));
        assert!(b.answer(Naming::AddLibrary(1), "not a name").is_err());
        assert!(b.answer(Naming::AddLibrary(1), "ÆØÅ").is_err());
        assert_eq!(b.library_groups()[0].libraries, ["UTILS"]);
        assert_eq!(b.library_groups()[1].libraries, ["UTILS"]);
        assert_eq!(b.just_added, Some((1, "UTILS".to_owned())));
        assert_eq!(b.groups_without("UTILS"), []);
        b.remove_from(0, "UTILS");
        assert_eq!(b.groups_without("UTILS"), [(0, "Development".to_owned())]);
        assert_eq!(
            b.library_groups()[1].libraries,
            ["UTILS"],
            "the other group keeps it"
        );
    }

    /// R-SQL-83: groups move up and down, but not past either end.
    #[test]
    fn groups_move_up_and_down_within_the_list_r_sql_83() {
        let mut b = named(&["A", "B", "C"]);
        b.move_group(2, Direction::Up);
        b.move_group(0, Direction::Up);
        b.move_group(2, Direction::Down);
        let names: Vec<&str> = b.library_groups().iter().map(|g| g.name.as_str()).collect();
        assert_eq!(names, ["A", "C", "B"]);
    }

    /// R-SQL-83: groups are kept by name in the settings, and a group kept without `open`
    /// reads as open.
    #[test]
    fn a_group_is_kept_and_read_back() {
        let group = LibraryGroup {
            name: "Testing".to_owned(),
            libraries: vec!["QGPL".to_owned()],
            open: false,
        };
        let json = serde_json::to_string(&group).expect("serializes");
        let back: LibraryGroup = serde_json::from_str(&json).expect("reads");
        assert_eq!(back, group);
        let old: LibraryGroup =
            serde_json::from_str(r#"{"name":"A","libraries":[]}"#).expect("reads");
        assert!(old.open);
    }
}
