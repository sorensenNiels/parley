//! Signing on (R-SQL-6): the form, and starting the helpers. Parley keeps no password (D16).

use egui::{Key, TextEdit, Ui};
use parley_core::password::Password;
use parley_sql_core::protocol::{Connect, Request};
use parley_sql_core::runner::Runner;
use parley_ui_egui::theme;

use super::{SqlWindow, Stage, Waiting};

impl SqlWindow {
    pub(crate) fn sign_in(&mut self, ui: &mut Ui) {
        let connecting = self.stage == Stage::Connecting;
        ui.add_space(24.0);
        ui.vertical_centered(|ui| {
            ui.set_max_width(380.0);
            theme::card(ui).show(ui, |ui| {
                ui.label(theme::semibold(format!("Sign on to {}", self.config.host)));
                theme::hint(
                    ui,
                    "Parley keeps no password: it is asked each time, and used once.",
                );
                ui.add_space(8.0);
                egui::Grid::new("sql-sign-in")
                    .num_columns(2)
                    .spacing([12.0, 8.0])
                    .show(ui, |ui| {
                        let user = ui.label("User");
                        theme::sized_edit(ui, 220.0, TextEdit::singleline(&mut self.user))
                            .labelled_by(user.id);
                        ui.end_row();
                        let password = ui.label("Password");
                        let field = theme::sized_edit(
                            ui,
                            220.0,
                            TextEdit::singleline(&mut self.password.0).password(true),
                        )
                        .labelled_by(password.id);
                        ui.end_row();
                        if field.lost_focus()
                            && ui.input(|i| i.key_pressed(Key::Enter))
                            && !connecting
                        {
                            self.connect(ui.ctx());
                        }
                    });
                if let Some(error) = &self.error_line {
                    theme::error(ui, error);
                }
                ui.add_space(8.0);
                theme::button_row(ui, |ui| {
                    let ready =
                        !connecting && !self.user.trim().is_empty() && !self.password.0.is_empty();
                    let label = if connecting {
                        "Connecting…"
                    } else {
                        "Connect"
                    };
                    if ui
                        .add_enabled(ready, theme::primary_button(ui, label))
                        .clicked()
                    {
                        self.connect(ui.ctx());
                    }
                });
            });
        });
    }

    /// Starts the helper and signs on once; the password field is emptied at once.
    pub(crate) fn connect(&mut self, ctx: &egui::Context) {
        // Taken out of the field first: whatever happens next, it is not kept there.
        let connect = Request::Connect(Connect {
            host: self.config.host.clone(),
            tls: self.config.tls,
            user: self.user.trim().to_owned(),
            password: Password::new(std::mem::take(&mut self.password.0)),
            ca_file: self.config.ca_file.clone(),
        });
        let wake = ctx.clone();
        match Runner::start(&self.java, &self.files, move || wake.request_repaint()) {
            Ok(runner) => self.runner = Some(runner),
            Err(e) => {
                self.error_line = Some(e.to_string());
                return;
            }
        }
        self.error_line = None;
        self.stage = Stage::Connecting;
        self.send(Request::Hello, Waiting::Hello);
        // R-SQL-33: the second helper starts now, and signs on after the first has.
        self.database.start(&self.java, &self.files, ctx);
        self.pending_catalog = Some(connect.clone());
        // Sent once the hello shows the helper speaks this protocol.
        self.pending_connect = Some(connect);
    }
}
