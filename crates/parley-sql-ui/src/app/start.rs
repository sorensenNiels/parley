//! Parley SQL's start screen (R-SQL-105, D64) on [`connect`]'s: the saved connections with their favourites, the
//! recent connections and quick connect; and the recent connections kept.

use egui::{CentralPanel, Ui};
use parley_ui_egui::connect;

use super::{ParleySqlUi, SqlConnection};

/// The start screen's words: saved connections.
const CONNECTIONS: connect::Words = connect::Words {
    id: "sql-start",
    heading: "Connect to IBM i",
    none: "No connections yet.",
    new: "New connection…",
    all: "All connections",
    find: "Find a connection",
    no_match: "No saved connection matches.",
    save_as: "Save as connection…",
    port: false,
};

impl ParleySqlUi {
    pub(super) fn start_screen(&mut self, ui: &mut Ui) {
        CentralPanel::default().show(ui, |ui| {
            let saved: Vec<connect::Saved> =
                self.connections.iter().map(SqlConnection::saved).collect();
            let recent: Vec<connect::Recent> = self
                .settings
                .recent_connections
                .iter()
                .map(|r| connect::Recent {
                    address: r.address(),
                    saved: recent_saved(&self.connections, r),
                })
                .collect();
            let lists = connect::Lists {
                saved: &saved,
                recent: &recent,
                groups: None,
                all_open: self.settings.all_connections_open,
                last_error: None,
            };
            if let Some(choice) = self.start.show(ui, CONNECTIONS, lists) {
                self.chosen(choice);
            }
        });
    }

    /// Does what was chosen on the start screen.
    fn chosen(&mut self, choice: connect::Choice) {
        match choice {
            connect::Choice::Connect(i) => self.connect(i),
            connect::Choice::Edit(i) => self.open_editor(Some(i)),
            connect::Choice::New => self.open_editor(None),
            connect::Choice::Favourite(i) => {
                if let Some(connection) = self.connections.get_mut(i) {
                    connection.favourite = !connection.favourite;
                }
            }
            connect::Choice::Recent(r) => {
                let Some(kept) = self.settings.recent_connections.get(r).cloned() else {
                    return;
                };
                match recent_saved(&self.connections, &kept) {
                    // A saved connection opens with its settings as they are now.
                    Some(i) => self.connect(i),
                    None => self.open(named_by_host(kept)),
                }
            }
            connect::Choice::RecentSave(r) => {
                if let Some(kept) = self.settings.recent_connections.get(r) {
                    self.editor = Some(super::connections::Editor::open_new(kept.clone()));
                }
            }
            connect::Choice::Quick(quick) => self.open(named_by_host(SqlConnection {
                host: quick.host,
                tls: quick.tls,
                ..SqlConnection::default()
            })),
            connect::Choice::AllOpen(open) => self.settings.all_connections_open = open,
            // Parley SQL has no groups.
            connect::Choice::StartGroup(_) | connect::Choice::EditGroup(_) => {}
        }
    }

    /// Puts `connection` first among the recent connections: by name when it is saved, else by its host, without a
    /// name (R-SQL-105).
    pub(super) fn remember_connection(&mut self, connection: &SqlConnection) {
        let mut kept = connection.clone();
        if !self.connections.iter().any(|c| c.name == connection.name) {
            kept.name.clear();
        }
        let recent = &mut self.settings.recent_connections;
        recent.retain(|r| !same_recent(r, &kept));
        recent.insert(0, kept);
        recent.truncate(connect::RECENT_KEPT);
    }
}

/// Where a connection not saved goes: its host and TLS.
fn place(connection: &SqlConnection) -> (String, bool) {
    (connection.host.trim().to_lowercase(), connection.tls)
}

/// The same recent connection: the same saved one, or the same place for one not saved.
fn same_recent(a: &SqlConnection, b: &SqlConnection) -> bool {
    if a.name.is_empty() && b.name.is_empty() {
        place(a) == place(b)
    } else {
        a.name == b.name
    }
}

/// The saved connection a recent one was made with: by its name, or by its host and TLS for one kept without a name;
/// `None` once it is renamed or deleted, or for a quick connect never saved.
fn recent_saved(connections: &[SqlConnection], recent: &SqlConnection) -> Option<usize> {
    if recent.name.is_empty() {
        let wanted = place(recent);
        return connections.iter().position(|c| place(c) == wanted);
    }
    connections.iter().position(|c| c.name == recent.name)
}

/// A connection not saved, named by its host for the title, its history and its scripts.
fn named_by_host(mut connection: SqlConnection) -> SqlConnection {
    connection.name = connection.host.trim().to_owned();
    connection
}

#[cfg(test)]
mod tests {
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    use super::*;
    use crate::settings::Settings;

    fn connection(name: &str, host: &str) -> SqlConnection {
        SqlConnection {
            name: name.to_owned(),
            host: host.to_owned(),
            ..SqlConnection::default()
        }
    }

    fn start(connections: Vec<SqlConnection>, settings: Settings) -> Harness<'static, ParleySqlUi> {
        let mut app = ParleySqlUi::new(connections);
        app.set_settings(settings);
        Harness::builder()
            .with_size(egui::vec2(1100.0, 800.0))
            .with_max_steps(16)
            .build_ui_state(|ui, app: &mut ParleySqlUi| app.ui(ui), app)
    }

    #[test]
    fn recent_connections_are_newest_first_without_repeats_and_kept_by_host_when_not_saved_r_sql_105()
     {
        let mut app = ParleySqlUi::new(vec![connection("Prod", "prod.lan")]);
        let prod = app.connections[0].clone();
        app.remember_connection(&named_by_host(connection("", "Quick.lan")));
        app.remember_connection(&prod);
        app.remember_connection(&named_by_host(connection("", "quick.lan")));
        let kept: Vec<(&str, &str)> = app
            .settings
            .recent_connections
            .iter()
            .map(|r| (r.name.as_str(), r.host.as_str()))
            .collect();
        assert_eq!(kept, [("", "quick.lan"), ("Prod", "prod.lan")]);
        for n in 0..6 {
            app.remember_connection(&named_by_host(connection("", &format!("h{n}.æøå"))));
        }
        assert_eq!(app.settings.recent_connections.len(), connect::RECENT_KEPT);
    }

    #[test]
    fn a_recent_connection_is_found_by_name_or_by_host_and_tls_r_sql_105() {
        let connections = vec![connection("Prod", "prod.lan")];
        assert_eq!(recent_saved(&connections, &connections[0]), Some(0));
        assert_eq!(
            recent_saved(&connections, &connection("Old", "prod.lan")),
            None
        );
        assert_eq!(
            recent_saved(&connections, &connection("", "PROD.lan")),
            Some(0)
        );
        let mut plain = connection("", "prod.lan");
        plain.tls = false;
        assert_eq!(recent_saved(&connections, &plain), None, "TLS differs");
    }

    #[test]
    fn the_star_makes_a_favourite_and_all_connections_folds_once_something_is_above_it_r_sql_105() {
        let mut h = start(
            vec![
                connection("Alpha", "a.lan"),
                connection("Beta æøå", "b.lan"),
            ],
            Settings::default(),
        );
        h.run_steps(2);
        h.get_by_label("Alpha");
        assert!(
            !h.state().settings.all_connections_open,
            "open with nothing above it"
        );
        h.get_all_by_label(connect::list::ADD_FAVOURITE)
            .nth(1)
            .expect("Beta's star")
            .click();
        h.run_steps(2);
        assert!(h.state().connections[1].favourite);
        h.get_by_label("FAVOURITES");
        assert!(
            h.query_by_label("Alpha").is_none(),
            "folded under a favourite"
        );
        h.get_by_label("ALL CONNECTIONS (2)").click();
        h.run_steps(2);
        assert!(h.state().settings.all_connections_open, "kept as left");
        h.get_by_label("Alpha");
        assert!(h.query_by_label("Session groups…").is_none(), "no groups");
    }

    #[test]
    fn recent_lists_one_not_saved_with_save_as_connection_r_sql_105() {
        let settings = Settings {
            recent_connections: vec![connection("", "quick.lan")],
            ..Settings::default()
        };
        let mut h = start(Vec::new(), settings);
        h.run_steps(2);
        h.get_by_label("RECENT");
        h.get_by_label("quick.lan  ·  TLS");
        h.get_by_label("Not saved");
        h.get_by_label("Save as connection…").click();
        h.run_steps(2);
        assert!(h.state().editor.is_some(), "the connection dialog");
    }

    #[test]
    fn quick_connect_takes_no_port_r_sql_105() {
        let mut h = start(Vec::new(), Settings::default());
        h.run_steps(2);
        h.get_by_label(connect::QUICK_HOST).focus();
        h.run();
        h.event(egui::Event::Text("prod.lan:8471".into()));
        h.run();
        h.get_by_label("Connect").click();
        h.run_steps(2);
        assert!(h.state().window.is_none());
        h.get_by_label("Name the host without a port: Parley SQL finds the port itself.");
    }
}
