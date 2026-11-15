//! The window's top bar: who is signed on where, Close, Upload CSV…, and Autocommit with Commit
//! and Rollback (R-SQL-5).

use egui::{RichText, Ui};
use parley_sql_core::protocol::Request;
use parley_ui_egui::{icons, theme};

use super::{Place, SqlWindow, Stage, Waiting};

impl SqlWindow {
    pub(crate) fn top_bar(&mut self, ui: &mut Ui) {
        let p = theme::palette(ui);
        ui.horizontal(|ui| {
            ui.label(icons::icon(icons::DATABASE).color(p.muted));
            match &self.connected {
                Some(c) => {
                    ui.label(theme::semibold(&c.user));
                    ui.label(
                        RichText::new(format!(
                            "{} · {} · job {}",
                            self.config.host, c.release, c.job
                        ))
                        .color(p.muted),
                    );
                    self.schema_menu(ui);
                    if let Some((text, problem)) = self.database.status() {
                        if problem {
                            ui.label(icons::icon(icons::INFO).color(p.warning_text))
                                .on_hover_text(text);
                        } else {
                            ui.label(icons::icon(icons::DATABASE).color(p.muted).small())
                                .on_hover_text(text);
                        }
                    }
                }
                None => {
                    ui.label(RichText::new(&self.config.host).color(p.muted));
                }
            }
            theme::button_row(ui, |ui| {
                let close = match self.place {
                    Place::OwnWindow => "Close",
                    Place::Embedded => "Disconnect",
                };
                if ui.button(close).clicked() {
                    self.request_close();
                }
                if self.stage == Stage::Ready {
                    self.commit_controls(ui);
                    // R-SQL-73: a CSV file into a table, asked for in the dialog.
                    let upload = icons::button(icons::UPLOAD_SIMPLE, "Upload CSV…");
                    let idle = self.waiting.is_none() && self.queue.is_empty();
                    if icons::add_named(ui, idle, upload, "Upload CSV…")
                        .on_hover_text("Rows from a CSV file into a table, all or nothing")
                        .clicked()
                    {
                        self.upload.ask("", "");
                    }
                }
            });
        });
        if !self.config.tls {
            theme::warning(
                ui,
                "Not encrypted: this host's connection has no TLS, so statements and results travel as plain text.",
            );
        }
    }

    /// Autocommit, and Commit and Rollback while it is off (R-SQL-5).
    pub(crate) fn commit_controls(&mut self, ui: &mut Ui) {
        let idle = self.waiting.is_none();
        if !self.autocommit {
            let commit = ui.add_enabled(idle && self.uncommitted, egui::Button::new("Commit"));
            if commit.clicked() {
                self.send(Request::Commit, Waiting::Commit);
            }
            if ui
                .add_enabled(idle && self.uncommitted, egui::Button::new("Rollback"))
                .clicked()
            {
                self.send(Request::Rollback, Waiting::Rollback);
            }
        }
        let mut on = self.autocommit;
        let switch = ui
            .add_enabled_ui(idle && !self.uncommitted, |ui| theme::switch(ui, &mut on))
            .inner;
        let label = ui.label("Autocommit");
        switch
            .labelled_by(label.id)
            .on_hover_text("Off: changes wait for Commit or Rollback");
        if on != self.autocommit {
            self.send(Request::Autocommit { on }, Waiting::Autocommit(on));
        }
    }
}
