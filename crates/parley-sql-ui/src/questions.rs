//! The window's questions: a statement that changes much (R-SQL-9), and closing with changes
//! not committed (R-SQL-5).

use egui::RichText;
use parley_sql_core::protocol::Request;
use parley_ui_egui::{dialog, theme};

use super::{Choice, Closing, SqlWindow, Waiting};

impl SqlWindow {
    /// R-SQL-9: names the statement and what it does; Run goes on, Cancel stops the queue.
    pub(crate) fn confirm_dialog(&mut self, ctx: &egui::Context) {
        let Some((statement, risk)) = self.confirm.clone() else {
            return;
        };
        let (close, (cancel, run)) = dialog::dialog(
            ctx,
            "sql-confirm",
            "Run this statement?",
            460.0,
            |ui| {
                // A long statement scrolls: the buttons stay in the window.
                dialog::fitting_scroll_area(
                    ui,
                    "sql-confirm-statement",
                    260.0,
                    &mut self.confirm_height,
                    |ui| ui.label(RichText::new(&statement).monospace()),
                );
                ui.add_space(6.0);
                theme::warning(ui, format!("It {}.", risk.describe()));
                theme::hint(
                    ui,
                    "Parley only reads the statement's first words: this guards against slips, it does not check what the statement does.",
                );
                dialog::actions(ui, theme::danger_button(ui, "Run"), true)
            },
        );
        if run {
            self.confirm = None;
            self.start(statement);
        } else if close || cancel {
            self.confirm = None;
            self.queue.clear();
        }
    }

    /// R-SQL-5: closing with changes not committed.
    pub(crate) fn closing_dialog(&mut self, ctx: &egui::Context) {
        if self.closing != Closing::Asking {
            return;
        }
        let (close, choice) = dialog::dialog(
            ctx,
            "sql-closing",
            "Changes are not committed",
            420.0,
            |ui| {
                ui.label("Commit them, or roll them back, before the SQL window closes?");
                ui.add_space(12.0);
                theme::button_row(ui, |ui| {
                    if ui.add(theme::primary_button(ui, "Commit")).clicked() {
                        Some(Choice::Commit)
                    } else if ui.button("Roll back").clicked() {
                        Some(Choice::RollBack)
                    } else if ui.button("Cancel").clicked() {
                        Some(Choice::Cancel)
                    } else {
                        None
                    }
                })
            },
        );
        let sent = match choice {
            Some(Choice::Commit) => self.send(Request::Commit, Waiting::Commit),
            Some(Choice::RollBack) => self.send(Request::Rollback, Waiting::Rollback),
            Some(Choice::Cancel) => false,
            None if close => false,
            None => return,
        };
        // Not sent (cancelled, or a statement still runs): the window stays open.
        self.closing = if sent {
            Closing::AfterCommit
        } else {
            Closing::Open
        };
    }
}
