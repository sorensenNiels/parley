//! The questions a session asks: a host key (R-SSH-3), the sign-on's prompts (R-SSH-4), a paste of several
//! lines (R-SSH-9); and the shortcut sheet (R-SSH-7).

use egui::{Context, RichText, Ui};
use parley_ssh_core::Secret;
use parley_ssh_core::known_hosts::{HostKeyStatus, KnownHosts};
use parley_ssh_core::net::{self, HostKeyAnswer, HostKeyQuestion, Target};
use parley_ui_egui::{dialog, theme};

const WIDTH: f32 = 520.0;
/// The most banner lines shown above the prompts; a banner is a notice, not a document.
const BANNER_LINES: usize = 12;

/// The host key dialog: a new key, or one that changed.
#[derive(Debug)]
pub struct HostKey {
    question: HostKeyQuestion,
    /// Why the saved key could not be removed.
    error: Option<String>,
}

impl HostKey {
    pub fn new(question: HostKeyQuestion) -> Self {
        Self {
            question,
            error: None,
        }
    }

    /// Draws the dialog; returns the answer once given.
    pub fn show(&mut self, ctx: &Context, known: &KnownHosts) -> Option<HostKeyAnswer> {
        if let HostKeyStatus::Changed { saved } = &self.question.status {
            let saved = saved.clone();
            return self.changed(ctx, known, &saved);
        }
        let (close, answer) = dialog::dialog(
            ctx,
            "ssh-host-key",
            "A host Parley SSH has not seen",
            WIDTH,
            |ui| {
                ui.label(format!(
                "{} sent a key Parley SSH has not saved. Before you trust it, check that the fingerprint is the \
                 one the server's administrator gives.",
                self.question.host
            ));
                ui.add_space(10.0);
                key_facts(ui, &self.question);
                ui.add_space(16.0);
                theme::button_row(ui, |ui| {
                    if ui
                        .add(theme::primary_button(ui, "Accept and save"))
                        .clicked()
                    {
                        return Some(HostKeyAnswer::AcceptAndSave);
                    }
                    if ui
                        .button("Connect once")
                        .on_hover_text("Trust the key for this connection only.")
                        .clicked()
                    {
                        return Some(HostKeyAnswer::ConnectOnce);
                    }
                    ui.button("Cancel")
                        .clicked()
                        .then_some(HostKeyAnswer::Cancel)
                })
            },
        );
        if close {
            Some(HostKeyAnswer::Cancel)
        } else {
            answer
        }
    }

    /// A key that differs from the saved one: Cancel is the way out; going on takes a separate, explicit step that
    /// removes the saved key from Parley SSH's file and then asks as for a new key (R-SSH-3).
    fn changed(
        &mut self,
        ctx: &Context,
        known: &KnownHosts,
        saved: &[parley_ssh_core::known_hosts::SavedKey],
    ) -> Option<HostKeyAnswer> {
        let mut replace = false;
        let (close, cancel) = dialog::dialog(
            ctx,
            "ssh-host-key-changed",
            "The host key has changed",
            WIDTH,
            |ui| {
                theme::error(
                    ui,
                    format!(
                        "{} sent a different key from the one saved. Someone may be listening in between. Or the \
                     server was set up again: ask its administrator before you go on.",
                        self.question.host
                    ),
                );
                ui.add_space(10.0);
                key_facts(ui, &self.question);
                for key in saved {
                    theme::hint(
                        ui,
                        format!(
                            "Saved: {} ({}, line {})",
                            key.fingerprint,
                            key.file.display(),
                            key.line
                        ),
                    );
                }
                if saved.iter().any(|k| !k.removable(known)) {
                    theme::hint(
                        ui,
                        "OpenSSH's known_hosts is never changed by Parley SSH: a key you accept is saved in Parley SSH's \
                     own file, which then counts for this host.",
                    );
                }
                if let Some(error) = &self.error {
                    theme::error(ui, error);
                }
                ui.add_space(16.0);
                theme::button_row(ui, |ui| {
                    let cancel = ui.add(theme::primary_button(ui, "Cancel")).clicked();
                    replace = ui
                        .add(theme::danger_button(ui, "Remove the saved key…"))
                        .clicked();
                    cancel
                })
            },
        );
        if replace {
            match known.forget(&self.question.host, self.question.port) {
                Ok(_) => self.question.status = HostKeyStatus::Unknown,
                Err(e) => self.error = Some(format!("The saved key could not be removed: {e}")),
            }
        }
        (close || cancel).then_some(HostKeyAnswer::Cancel)
    }
}

fn key_facts(ui: &mut Ui, question: &HostKeyQuestion) {
    egui::Grid::new("ssh-host-key-facts")
        .num_columns(2)
        .spacing([16.0, 6.0])
        .show(ui, |ui| {
            let host = if question.port == 22 {
                question.host.clone()
            } else {
                format!("{} port {}", question.host, question.port)
            };
            ui.label("Host");
            ui.label(host);
            ui.end_row();
            ui.label("Key type");
            ui.label(&question.key_type);
            ui.end_row();
            ui.label("Fingerprint");
            ui.add(
                egui::Label::new(RichText::new(&question.fingerprint).monospace()).selectable(true),
            );
            ui.end_row();
        });
}

/// The sign-on's prompts: the password, or a keyboard-interactive round with its own fields (R-SSH-4).
#[derive(Debug)]
pub struct Prompt {
    request: net::Prompt,
    banner: Option<String>,
    answers: Vec<Secret>,
    focus_first: bool,
}

impl Prompt {
    pub fn new(prompt: net::Prompt, banner: Option<String>) -> Self {
        let answers = vec![Secret::default(); prompt.fields.len()];
        Self {
            request: prompt,
            banner,
            answers,
            focus_first: true,
        }
    }

    /// Draws the dialog; `None` while it is open.
    pub fn show(&mut self, ctx: &Context, target: &Target) -> Option<Answered> {
        let title = if self.request.name.trim().is_empty() {
            format!("Sign on to {}", target.label())
        } else {
            self.request.name.clone()
        };
        let (close, (cancel, confirm)) = dialog::dialog(ctx, "ssh-prompt", &title, WIDTH, |ui| {
            if let Some(banner) = &self.banner {
                ui.label(RichText::new(banner_excerpt(banner)).monospace().small());
                ui.add_space(10.0);
            }
            if !self.request.instructions.trim().is_empty() {
                ui.label(self.request.instructions.trim());
                ui.add_space(6.0);
            }
            let mut enter = false;
            egui::Grid::new("ssh-prompt-fields")
                .num_columns(2)
                .spacing([12.0, 8.0])
                .show(ui, |ui| {
                    for (i, (field, answer)) in self
                        .request
                        .fields
                        .iter()
                        .zip(&mut self.answers)
                        .enumerate()
                    {
                        let name = ui.label(field.label.trim().trim_end_matches(':'));
                        let edit =
                            egui::TextEdit::singleline(answer.text_mut()).password(!field.echo);
                        let response = theme::sized_edit(ui, 280.0, edit).labelled_by(name.id);
                        if i == 0 && std::mem::take(&mut self.focus_first) {
                            response.request_focus();
                        }
                        enter |=
                            response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                        ui.end_row();
                    }
                });
            let (cancel, confirm) = dialog::actions(ui, theme::primary_button(ui, "Sign on"), true);
            (cancel, confirm || enter)
        });
        if close || cancel {
            return Some(Answered::Cancel);
        }
        confirm.then(|| Answered::Answers(std::mem::take(&mut self.answers)))
    }
}

/// How the sign-on dialog was closed.
#[derive(Debug)]
pub enum Answered {
    Cancel,
    Answers(Vec<Secret>),
}

/// The banner's first lines, with a mark when there is more.
fn banner_excerpt(banner: &str) -> String {
    let lines: Vec<&str> = banner.trim_end().lines().collect();
    let mut text = lines
        .iter()
        .take(BANNER_LINES)
        .copied()
        .collect::<Vec<_>>()
        .join("\n");
    if lines.len() > BANNER_LINES {
        text.push_str("\n…");
    }
    text
}

/// Asks before several lines are pasted outside bracketed paste (R-SSH-9): `Some(true)` to paste.
pub fn confirm_paste(ctx: &Context, text: &str) -> Option<bool> {
    let lines = text.trim_end_matches(['\r', '\n']).lines().count();
    let (close, (cancel, paste)) =
        dialog::dialog(ctx, "ssh-paste", "Paste several lines?", 460.0, |ui| {
            ui.label(format!(
                "The text has {lines} lines. The shell may run each as a command as it arrives."
            ));
            ui.add_space(6.0);
            ui.label(RichText::new(banner_excerpt(text)).monospace().small());
            dialog::actions(ui, theme::primary_button(ui, "Paste"), true)
        });
    if close || cancel {
        Some(false)
    } else {
        paste.then_some(true)
    }
}

/// Asks before closing the window while connected (R-SSH-10): `Some(true)` to close.
pub fn confirm_close(ctx: &Context, target: &Target) -> Option<bool> {
    let (close, (cancel, go)) =
        dialog::dialog(ctx, "ssh-close", "Close the session?", 420.0, |ui| {
            ui.label(format!(
                "You are signed on to {}. Closing ends the session.",
                target.label()
            ));
            dialog::actions(ui, theme::danger_button(ui, "End session"), true)
        });
    if close || cancel {
        Some(false)
    } else {
        go.then_some(true)
    }
}

/// Parley SSH's shortcuts (R-SSH-7); true when closed.
pub fn shortcut_sheet(ctx: &Context, mac: bool) -> bool {
    let command = if mac { "Cmd" } else { "Ctrl+Shift" };
    let rows = [
        ("Copy", format!("{command}+C")),
        ("Paste", format!("{command}+V")),
        ("Paste", "Shift+Insert".to_owned()),
        ("Restart session", format!("{command}+R")),
        ("Scroll back a page", "Shift+Page Up".to_owned()),
        ("Scroll forward a page", "Shift+Page Down".to_owned()),
    ];
    let (close, ok) = dialog::dialog(ctx, "ssh-shortcuts", "Keyboard shortcuts", 420.0, |ui| {
        theme::hint(
            ui,
            "Every other key goes to the host as xterm sends it, Ctrl+C and Ctrl+V too.",
        );
        ui.add_space(8.0);
        egui::Grid::new("ssh-shortcut-rows")
            .num_columns(2)
            .spacing([24.0, 6.0])
            .show(ui, |ui| {
                for (what, keys) in &rows {
                    ui.label(*what);
                    theme::kbd(ui, keys);
                    ui.end_row();
                }
            });
        ui.add_space(12.0);
        theme::button_row(ui, |ui| {
            ui.add(theme::primary_button(ui, "Close")).clicked()
        })
    });
    close || ok
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;
    use parley_ssh_core::known_hosts::SavedKey;
    use parley_ssh_core::net::PromptField;

    #[test]
    fn a_long_banner_is_cut_with_a_mark() {
        let banner = (1..=20)
            .map(|n| format!("line {n}"))
            .collect::<Vec<_>>()
            .join("\n");
        let excerpt = banner_excerpt(&banner);
        assert_eq!(excerpt.lines().count(), BANNER_LINES + 1);
        assert!(excerpt.ends_with('…'));
        assert_eq!(banner_excerpt("Welcome æøå\n"), "Welcome æøå");
    }

    fn harness<S: 'static>(
        state: S,
        show: impl FnMut(&mut egui::Ui, &mut S) + 'static,
    ) -> Harness<'static, S> {
        let mut show = show;
        Harness::builder()
            .with_size(egui::vec2(900.0, 640.0))
            .build_ui_state(
                move |ui, state: &mut S| {
                    parley_ui_egui::test_support::install_fonts(ui);
                    show(ui, state);
                },
                state,
            )
    }

    /// The button called `label` stays where it is over more frames (CLAUDE.md "Layout traps").
    fn stays_put<S>(harness: &mut Harness<'static, S>, label: &str) -> egui::Rect {
        harness.run_steps(3);
        let first = harness.get_by_label(label).rect();
        harness.run_steps(5);
        assert_eq!(harness.get_by_label(label).rect(), first, "{label} moved");
        first
    }

    fn question(status: HostKeyStatus) -> HostKeyQuestion {
        HostKeyQuestion {
            host: "box.lan".into(),
            port: 2222,
            key_type: "ssh-ed25519".into(),
            fingerprint: "SHA256:abcæøå".into(),
            status,
        }
    }

    fn known() -> KnownHosts {
        KnownHosts::new(
            std::env::temp_dir()
                .join("parley-ssh-dialog-test-none")
                .join("known_hosts"),
            None,
        )
    }

    #[test]
    fn a_new_host_key_is_accepted_saved_or_once_and_its_buttons_stay_put_r_ssh_3() {
        let state = (HostKey::new(question(HostKeyStatus::Unknown)), None);
        let mut h = harness(
            state,
            |ui, (dialog, answer): &mut (HostKey, Option<HostKeyAnswer>)| {
                if let Some(a) = dialog.show(ui.ctx(), &known()) {
                    *answer = Some(a);
                }
            },
        );
        stays_put(&mut h, "Accept and save");
        stays_put(&mut h, "Connect once");
        h.get_by_label("Connect once").click();
        h.run_steps(2);
        assert_eq!(h.state().1, Some(HostKeyAnswer::ConnectOnce));
    }

    #[test]
    fn a_changed_host_key_offers_cancel_and_a_separate_step_r_ssh_3() {
        let saved = vec![SavedKey {
            file: "/elsewhere/known_hosts".into(),
            line: 3,
            fingerprint: "SHA256:old".into(),
        }];
        let state = (
            HostKey::new(question(HostKeyStatus::Changed { saved })),
            None,
        );
        let mut h = harness(
            state,
            |ui, (dialog, answer): &mut (HostKey, Option<HostKeyAnswer>)| {
                if let Some(a) = dialog.show(ui.ctx(), &known()) {
                    *answer = Some(a);
                }
            },
        );
        stays_put(&mut h, "Cancel");
        assert!(
            h.query_by_label("Accept and save").is_none(),
            "no one-click accept"
        );
        h.get_by_label("Remove the saved key…").click();
        h.run_steps(3);
        assert_eq!(h.state().1, None);
        // The saved key was in another file: the dialog now asks as for a new key.
        stays_put(&mut h, "Accept and save");
    }

    #[test]
    fn the_sign_on_prompt_hides_secrets_and_returns_the_answers_r_ssh_4() {
        let prompt = net::Prompt {
            name: String::new(),
            instructions: "Two things".into(),
            fields: vec![
                PromptField {
                    label: "Password:".into(),
                    echo: false,
                },
                PromptField {
                    label: "Code:".into(),
                    echo: true,
                },
            ],
        };
        let target = Target {
            host: "box".into(),
            port: 22,
            user: "me".into(),
        };
        let state = (
            Prompt::new(prompt, Some("Authorised use only".into())),
            None::<Answered>,
        );
        let mut h = harness(
            state,
            move |ui, (dialog, answer): &mut (Prompt, Option<Answered>)| {
                if let Some(a) = dialog.show(ui.ctx(), &target) {
                    *answer = Some(a);
                }
            },
        );
        stays_put(&mut h, "Sign on");
        h.get_by_label("Password").type_text("æøå-secret");
        h.run_steps(2);
        h.get_by_label("Sign on").click();
        h.run_steps(2);
        let Some(Answered::Answers(answers)) = &h.state().1 else {
            panic!("answered")
        };
        assert_eq!(answers[0].expose(), "æøå-secret");
        assert_eq!(answers[1].expose(), "");
    }

    #[test]
    fn the_paste_question_and_its_buttons_stay_put_r_ssh_9() {
        let mut h = harness(None::<bool>, |ui, answer: &mut Option<bool>| {
            if let Some(a) = confirm_paste(ui.ctx(), "ls\nrm -r x\n") {
                *answer = Some(a);
            }
        });
        stays_put(&mut h, "Paste");
        h.get_by_label("Cancel").click();
        h.run_steps(2);
        assert_eq!(*h.state(), Some(false));
    }

    #[test]
    fn the_close_question_and_its_buttons_stay_put_r_ssh_10() {
        let target = Target {
            host: "box".into(),
            port: 22,
            user: "me".into(),
        };
        let mut h = harness(None::<bool>, move |ui, answer: &mut Option<bool>| {
            if let Some(a) = confirm_close(ui.ctx(), &target) {
                *answer = Some(a);
            }
        });
        stays_put(&mut h, "End session");
        h.get_by_label("End session").click();
        h.run_steps(2);
        assert_eq!(*h.state(), Some(true));
    }

    #[test]
    fn the_shortcut_sheet_stays_put_and_closes_r_ssh_7() {
        let mut h = harness(false, |ui, closed: &mut bool| {
            *closed |= shortcut_sheet(ui.ctx(), false);
        });
        h.run_steps(3);
        let first = h.get_all_by_label("Close").last().expect("button").rect();
        h.run_steps(5);
        assert_eq!(
            h.get_all_by_label("Close").last().expect("button").rect(),
            first
        );
        h.get_all_by_label("Close").last().expect("button").click();
        h.run_steps(2);
        assert!(*h.state());
    }
}
