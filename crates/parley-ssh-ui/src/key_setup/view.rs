//! The Set up key sign-on dialog (R-SSH-67, R-SSH-68): one fixed width, its content per stage.

use egui::{Context, Ui};
use parley_ssh_core::key_setup::{Added, OpenFolder};
use parley_ui_egui::{dialog, theme};

use super::{AgentState, Choice, Fix, KeySetup, Order, Stage};

const WIDTH: f32 = 540.0;

/// Room for the list of keys before it scrolls.
const LIST_ROOM: f32 = 200.0;

/// What a click in the dialog asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Clicked {
    None,
    Close,
    Start,
    Make,
    Back,
    AskFix,
    Fix,
    KeepFolders,
}

impl KeySetup {
    /// Draws the dialog and carries out what was clicked.
    pub fn show(&mut self, ctx: &Context, list_height: &mut f32) -> Vec<Order> {
        let (closed, clicked) =
            dialog::dialog(ctx, "ssh-key-setup", "Set up key sign-on", WIDTH, |ui| {
                self.body(ui, list_height)
            });
        match if closed { Clicked::Close } else { clicked } {
            Clicked::None => Vec::new(),
            Clicked::Close => self.close(),
            Clicked::Start => self.start(),
            Clicked::Make => self.make(),
            Clicked::Back => {
                self.back();
                Vec::new()
            }
            Clicked::AskFix => {
                self.ask_fix();
                Vec::new()
            }
            Clicked::Fix => self.fix(),
            Clicked::KeepFolders => {
                self.keep_folders();
                Vec::new()
            }
        }
    }

    fn body(&mut self, ui: &mut Ui, list_height: &mut f32) -> Clicked {
        match &self.stage {
            Stage::Choose => self.choose_body(ui, list_height),
            Stage::Make { .. } => self.make_body(ui),
            Stage::Checking { .. } | Stage::Adding { .. } => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(format!("Adding the key on {}…", self.server));
                });
                cancel_only(ui)
            }
            Stage::Done { .. } => self.done_body(ui),
            Stage::Failed(why) => {
                theme::error(ui, why.clone());
                close_only(ui)
            }
        }
    }

    fn choose_body(&mut self, ui: &mut Ui, list_height: &mut f32) -> Clicked {
        ui.label(format!(
            "Adds a public key to ~/.ssh/authorized_keys on {}, so the next connection signs on with it.",
            self.server
        ));
        ui.add_space(10.0);
        theme::section_label(ui, "Key");
        let mut chosen = self.chosen;
        dialog::fitting_scroll_area(ui, "ssh-key-setup-keys", LIST_ROOM, list_height, |ui| {
            for (i, choice) in self.choices.iter().enumerate() {
                let (text, usable, why) = describe(choice);
                ui.add_enabled_ui(usable, |ui| ui.radio_value(&mut chosen, i, text));
                if let Some(why) = why {
                    theme::hint(ui, why);
                }
            }
        });
        if chosen != self.chosen {
            self.choose(chosen);
        }
        match &self.agent {
            AgentState::Asking => theme::hint(ui, "Asking the running agent for its keys…"),
            AgentState::Unavailable(why) => theme::hint(ui, format!("Agent: {why}")),
            AgentState::Listed => {}
        }
        let usable = self
            .choices
            .get(self.chosen)
            .is_some_and(|c| matches!(c, Choice::New) || c.line().is_some());
        let label = if matches!(self.choices.get(self.chosen), Some(Choice::New)) {
            "Next"
        } else {
            "Set up"
        };
        match dialog::actions(ui, theme::primary_button(ui, label), usable) {
            (true, _) => Clicked::Close,
            (_, true) => Clicked::Start,
            _ => Clicked::None,
        }
    }

    fn make_body(&mut self, ui: &mut Ui) -> Clicked {
        let Stage::Make {
            passphrase,
            again,
            error,
        } = &mut self.stage
        else {
            return Clicked::None;
        };
        ui.label("A new Ed25519 key in ~/.ssh, readable by you only.");
        ui.add_space(8.0);
        egui::Grid::new("ssh-key-setup-make")
            .num_columns(2)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                let name = ui.label("Comment");
                let edit = egui::TextEdit::singleline(&mut self.comment);
                theme::sized_edit(ui, 300.0, edit).labelled_by(name.id);
                ui.end_row();
                let name = ui.label("Passphrase");
                let edit = egui::TextEdit::singleline(passphrase.text_mut()).password(true);
                theme::sized_edit(ui, 300.0, edit).labelled_by(name.id);
                ui.end_row();
                let name = ui.label("Again");
                let edit = egui::TextEdit::singleline(again.text_mut()).password(true);
                theme::sized_edit(ui, 300.0, edit).labelled_by(name.id);
                ui.end_row();
            });
        if passphrase.expose().is_empty() {
            theme::warning(
                ui,
                "Without a passphrase, anyone who can read the key file can sign on with it.",
            );
        }
        if let Some(error) = error {
            theme::error(ui, error.clone());
        }
        match dialog::actions(ui, theme::primary_button(ui, "Make key"), true) {
            (true, _) => Clicked::Back,
            (_, true) => Clicked::Make,
            _ => Clicked::None,
        }
    }

    fn done_body(&self, ui: &mut Ui) -> Clicked {
        let Stage::Done { added, to_fix, fix } = &self.stage else {
            return Clicked::None;
        };
        ui.label(match added {
            Added::Added => format!("The key was added on {}.", self.server),
            Added::AlreadyThere => format!("The key was on {} already.", self.server),
        });
        match &self.saved {
            Some(name) => theme::hint(
                ui,
                format!("The saved session {name} signs on with it from the next connection."),
            ),
            None => theme::hint(
                ui,
                "This session is not saved: save it with this key to sign on with it next time.",
            ),
        }
        if to_fix.is_empty() {
            return close_only(ui);
        }
        ui.add_space(10.0);
        match fix {
            Fix::Offered => {
                folders(ui, to_fix);
                ui.add_space(16.0);
                theme::button_row(ui, |ui| {
                    if ui.add(theme::primary_button(ui, "Close")).clicked() {
                        Clicked::Close
                    } else if ui.button("Fix…").clicked() {
                        Clicked::AskFix
                    } else {
                        Clicked::None
                    }
                })
            }
            Fix::Asking => {
                ui.label("Take the group's and others' write permission from these folders?");
                for folder in to_fix {
                    ui.label(format!("  {}", folder.path));
                }
                match dialog::actions(ui, theme::primary_button(ui, "Fix"), true) {
                    (true, _) => Clicked::KeepFolders,
                    (_, true) => Clicked::Fix,
                    _ => Clicked::None,
                }
            }
            Fix::Running { .. } => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Changing the folders…");
                });
                cancel_only(ui)
            }
            Fix::Fixed => {
                theme::hint(
                    ui,
                    "The folders were fixed: IBM i's sshd will take the key.",
                );
                close_only(ui)
            }
            Fix::Failed(why) => {
                theme::error(ui, why.clone());
                close_only(ui)
            }
        }
    }
}

/// IBM i's sshd refuses the key while these folders stay as they are (D63).
fn folders(ui: &mut Ui, to_fix: &[OpenFolder]) {
    theme::warning(ui, "IBM i's sshd refuses keys while:");
    for folder in to_fix {
        theme::warning(ui, format!("  {}", folder.why()));
    }
}

/// A choice's text, whether it can be used, and why not.
fn describe(choice: &Choice) -> (String, bool, Option<String>) {
    match choice {
        Choice::File(path, Ok(line)) => (
            format!("Key file {} · {}", path.display(), line.fingerprint),
            true,
            None,
        ),
        Choice::File(path, Err(why)) => (
            format!("Key file {}", path.display()),
            false,
            Some(why.clone()),
        ),
        Choice::Agent(line) => {
            let comment: Vec<&str> = line.line.split_whitespace().skip(2).collect();
            let name = if comment.is_empty() {
                line.fingerprint.clone()
            } else {
                format!("{} · {}", comment.join(" "), line.fingerprint)
            };
            (format!("Agent key {name}"), true, None)
        }
        Choice::New => ("Make a new key (Ed25519)".to_owned(), true, None),
    }
}

fn close_only(ui: &mut Ui) -> Clicked {
    ui.add_space(16.0);
    theme::button_row(ui, |ui| {
        if ui.add(theme::primary_button(ui, "Close")).clicked() {
            Clicked::Close
        } else {
            Clicked::None
        }
    })
}

fn cancel_only(ui: &mut Ui) -> Clicked {
    ui.add_space(16.0);
    theme::button_row(ui, |ui| {
        if ui.button("Cancel").clicked() {
            Clicked::Close
        } else {
            Clicked::None
        }
    })
}
