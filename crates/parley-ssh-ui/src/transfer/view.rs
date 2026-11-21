//! Draws the Transfer panel (R-SSH-56): saved transfers, the two ends, what the source path copies, the switches,
//! then the run: the check, the dry run's list to confirm, progress and the outcome.

use egui::{ComboBox, ProgressBar, ScrollArea, TextEdit, Ui};
use parley_ssh_core::transfer::{self, Changes, Listed};
use parley_ui_egui::theme;

use super::{Outcome, PaneId, Panel, Request, Route, SavedTransfer, Stage, copies_text};

/// A session to choose: its pane, its name, and whether it is connected.
pub type Choice = (PaneId, String, bool);

/// The height of the dry run's list.
const LIST_HEIGHT: f32 = 180.0;

/// Whether the source path copies the folder itself or what is inside it (its trailing `/`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum What {
    Itself,
    Inside,
}

impl Panel {
    /// Draws the panel; `sessions` are the sessions to choose from, `saved` the saved transfers.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        sessions: &[Choice],
        saved: &[SavedTransfer],
    ) -> Option<Request> {
        let mut request = None;
        ui.horizontal(|ui| {
            theme::section_label(ui, "Transfer");
            theme::button_row(ui, |ui| {
                if ui
                    .add(theme::ghost_button("Close"))
                    .on_hover_text("Closes the panel; a copy running is cancelled")
                    .clicked()
                {
                    request = Some(Request::Close);
                }
            });
        });
        ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let idle = !self.running();
                ui.add_enabled_ui(idle, |ui| {
                    if let Some(chosen) = self.form(ui, sessions, saved) {
                        request = Some(chosen);
                    }
                });
                ui.add_space(6.0);
                if let Some(asked) = self.run_part(ui, sessions) {
                    request = Some(asked);
                }
                if idle && let Some(asked) = self.save_part(ui, saved) {
                    request = Some(asked);
                }
            });
        request
    }

    /// What can be changed while nothing runs.
    fn form(
        &mut self,
        ui: &mut Ui,
        sessions: &[Choice],
        saved: &[SavedTransfer],
    ) -> Option<Request> {
        let mut request = None;
        if !saved.is_empty() {
            ComboBox::from_id_salt("transfer-saved")
                .width(ui.available_width())
                .selected_text("Saved transfers")
                .show_ui(ui, |ui| {
                    for (i, s) in saved.iter().enumerate() {
                        if ui.selectable_label(s.name == self.name, &s.name).clicked() {
                            request = Some(Request::Load(i));
                        }
                    }
                });
            ui.add_space(4.0);
        }
        end_row(
            ui,
            "From",
            "transfer-source",
            &mut self.source,
            &mut self.source_path,
            sessions,
        );
        let path = self.source_path.trim();
        if path.len() > 1 {
            let mut what = if path.ends_with('/') {
                What::Inside
            } else {
                What::Itself
            };
            if theme::segmented(
                ui,
                &mut what,
                &[
                    (What::Itself, "The folder itself"),
                    (What::Inside, "What is inside"),
                ],
            ) {
                let bare = path.trim_end_matches('/').to_owned();
                self.source_path = match what {
                    What::Itself => bare,
                    What::Inside => format!("{bare}/"),
                };
            }
        }
        ui.add_space(4.0);
        end_row(
            ui,
            "To",
            "transfer-target",
            &mut self.target,
            &mut self.target_path,
            sessions,
        );
        if !self.source_path.trim().is_empty() {
            theme::hint(ui, copies_text(&self.source_path, &self.target_path));
        }
        ui.add_space(6.0);
        theme::section_label(ui, "Options");
        theme::switch_row(
            ui,
            "Keep permissions, owner and times",
            &mut self.switches.keep,
        );
        theme::switch_row(ui, "Compress on the way", &mut self.switches.compress);
        theme::switch_row(
            ui,
            "Delete extra files at the target",
            &mut self.switches.delete,
        );
        theme::segmented(
            ui,
            &mut self.switches.route,
            &[
                (Route::Straight, "Straight"),
                (Route::Through, "Through Parley SSH"),
            ],
        );
        ui.label("Leave out (one pattern a line)");
        ui.add(
            TextEdit::multiline(&mut self.switches.excludes)
                .hint_text("*.log\ncache/")
                .desired_rows(2)
                .desired_width(f32::INFINITY),
        );
        theme::hint(
            ui,
            if self.switches.route == Route::Through {
                "rsync on the first server reaches the second through this computer, for servers that cannot reach \
                 each other: slower, as every byte passes through here. It signs on to the second with your agent \
                 while the transfer runs; the first server's administrator could use the agent meanwhile. A target \
                 reached through a jump host always goes this way."
            } else {
                "rsync on the first server copies straight to the second, which it signs on to with your agent while \
                 the transfer runs; that server's administrator could use the agent meanwhile. A target reached \
                 through a jump host goes through Parley SSH."
            },
        );
        request
    }

    /// The button that starts, the run as it goes, and how the last one ended.
    fn run_part(&self, ui: &mut Ui, sessions: &[Choice]) -> Option<Request> {
        let mut request = None;
        match self.run.as_ref().map(|run| &run.stage) {
            None => {
                let connected = |pane: Option<PaneId>| {
                    sessions.iter().any(|(p, _, on)| Some(*p) == pane && *on)
                };
                let ready = connected(self.source) && connected(self.target);
                if self.waiting {
                    waiting(ui, "Waiting for both sessions to connect…");
                }
                theme::button_row(ui, |ui| {
                    if self.waiting && ui.button("Cancel").clicked() {
                        request = Some(Request::Cancel);
                    }
                    if ui
                        .add_enabled(ready && !self.waiting, theme::primary_button(ui, "Copy…"))
                        .on_hover_text(
                            "Shows what will be copied first; nothing is copied before you confirm",
                        )
                        .clicked()
                    {
                        request = Some(Request::Copy);
                    }
                });
            }
            Some(Stage::Review { changes, .. }) => {
                review(ui, changes);
                theme::button_row(ui, |ui| {
                    let deleted = changes.deleted.count;
                    let confirm = if deleted > 0 {
                        theme::danger_button(ui, format!("Copy and delete {deleted}"))
                    } else {
                        theme::primary_button(ui, "Copy")
                    };
                    if ui.add(confirm).clicked() {
                        request = Some(Request::Confirm);
                    }
                    if ui.button("Cancel").clicked() {
                        request = Some(Request::Cancel);
                    }
                });
            }
            Some(stage) => {
                show_stage(ui, stage);
                theme::button_row(ui, |ui| {
                    if ui.button("Cancel").clicked() {
                        request = Some(Request::Cancel);
                    }
                });
            }
        }
        match &self.outcome {
            Some(Outcome::Done(text)) => theme::hint(ui, text.clone()),
            Some(Outcome::Cancelled) => {
                theme::hint(ui, "Cancelled. Copy again to go on where it stopped.");
            }
            Some(Outcome::Failed(why)) => theme::error(ui, why.clone()),
            None => {}
        }
        if self.offer_through && !self.running() {
            theme::button_row(ui, |ui| {
                if ui
                    .button("Try through Parley SSH")
                    .on_hover_text("Carries the copy through this computer: slower, but the servers need not reach each other")
                    .clicked()
                {
                    request = Some(Request::TryThrough);
                }
            });
        }
        request
    }

    /// Saving the transfer by name, and forgetting a saved one.
    fn save_part(&mut self, ui: &mut Ui, saved: &[SavedTransfer]) -> Option<Request> {
        let mut request = None;
        ui.add_space(8.0);
        theme::section_label(ui, "Save");
        let known = saved.iter().position(|s| s.name == self.name.trim());
        ui.horizontal(|ui| {
            let buttons = if known.is_some() { 120.0 } else { 56.0 };
            ui.add(
                TextEdit::singleline(&mut self.name)
                    .hint_text("Name")
                    .desired_width(ui.available_width() - buttons),
            );
            let ready =
                !self.name.trim().is_empty() && self.source.is_some() && self.target.is_some();
            if ui
                .add_enabled(ready, theme::ghost_button("Save"))
                .on_hover_text("Keeps this transfer by name, with its sessions' saved names")
                .clicked()
            {
                request = Some(Request::Save);
            }
            if let Some(i) = known
                && ui.add(theme::ghost_button("Forget")).clicked()
            {
                request = Some(Request::Forget(i));
            }
        });
        request
    }
}

fn waiting(ui: &mut Ui, text: &str) {
    ui.horizontal(|ui| {
        ui.spinner();
        ui.label(text);
    });
}

/// A session to choose and its path.
fn end_row(
    ui: &mut Ui,
    label: &str,
    id: &str,
    pane: &mut Option<PaneId>,
    path: &mut String,
    sessions: &[Choice],
) {
    ui.label(theme::semibold(label));
    let shown = match pane.and_then(|p| sessions.iter().find(|(id, _, _)| *id == p)) {
        Some((_, name, true)) => name.clone(),
        Some((_, name, false)) => format!("{name} (connecting…)"),
        None => "Choose a session".to_owned(),
    };
    ComboBox::from_id_salt(id)
        .width(ui.available_width())
        .selected_text(shown)
        .show_ui(ui, |ui| {
            for (p, name, _) in sessions.iter().filter(|(_, _, on)| *on) {
                ui.selectable_value(pane, Some(*p), name);
            }
        });
    ui.add(
        TextEdit::singleline(path)
            .hint_text("/path/on/the/server")
            .desired_width(f32::INFINITY),
    );
}

/// The dry run's list: how many of each, and their names.
fn review(ui: &mut Ui, changes: &Changes) {
    let count = |listed: &Listed, what: &str| format!("{} {what}", listed.count);
    ui.label(theme::semibold(format!(
        "{}, {}, {}",
        count(&changes.added, "to add"),
        count(&changes.changed, "to change"),
        count(&changes.deleted, "to delete"),
    )));
    ScrollArea::vertical()
        .id_salt("transfer-changes")
        .max_height(LIST_HEIGHT)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            for (mark, listed) in [
                ("-", &changes.deleted),
                ("+", &changes.added),
                ("~", &changes.changed),
            ] {
                for name in &listed.names {
                    let mut line = egui::RichText::new(format!("{mark} {name}")).monospace();
                    if mark == "-" {
                        line = line.color(theme::palette(ui).fail);
                    }
                    ui.label(line);
                }
                let more = listed.count - listed.names.len();
                if more > 0 {
                    theme::hint(ui, format!("and {more} more"));
                }
            }
        });
    if changes.deleted.count > 0 {
        theme::warning(
            ui,
            "The lines marked - are deleted at the target, because the source does not have them.",
        );
    }
    theme::hint(ui, "Nothing has been copied yet.");
}

fn show_stage(ui: &mut Ui, stage: &Stage) {
    match stage {
        Stage::Checking { .. } => waiting(ui, "Checking rsync on both servers…"),
        Stage::Relaying { .. } => waiting(ui, "Opening the way through Parley SSH…"),
        Stage::Looking { changes, .. } => {
            let found = changes.added.count + changes.changed.count + changes.deleted.count;
            waiting(ui, &format!("Looking at what to copy… {found} so far"));
        }
        Stage::Copying { progress: None, .. } => waiting(ui, "Starting…"),
        Stage::Copying {
            progress: Some(p), ..
        } => {
            ui.add(ProgressBar::new(f32::from(p.percent) / 100.0).show_percentage());
            theme::hint(
                ui,
                format!(
                    "{} at {}, {} left",
                    transfer::bytes_text(p.bytes),
                    p.speed,
                    p.time
                ),
            );
        }
        // Drawn with its buttons by `run_part`.
        Stage::Review { .. } => {}
    }
}
