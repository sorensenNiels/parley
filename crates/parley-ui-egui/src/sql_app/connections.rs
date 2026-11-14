//! Parley SQL's saved connections (R-SQL-46): a name, the host, TLS and its CA file, the code
//! page and the user to fill in at sign-on. Never a password (D16): it is asked at each
//! sign-on and forgotten. The dialog adds, edits and deletes them, and connects to one.

use egui::{Button, ComboBox, Grid, TextEdit, Ui};
use parley_core::{CodePage, SessionConfig};
use serde::{Deserialize, Serialize};

use crate::connect::Entry;
use crate::{dialog, theme};

/// The telnet port and its TLS one: the SQL window does not use them (its helper finds the
/// database server through the port mapper), but Parley's status bar and "Run on…" read a
/// `SessionConfig`, which has one.
const TELNET_PORT: u16 = 23;
const TLS_PORT: u16 = 992;

const HELP: &str = "\
Name: what the connection is called in the File menu and on the start screen.
Host: the IBM i's name or IP address.
TLS: encrypt the connection (the IBM i's host servers need TLS set up). A CA certificate \
names a PEM file to trust besides the system's own.
Code page: how binary columns show as text in the hex view.
User: filled in at sign-on; the password is asked each time and never kept.";

/// One saved connection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SqlConnection {
    pub name: String,
    pub host: String,
    pub tls: bool,
    /// A PEM file of CAs to trust besides the system's own; empty for none.
    pub ca_file: String,
    pub code_page: CodePage,
    /// The user to fill in at sign-on; empty for none. The last one signed on with.
    pub user: String,
}

impl Default for SqlConnection {
    /// A new connection: TLS on, as R-P-3 asks of new sessions.
    fn default() -> Self {
        Self {
            name: String::new(),
            host: String::new(),
            tls: true,
            ca_file: String::new(),
            code_page: CodePage::default(),
            user: String::new(),
        }
    }
}

impl SqlConnection {
    /// The settings the SQL window opens with.
    pub fn config(&self) -> SessionConfig {
        SessionConfig {
            host: self.host.trim().to_owned(),
            port: if self.tls { TLS_PORT } else { TELNET_PORT },
            tls: self.tls,
            ca_file: self.ca_file.trim().to_owned(),
            code_page: self.code_page,
            ..SessionConfig::default()
        }
    }

    /// Its card on the start screen: the host, TLS and the user.
    pub fn entry(&self) -> Entry<'_> {
        let mut address = self.host.clone();
        if self.tls {
            address.push_str("  ·  TLS");
        }
        if !self.user.is_empty() {
            address.push_str("  ·  ");
            address.push_str(&self.user);
        }
        Entry {
            name: &self.name,
            address,
        }
    }
}

/// What the dialog asks Parley SQL to do once it closes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Close,
    /// Connect to the saved connection at this index.
    Connect(usize),
}

/// The open dialog: a copy of one connection being edited.
#[derive(Debug)]
pub struct Editor {
    /// The saved connection being edited; `None` for a new one.
    index: Option<usize>,
    draft: SqlConnection,
    show_help: bool,
    message: Option<String>,
    /// A connection renamed by Save or Connect: (old name, new name).
    renamed: Option<(String, String)>,
}

impl Editor {
    /// Opens on the saved connection at `index`, or on a new one.
    pub fn open(connections: &[SqlConnection], index: Option<usize>) -> Self {
        let connection = index.and_then(|i| connections.get(i));
        Self {
            index: connection.and(index),
            draft: connection.cloned().unwrap_or_default(),
            show_help: false,
            message: None,
            renamed: None,
        }
    }

    /// Draws the dialog; returns what to do once it closes. Connect is off while `connected`.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        connections: &mut Vec<SqlConnection>,
        connected: bool,
    ) -> Option<Outcome> {
        let title = if self.index.is_some() {
            "Edit connection"
        } else {
            "New connection"
        };
        let (close, outcome) = dialog::dialog(ctx, "sql-connection", title, 520.0, |ui| {
            fields(ui, &mut self.draft);
            if self.show_help {
                ui.add_space(10.0);
                theme::hint(ui, HELP);
            }
            if let Some(message) = &self.message {
                ui.add_space(6.0);
                theme::warning(ui, message);
            }
            ui.add_space(16.0);
            self.buttons(ui, connections, connected)
        });
        if close { Some(Outcome::Close) } else { outcome }
    }

    /// A rename saved since the last call: (old name, new name).
    pub fn take_renamed(&mut self) -> Option<(String, String)> {
        self.renamed.take()
    }

    /// Delete and Help on the left; Connect (filled), Save and Cancel on the right.
    fn buttons(
        &mut self,
        ui: &mut Ui,
        connections: &mut Vec<SqlConnection>,
        connected: bool,
    ) -> Option<Outcome> {
        ui.horizontal(|ui| {
            let delete = theme::danger_button(ui, "Delete");
            if ui.add_enabled(self.index.is_some(), delete).clicked() {
                self.delete(connections);
            }
            if ui.add(Button::selectable(self.show_help, "Help")).clicked() {
                self.show_help = !self.show_help;
            }
            let connect = theme::primary_button(ui, "Connect");
            theme::button_row(ui, |ui| {
                let mut outcome = None;
                let connect = ui
                    .add_enabled(!connected, connect)
                    .on_disabled_hover_text("Disconnect first: one connection per window.");
                if connect.clicked()
                    && let Some(index) = self.apply(connections)
                {
                    outcome = Some(Outcome::Connect(index));
                }
                if ui.button("Save").clicked() && self.apply(connections).is_some() {
                    outcome = Some(Outcome::Close);
                }
                if ui.button("Cancel").clicked() {
                    outcome = Some(Outcome::Close);
                }
                outcome
            })
        })
        .inner
    }

    /// Saves the draft; its index, or `None` when the draft is not complete.
    fn apply(&mut self, connections: &mut Vec<SqlConnection>) -> Option<usize> {
        let name = self.draft.name.trim().to_owned();
        let taken = connections
            .iter()
            .enumerate()
            .any(|(i, c)| c.name == name && Some(i) != self.index);
        self.message = if name.is_empty() {
            Some("Give the connection a name.".to_owned())
        } else if taken {
            Some(format!("There is already a connection called \"{name}\"."))
        } else if self.draft.host.trim().is_empty() {
            Some("Enter the IBM i's host name or address.".to_owned())
        } else {
            None
        };
        if self.message.is_some() {
            return None;
        }
        self.draft.name = name;
        self.draft.host = self.draft.host.trim().to_owned();
        self.draft.user = self.draft.user.trim().to_owned();
        let old_name = self
            .index
            .and_then(|i| connections.get(i))
            .map(|c| c.name.clone());
        let saved = self
            .index
            .and_then(|i| connections.get_mut(i).map(|c| (i, c)));
        let index = if let Some((i, saved)) = saved {
            saved.clone_from(&self.draft);
            i
        } else {
            connections.push(self.draft.clone());
            connections.len() - 1
        };
        self.index = Some(index);
        if let Some(old) = old_name.filter(|old| *old != self.draft.name) {
            self.renamed = Some((old, self.draft.name.clone()));
        }
        Some(index)
    }

    fn delete(&mut self, connections: &mut Vec<SqlConnection>) {
        let Some(index) = self.index.filter(|&i| i < connections.len()) else {
            return;
        };
        let removed = connections.remove(index);
        self.index = None;
        self.draft = SqlConnection::default();
        self.message = Some(format!("Deleted \"{}\".", removed.name));
    }
}

/// Name, host and TLS, the CA certificate, code page and user.
fn fields(ui: &mut Ui, draft: &mut SqlConnection) {
    Grid::new("sql-connection-fields")
        .num_columns(2)
        .spacing([12.0, 8.0])
        .show(ui, |ui| {
            let label = ui.label("Name");
            theme::sized_edit(
                ui,
                300.0,
                TextEdit::singleline(&mut draft.name).hint_text("Production"),
            )
            .labelled_by(label.id);
            ui.end_row();

            let label = ui.label("Host");
            theme::sized_edit(
                ui,
                300.0,
                TextEdit::singleline(&mut draft.host).hint_text("ibmi.example.com"),
            )
            .labelled_by(label.id);
            ui.end_row();

            ui.label("");
            theme::switch_row(ui, "TLS", &mut draft.tls);
            ui.end_row();

            if draft.tls {
                let label = ui.label("CA certificate");
                theme::sized_edit(
                    ui,
                    300.0,
                    TextEdit::singleline(&mut draft.ca_file).hint_text("PEM file (optional)"),
                )
                .labelled_by(label.id);
                ui.end_row();
            }

            let label = ui.label("Code page");
            ComboBox::from_id_salt("sql-connection-code-page")
                .selected_text(draft.code_page.label())
                .width(300.0)
                .show_ui(ui, |ui| {
                    for page in CodePage::ALL {
                        ui.selectable_value(&mut draft.code_page, page, page.label());
                    }
                })
                .response
                .labelled_by(label.id);
            ui.end_row();

            let label = ui.label("User");
            theme::sized_edit(
                ui,
                200.0,
                TextEdit::singleline(&mut draft.user)
                    .char_limit(10)
                    .hint_text("optional"),
            )
            .labelled_by(label.id);
            ui.end_row();
        });
    if !draft.tls {
        theme::warning(
            ui,
            "Without TLS, statements, results and the password travel as plain text.",
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn named(name: &str) -> SqlConnection {
        SqlConnection {
            name: name.to_owned(),
            host: format!("{}.example.com", name.to_lowercase()),
            ..SqlConnection::default()
        }
    }

    #[test]
    fn a_connection_needs_a_name_of_its_own_and_a_host_r_sql_46() {
        let mut connections = vec![named("Prod")];
        let mut editor = Editor::open(&connections, None);
        assert_eq!(editor.apply(&mut connections), None, "no name");
        editor.draft.name = "Prod".to_owned();
        assert_eq!(editor.apply(&mut connections), None, "taken");
        editor.draft.name = " Test ".to_owned();
        assert_eq!(editor.apply(&mut connections), None, "no host");
        editor.draft.host = " test.example.com ".to_owned();
        assert_eq!(editor.apply(&mut connections), Some(1));
        assert_eq!(connections[1].name, "Test");
        assert_eq!(connections[1].host, "test.example.com");
    }

    #[test]
    fn renaming_a_connection_is_reported_once_r_sql_46() {
        let mut connections = vec![named("Prod")];
        let mut editor = Editor::open(&connections, Some(0));
        editor.draft.name = "Production".to_owned();
        assert_eq!(editor.apply(&mut connections), Some(0));
        assert_eq!(
            editor.take_renamed(),
            Some(("Prod".to_owned(), "Production".to_owned()))
        );
        assert_eq!(editor.take_renamed(), None);
        assert_eq!(connections.len(), 1, "edited, not added");
    }

    #[test]
    fn a_connection_keeps_no_password_and_new_ones_use_tls_r_sql_46() {
        let connection = SqlConnection::default();
        assert!(connection.tls, "R-P-3: TLS unless turned off");
        let json = serde_json::to_string(&named("Prod")).expect("a connection serializes");
        assert!(!json.contains("password"), "{json}");
        let config = named("Prod").config();
        assert_eq!(
            (config.host.as_str(), config.port),
            ("prod.example.com", 992)
        );
    }

    #[test]
    fn the_card_names_the_host_tls_and_user_r_sql_46() {
        let mut connection = named("Prod");
        connection.user = "QPGMR".to_owned();
        assert_eq!(
            connection.entry().address,
            "prod.example.com  ·  TLS  ·  QPGMR"
        );
    }
}
