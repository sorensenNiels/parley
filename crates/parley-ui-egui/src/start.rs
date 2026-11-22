//! Parley's start screen (R-UI-40..42, D64) on [`connect`]'s: the saved sessions with their favourites, the recent
//! connections, the session groups and quick connect; and the recent connections kept.

use parley_core::SessionConfig;
use parley_core::session::TLS_PORT;

use crate::menu::Action;
use crate::profile::Profile;
use crate::{ParleyUi, connect};

/// Telnet's own port, for a quick connect without TLS.
const TELNET_PORT: u16 = 23;

impl ParleyUi {
    /// The start screen; what it asks goes to `actions`, or opens a dialog.
    pub(crate) fn start_screen(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        let saved: Vec<connect::Saved> =
            self.profiles.iter().map(connect::Saved::session).collect();
        let recent: Vec<connect::Recent> = self
            .settings
            .recent_sessions
            .iter()
            .map(|r| connect::Recent {
                address: address(&r.config),
                saved: recent_saved(&self.profiles, r),
            })
            .collect();
        let groups: Vec<connect::Group> = self
            .settings
            .session_groups
            .iter()
            .map(|g| connect::Group {
                name: g.name.clone(),
                sessions: g.sessions.join(", "),
            })
            .collect();
        let lists = connect::Lists {
            saved: &saved,
            recent: &recent,
            groups: Some(&groups),
            all_open: self.settings.all_sessions_open,
            last_error: self.closed.as_deref(),
        };
        let Some(choice) = self.start.show(ui, connect::SESSIONS, lists) else {
            return;
        };
        match choice {
            connect::Choice::Connect(i) => actions.push(Action::Connect(i)),
            connect::Choice::Edit(i) => self.open_editor(Some(i)),
            connect::Choice::New => actions.push(Action::NewSession),
            connect::Choice::Favourite(i) => {
                if let Some(profile) = self.profiles.get_mut(i) {
                    profile.favourite = !profile.favourite;
                }
            }
            connect::Choice::Recent(r) => {
                let Some(kept) = self.settings.recent_sessions.get(r) else {
                    return;
                };
                match recent_saved(&self.profiles, kept) {
                    // A saved session connects with its settings as they are now (R-UI-41).
                    Some(i) => actions.push(Action::Connect(i)),
                    None => self.pending = Some(named_by_address(kept.clone())),
                }
            }
            connect::Choice::RecentSave(r) => {
                if let Some(kept) = self.settings.recent_sessions.get(r) {
                    self.editor = Some(crate::sessions::Editor::open_new(kept.clone()));
                }
            }
            connect::Choice::Quick(quick) => self.pending = Some(quick_profile(&quick)),
            connect::Choice::AllOpen(open) => self.settings.all_sessions_open = open,
            connect::Choice::StartGroup(g) => actions.push(Action::StartGroup(g)),
            connect::Choice::EditGroup(g) => self.open_groups(g),
        }
    }

    /// Puts `profile` first among the recent connections (R-UI-41): by name when it is a saved session, else by its
    /// address, without a name.
    pub(crate) fn remember_connection(&mut self, profile: &Profile) {
        let mut kept = profile.clone();
        if !self.profiles.iter().any(|p| p.name == profile.name) {
            kept.name.clear();
        }
        let recent = &mut self.settings.recent_sessions;
        recent.retain(|r| !same_recent(r, &kept));
        recent.insert(0, kept);
        recent.truncate(connect::RECENT_KEPT);
    }
}

/// A recent connection's line: `host:port · TLS`.
fn address(config: &SessionConfig) -> String {
    connect::address(&config.host, Some(config.port), config.tls)
}

/// The same recent connection: the same saved session, or the same address for one not saved.
fn same_recent(a: &Profile, b: &Profile) -> bool {
    if a.name.is_empty() && b.name.is_empty() {
        address(&a.config) == address(&b.config)
    } else {
        a.name == b.name
    }
}

/// The saved session a recent connection was made with: by its name, or by its address for one kept without a name
/// (R-UI-41); `None` once it is renamed or deleted, or for a quick connect never saved.
fn recent_saved(profiles: &[Profile], recent: &Profile) -> Option<usize> {
    if recent.name.is_empty() {
        let address = address(&recent.config);
        return profiles
            .iter()
            .position(|p| self::address(&p.config) == address);
    }
    profiles.iter().position(|p| p.name == recent.name)
}

/// An unsaved session, named by its address for its tab and the window's title (R-UI-41).
fn named_by_address(mut profile: Profile) -> Profile {
    profile.name = address(&profile.config);
    profile
}

/// Quick connect's session: the default settings on its host, TLS's port or telnet's unless one was typed (R-UI-41).
fn quick_profile(quick: &connect::Quick) -> Profile {
    let default = if quick.tls { TLS_PORT } else { TELNET_PORT };
    named_by_address(Profile {
        config: SessionConfig {
            host: quick.host.clone(),
            port: quick.port.unwrap_or(default),
            tls: quick.tls,
            ..Profile::default().config
        },
        ..Profile::default()
    })
}

#[cfg(test)]
mod tests {
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    use super::*;
    use crate::connect::list;
    use crate::test_support::profile;

    fn ui_with(profiles: Vec<Profile>) -> ParleyUi {
        ParleyUi::new(profiles)
    }

    fn quick(host: &str, port: Option<u16>, tls: bool) -> Profile {
        quick_profile(&connect::Quick {
            host: host.to_owned(),
            port,
            tls,
        })
    }

    #[test]
    fn quick_connect_takes_tls_or_telnet_port_and_is_named_by_its_address_r_ui_41() {
        let tls = quick("prod.lan", None, true);
        assert_eq!((tls.config.port, tls.config.tls), (TLS_PORT, true));
        assert_eq!(tls.name, "prod.lan:992  ·  TLS");
        let telnet = quick("æøå.dk", None, false);
        assert_eq!(telnet.config.port, TELNET_PORT);
        assert_eq!(quick("prod.lan", Some(2323), false).config.port, 2323);
    }

    #[test]
    fn recent_connections_are_newest_first_without_repeats_and_at_most_five_r_ui_41() {
        let mut ui = ui_with(vec![
            profile("Prod", "prod.lan"),
            profile("Test", "test.lan"),
        ]);
        let prod = ui.profiles[0].clone();
        let test = ui.profiles[1].clone();
        ui.remember_connection(&prod);
        ui.remember_connection(&test);
        ui.remember_connection(&prod);
        let names: Vec<&str> = ui
            .settings
            .recent_sessions
            .iter()
            .map(|r| r.name.as_str())
            .collect();
        assert_eq!(names, ["Prod", "Test"]);
        for n in 0..6 {
            ui.remember_connection(&quick(&format!("h{n}.lan"), None, true));
        }
        assert_eq!(ui.settings.recent_sessions.len(), connect::RECENT_KEPT);
        assert!(
            ui.settings
                .recent_sessions
                .iter()
                .all(|r| r.name.is_empty()),
            "a quick connect is kept without a name"
        );
        ui.remember_connection(&quick("h5.lan", None, true));
        assert_eq!(
            ui.settings.recent_sessions.len(),
            connect::RECENT_KEPT,
            "the same address once"
        );
    }

    #[test]
    fn a_recent_saved_session_is_found_by_name_and_one_renamed_is_not_r_ui_41() {
        let profiles = vec![profile("Prod", "prod.lan"), profile("Test", "test.lan")];
        assert_eq!(recent_saved(&profiles, &profiles[1]), Some(1));
        let renamed = profile("Prod old", "prod.lan");
        assert_eq!(recent_saved(&profiles, &renamed), None);
        let mut unnamed = profiles[0].clone();
        unnamed.name.clear();
        assert_eq!(recent_saved(&profiles, &unnamed), Some(0), "by its address");
    }

    /// Parley on its start screen with `profiles` and `settings`, quick connect beside the list.
    fn start(
        profiles: Vec<Profile>,
        settings: crate::settings::Settings,
    ) -> Harness<'static, ParleyUi> {
        let mut app = ParleyUi::new(profiles);
        app.set_settings(settings);
        Harness::builder()
            .with_size(egui::vec2(1100.0, 800.0))
            .with_max_steps(16)
            .build_ui_state(|ui, app: &mut ParleyUi| app.ui(ui), app)
    }

    fn open_all() -> crate::settings::Settings {
        crate::settings::Settings {
            all_sessions_open: true,
            ..crate::settings::Settings::default()
        }
    }

    #[test]
    fn the_star_makes_a_session_a_favourite_listed_first_and_back_r_ui_40() {
        let mut h = start(
            vec![profile("Alpha", "a.lan"), profile("Beta æøå", "b.lan")],
            open_all(),
        );
        h.run_steps(2);
        assert!(h.query_by_label("FAVOURITES").is_none());
        h.get_all_by_label(list::ADD_FAVOURITE)
            .nth(1)
            .expect("Beta's star")
            .click();
        h.run_steps(2);
        let favourites: Vec<bool> = h.state().profiles.iter().map(|p| p.favourite).collect();
        assert_eq!(favourites, [false, true]);
        h.get_by_label("FAVOURITES");
        assert_eq!(
            h.get_all_by_label("Beta æøå").count(),
            2,
            "a favourite is listed under All sessions too"
        );
        h.get_all_by_label(list::REMOVE_FAVOURITE)
            .next()
            .expect("Beta's star")
            .click();
        h.run_steps(2);
        assert!(!h.state().profiles[1].favourite);
        assert!(h.query_by_label("FAVOURITES").is_none());
    }

    #[test]
    fn all_sessions_is_open_with_nothing_above_it_and_else_kept_as_left_r_ui_42() {
        let mut beta = profile("Beta", "b.lan");
        beta.favourite = true;
        let mut h = start(
            vec![profile("Alpha", "a.lan")],
            crate::settings::Settings::default(),
        );
        h.run_steps(2);
        h.get_by_label("Alpha");
        h.state_mut().profiles.push(beta);
        h.run_steps(2);
        assert!(
            h.query_by_label("Alpha").is_none(),
            "folded under a favourite"
        );
        h.get_by_label("ALL SESSIONS (2)").click();
        h.run_steps(2);
        h.get_by_label("Alpha");
        assert!(h.state().settings.all_sessions_open, "kept in the settings");
    }

    #[test]
    fn quick_connect_connects_unsaved_and_recent_offers_to_save_it_r_ui_41() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a local port");
        let port = listener.local_addr().expect("its address").port();
        let mut h = start(Vec::new(), crate::settings::Settings::default());
        h.run_steps(2);
        h.get_by_label("Use TLS").click();
        h.get_by_label(connect::QUICK_HOST).focus();
        h.run();
        h.event(egui::Event::Text(format!("127.0.0.1:{port}")));
        h.run();
        h.get_by_label("Connect").click();
        h.run_steps(2);
        let (_stream, _) = listener.accept().expect("quick connect's connection");
        let name = format!("127.0.0.1:{port}");
        assert_eq!(
            h.state().current.as_ref().map(|p| p.name.as_str()),
            Some(name.as_str())
        );
        assert!(h.state().profiles.is_empty(), "not saved");
        let recent = &h.state().settings.recent_sessions;
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].name, "", "kept by its address");

        h.state_mut().connection.take().expect("connected").close();
        h.run_steps(2);
        h.get_by_label("RECENT");
        h.get_by_label("Not saved");
        h.get_by_label("Save as session…").click();
        h.run_steps(2);
        let editor = h.state().editor.as_ref().expect("the session dialog");
        assert_eq!(editor.draft.config.port, port);
        assert!(!editor.draft.config.tls);
    }

    #[test]
    fn the_search_has_the_keyboard_and_down_and_enter_connect_a_match_r_ui_42() {
        let mut h = start(
            vec![
                profile("Web one", "127.0.0.1"),
                profile("Database", "127.0.0.1"),
                profile("Web two", "127.0.0.1"),
                profile("Mail", "127.0.0.1"),
            ],
            crate::settings::Settings::default(),
        );
        h.run_steps(3);
        h.event(egui::Event::Text("web".into()));
        h.run_steps(2);
        h.get_by_label("Web one");
        assert!(h.query_by_label("Database").is_none(), "only the matches");
        h.key_press(egui::Key::ArrowDown);
        h.key_press(egui::Key::Enter);
        h.run_steps(2);
        let current = h.state().current.as_ref().map(|p| p.name.clone());
        assert_eq!(current.as_deref(), Some("Web two"));
    }

    #[test]
    fn the_groups_are_listed_with_start_and_edit_r_ui_42() {
        let mut settings = open_all();
        settings.session_groups = vec![crate::groups::SessionGroup {
            name: "Morgen".into(),
            sessions: vec!["Alpha".into()],
            start_on_launch: false,
        }];
        let mut h = start(vec![profile("Alpha", "a.lan")], settings);
        h.run_steps(2);
        h.get_by_label("GROUPS (1)");
        h.get_by_label("Morgen");
        h.get_by_label("Start");
        h.get_by_label("Session groups…").click();
        h.run_steps(2);
        assert!(h.state().groups_dialog.is_some());
    }
}
