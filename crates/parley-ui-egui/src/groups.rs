//! Session groups (R-UI-31, R-UI-32, D53): named lists of saved sessions, started together, and the dialog that
//! edits them.

use egui::{ComboBox, TextEdit};
use serde::{Deserialize, Serialize};

use crate::menu::Action;
use crate::profile::Profile;
use crate::{ParleyUi, connect, dialog, theme};

/// A named list of saved sessions, by name (R-UI-31); a session may be in several groups.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SessionGroup {
    pub name: String,
    /// The saved sessions' names, in the order they start.
    pub sessions: Vec<String>,
    /// Started when Parley starts (R-UI-32); at most one group or session is.
    pub start_on_launch: bool,
}

impl SessionGroup {
    /// Its sessions that are still saved, in its order.
    pub fn profiles<'a>(&self, profiles: &'a [Profile]) -> Vec<&'a Profile> {
        self.sessions
            .iter()
            .filter_map(|name| profiles.iter().find(|p| p.name == *name))
            .collect()
    }
}

/// The saved session `old` is now called `new` (`None`: deleted): every group follows (R-UI-31).
pub fn session_renamed(groups: &mut [SessionGroup], old: &str, new: Option<&str>) {
    for group in groups {
        match new {
            Some(new) => {
                for name in group.sessions.iter_mut().filter(|n| *n == old) {
                    new.clone_into(name);
                }
            }
            None => group.sessions.retain(|n| n != old),
        }
    }
}

/// Why `groups` cannot be saved: a name that is empty or used twice.
fn check(groups: &[SessionGroup]) -> Result<(), String> {
    for (i, group) in groups.iter().enumerate() {
        let name = group.name.trim();
        if name.is_empty() {
            return Err("Every group needs a name.".to_owned());
        }
        if groups[..i]
            .iter()
            .any(|g| g.name.trim().eq_ignore_ascii_case(name))
        {
            return Err(format!("There are two groups called \"{name}\"."));
        }
    }
    Ok(())
}

/// What the groups dialog ended with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Closed {
    /// Saved: the groups as edited.
    Save(Vec<SessionGroup>),
    Cancel,
}

/// File ▸ Session groups… (R-UI-31): makes, renames, orders and deletes groups and chooses their sessions.
#[derive(Debug)]
pub struct Dialog {
    draft: Vec<SessionGroup>,
    selected: usize,
    error: Option<String>,
    scroll_height: f32,
}

impl Dialog {
    /// On `groups`, with group `selected` (or the first) shown.
    pub fn open(groups: &[SessionGroup], selected: Option<usize>) -> Self {
        Self {
            draft: groups.to_vec(),
            selected: selected.unwrap_or(0),
            error: None,
            scroll_height: 0.0,
        }
    }

    /// Draws the dialog; `Some` once it is closed.
    pub fn show(&mut self, ctx: &egui::Context, profiles: &[Profile]) -> Option<Closed> {
        let (close, (cancel, save)) = dialog::dialog(
            ctx,
            "session-groups",
            "Session groups",
            460.0,
            |ui| {
                self.chooser(ui);
                ui.add_space(8.0);
                if let Some(group) = self.draft.get_mut(self.selected) {
                    let was = group.start_on_launch;
                    details(ui, group, profiles, &mut self.scroll_height);
                    if group.start_on_launch && !was {
                        // One group at most starts with Parley.
                        let selected = self.selected;
                        for (i, other) in self.draft.iter_mut().enumerate() {
                            other.start_on_launch = i == selected;
                        }
                    }
                } else {
                    theme::hint(
                        ui,
                        "Make a group, such as Morning or Month end, to start its sessions together.",
                    );
                }
                if let Some(error) = &self.error {
                    theme::error(ui, error);
                }
                dialog::actions(ui, theme::primary_button(ui, "Save"), true)
            },
        );
        if close || cancel {
            return Some(Closed::Cancel);
        }
        if save {
            match check(&self.draft) {
                Ok(()) => {
                    let mut groups = std::mem::take(&mut self.draft);
                    for group in &mut groups {
                        group.name = group.name.trim().to_owned();
                    }
                    return Some(Closed::Save(groups));
                }
                Err(why) => self.error = Some(why),
            }
        }
        None
    }

    /// The group to edit, and New, Delete, Up and Down.
    fn chooser(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let shown = self
                .draft
                .get(self.selected)
                .map_or("", |g| g.name.as_str())
                .to_owned();
            let label = ui.label("Group");
            ComboBox::from_id_salt("session-group-choice")
                .selected_text(shown)
                .width(180.0)
                .show_ui(ui, |ui| {
                    for (i, group) in self.draft.iter().enumerate() {
                        ui.selectable_value(&mut self.selected, i, group.name.as_str());
                    }
                })
                .response
                .labelled_by(label.id);
            if ui.button("New").clicked() {
                self.draft.push(SessionGroup {
                    name: format!("Group {}", self.draft.len() + 1),
                    ..SessionGroup::default()
                });
                self.selected = self.draft.len() - 1;
            }
            let any = self.selected < self.draft.len();
            if ui
                .add_enabled(any, theme::danger_button(ui, "Delete"))
                .clicked()
            {
                self.draft.remove(self.selected);
                self.selected = self.selected.min(self.draft.len().saturating_sub(1));
            }
            let up = self.selected > 0 && any;
            if ui.add_enabled(up, egui::Button::new("Up")).clicked() {
                self.draft.swap(self.selected, self.selected - 1);
                self.selected -= 1;
            }
            let down = self.selected + 1 < self.draft.len();
            if ui.add_enabled(down, egui::Button::new("Down")).clicked() {
                self.draft.swap(self.selected, self.selected + 1);
                self.selected += 1;
            }
        });
    }
}

/// The shown group's name, its sessions as switches in the saved sessions' order, and whether it starts with
/// Parley.
fn details(
    ui: &mut egui::Ui,
    group: &mut SessionGroup,
    profiles: &[Profile],
    scroll_height: &mut f32,
) {
    ui.horizontal(|ui| {
        let label = ui.label("Name");
        theme::sized_edit(ui, 300.0, TextEdit::singleline(&mut group.name)).labelled_by(label.id);
    });
    ui.add_space(8.0);
    theme::section_label(ui, "Sessions");
    if profiles.is_empty() {
        theme::hint(ui, "There are no saved sessions yet.");
    }
    dialog::fitting_scroll_area(ui, "session-group-sessions", 360.0, scroll_height, |ui| {
        for profile in profiles {
            let mut on = group.sessions.contains(&profile.name);
            if theme::switch_row(ui, &profile.name, &mut on).changed() {
                if on {
                    group.sessions.push(profile.name.clone());
                    // In the saved sessions' order, which is the order they start in.
                    group
                        .sessions
                        .sort_by_key(|n| profiles.iter().position(|p| p.name == *n));
                } else {
                    group.sessions.retain(|n| *n != profile.name);
                }
            }
        }
    });
    ui.add_space(8.0);
    theme::switch_row(ui, "Start when Parley starts", &mut group.start_on_launch);
}

/// Session groups in the window (R-UI-31, R-UI-32).
impl ParleyUi {
    /// File ▸ Session groups…, on group `selected` or the first.
    pub(crate) fn open_groups(&mut self, selected: Option<usize>) {
        self.groups_dialog = Some(Dialog::open(&self.settings.session_groups, selected));
    }

    pub(crate) fn show_groups(&mut self, ctx: &egui::Context) {
        let Some(dialog) = &mut self.groups_dialog else {
            return;
        };
        match dialog.show(ctx, &self.profiles) {
            Some(Closed::Save(groups)) => {
                if groups.iter().any(|g| g.start_on_launch) {
                    // One group or session at most starts with Parley.
                    for profile in &mut self.profiles {
                        profile.connect_on_start = false;
                    }
                }
                self.settings.session_groups = groups;
                self.groups_dialog = None;
            }
            Some(Closed::Cancel) => self.groups_dialog = None,
            None => {}
        }
    }

    /// The saved session `old` was saved as `new` (R-UI-31): groups follow a new name; a session marked to
    /// start with Parley unmarks the groups (R-UI-32).
    pub(crate) fn groups_follow_session(&mut self, old: &str, new: &Profile) {
        session_renamed(&mut self.settings.session_groups, old, Some(&new.name));
        if new.connect_on_start {
            for group in &mut self.settings.session_groups {
                group.start_on_launch = false;
            }
        }
    }

    /// Takes deleted sessions out of every group (R-UI-31).
    pub(crate) fn groups_forget_deleted(&mut self) {
        let profiles = &self.profiles;
        for group in &mut self.settings.session_groups {
            group
                .sessions
                .retain(|n| profiles.iter().any(|p| p.name == *n));
        }
    }

    /// The groups on the start screen, under the saved sessions, each with Start and Edit… (R-UI-32).
    pub(crate) fn start_screen_groups(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        if self.settings.session_groups.is_empty() {
            return;
        }
        let entries: Vec<connect::Entry<'_>> = self
            .settings
            .session_groups
            .iter()
            .map(|g| connect::Entry {
                name: &g.name,
                address: g.sessions.join(", "),
            })
            .collect();
        ui.add_space(16.0);
        match connect::group(ui, "Groups", &entries, "Start", "Edit…") {
            Some(connect::Choice::Connect(i)) => actions.push(Action::StartGroup(i)),
            Some(connect::Choice::Edit(i)) => self.open_groups(Some(i)),
            Some(connect::Choice::New) | None => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    use super::*;
    use crate::test_support::profile;

    fn group(name: &str, sessions: &[&str]) -> SessionGroup {
        SessionGroup {
            name: name.to_owned(),
            sessions: sessions.iter().map(|&s| s.to_owned()).collect(),
            start_on_launch: false,
        }
    }

    #[test]
    fn groups_follow_a_renamed_session_and_forget_a_deleted_one_r_ui_31() {
        let mut groups = vec![group("Morgen", &["Prod", "Test"]), group("Æøå", &["Prod"])];
        session_renamed(&mut groups, "Prod", Some("Produktion"));
        assert_eq!(groups[0].sessions, ["Produktion", "Test"]);
        assert_eq!(groups[1].sessions, ["Produktion"]);
        session_renamed(&mut groups, "Test", None);
        assert_eq!(groups[0].sessions, ["Produktion"]);
    }

    #[test]
    fn a_group_starts_only_its_sessions_that_are_still_saved_in_its_order_r_ui_32() {
        let profiles = [profile("A", "a"), profile("B", "b")];
        let g = group("G", &["B", "Gone", "A"]);
        let names: Vec<&str> = g
            .profiles(&profiles)
            .iter()
            .map(|p| p.name.as_str())
            .collect();
        assert_eq!(names, ["B", "A"]);
    }

    #[test]
    fn a_session_saved_to_start_with_parley_unmarks_the_groups_and_renames_in_them_r_ui_32() {
        let mut ui = ParleyUi::new(vec![profile("Prod", "p")]);
        let mut marked = group("Morgen", &["Prod"]);
        marked.start_on_launch = true;
        ui.settings.session_groups = vec![marked];
        let mut saved = profile("Produktion", "p");
        saved.connect_on_start = true;
        ui.groups_follow_session("Prod", &saved);
        assert_eq!(ui.settings.session_groups[0].sessions, ["Produktion"]);
        assert!(!ui.settings.session_groups[0].start_on_launch);
        assert_eq!(ui.startup_group(), None);
    }

    #[test]
    fn every_group_needs_a_name_of_its_own_r_ui_31() {
        assert!(check(&[group("A", &[]), group("B", &[])]).is_ok());
        assert!(check(&[group("  ", &[])]).is_err());
        let twice =
            check(&[group("Morgen", &[]), group(" morgen ", &[])]).expect_err("the same name");
        assert!(twice.contains("morgen"), "{twice}");
    }

    fn dialog(
        groups: &[SessionGroup],
        profiles: Vec<Profile>,
    ) -> Harness<'static, (Dialog, Vec<Profile>, Option<Closed>)> {
        Harness::builder()
            .with_size(egui::vec2(900.0, 700.0))
            .build_ui_state(
                |ui, state: &mut (Dialog, Vec<Profile>, Option<Closed>)| {
                    crate::test_support::install_fonts(ui);
                    if state.2.is_none() {
                        state.2 = state.0.show(ui.ctx(), &state.1);
                    }
                },
                (Dialog::open(groups, None), profiles, None),
            )
    }

    #[test]
    fn the_groups_dialog_keeps_its_buttons_in_place_and_saves_a_new_group_r_ui_31() {
        let profiles = vec![profile("Prod", "p"), profile("Test æøå", "t")];
        let mut harness = dialog(&[], profiles);
        harness.run();
        let before = harness.get_by_label("Save").rect();
        harness.run_steps(5);
        assert_eq!(
            harness.get_by_label("Save").rect(),
            before,
            "the dialog keeps its place"
        );
        harness.get_by_label("New").click();
        harness.run();
        harness.get_by_label("Test æøå").click();
        harness.run();
        harness.get_by_label("Prod").click();
        harness.run();
        harness.get_by_label("Start when Parley starts").click();
        harness.run();
        harness.get_by_label("Save").click();
        harness.run();
        let Some(Closed::Save(groups)) = &harness.state().2 else {
            panic!("not saved: {:?}", harness.state().2);
        };
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].name, "Group 1");
        assert_eq!(
            groups[0].sessions,
            ["Prod", "Test æøå"],
            "in the saved sessions' order"
        );
        assert!(groups[0].start_on_launch);
    }

    #[test]
    fn only_one_group_starts_with_parley_r_ui_32() {
        let mut first = group("A", &[]);
        first.start_on_launch = true;
        let mut harness = dialog(&[first, group("B", &[])], Vec::new());
        harness.run();
        harness.get_by_label("Group").click();
        harness.run();
        harness.get_by_label("B").click();
        harness.run();
        harness.get_by_label("Start when Parley starts").click();
        harness.run();
        harness.get_by_label("Save").click();
        harness.run();
        let Some(Closed::Save(groups)) = &harness.state().2 else {
            panic!("not saved");
        };
        let starting: Vec<&str> = groups
            .iter()
            .filter(|g| g.start_on_launch)
            .map(|g| g.name.as_str())
            .collect();
        assert_eq!(starting, ["B"]);
    }
}
