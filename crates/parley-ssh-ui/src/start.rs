//! The start screen (R-SSH-2, R-SSH-11): the saved sessions with a search, the defaults, and a quick connect to a
//! host, port and user name that is not saved.

use egui::{Key, Ui};
use parley_ssh_core::net::Target;
use parley_ui_egui::{connect, theme};

use crate::sessions::{DEFAULT_SETTINGS, SavedSession};
use crate::settings::Settings;

const WORDS: connect::Words = connect::Words {
    heading: "Connect to a server",
    none: "No saved sessions yet.",
    new: "New session…",
};

/// From this many saved sessions on, the list has a search field.
const SEARCH_FROM: usize = 4;

/// What the user chose on the start screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Choice {
    /// The saved session at this index.
    Connect(usize),
    Edit(usize),
    New,
    Defaults,
    /// Quick connect, with the defaults' keys and agent.
    Quick(Target),
}

/// The start screen's fields, as typed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Form {
    pub host: String,
    pub port: String,
    pub user: String,
    pub search: String,
    pub error: Option<String>,
}

impl Form {
    /// The quick connect as it was last used.
    pub fn from_settings(settings: &Settings) -> Self {
        Self {
            host: settings.last_host.clone(),
            port: settings.last_port.to_string(),
            user: settings.last_user.clone(),
            search: String::new(),
            error: None,
        }
    }

    pub fn show(&mut self, ui: &mut Ui, sessions: &[SavedSession]) -> Option<Choice> {
        let mut choice = None;
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let shown = matching(sessions, &self.search);
                let entries: Vec<connect::Entry<'_>> = shown
                    .iter()
                    .filter_map(|&i| sessions.get(i))
                    .map(|s| connect::Entry {
                        name: &s.name,
                        address: s.address(),
                    })
                    .collect();
                choice = connect::show(ui, WORDS, &entries, None).map(|c| match c {
                    connect::Choice::Connect(i) => Choice::Connect(shown[i]),
                    connect::Choice::Edit(i) => Choice::Edit(shown[i]),
                    connect::Choice::New => Choice::New,
                });
                ui.add_space(8.0);
                ui.vertical_centered(|ui| {
                    if sessions.len() >= SEARCH_FROM {
                        let edit = egui::TextEdit::singleline(&mut self.search)
                            .hint_text("Find a session");
                        let response = theme::sized_edit(ui, 300.0, edit);
                        if response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                            choice = shown.first().map(|&i| Choice::Connect(i));
                        }
                    }
                    if ui
                        .add(theme::ghost_button(format!("{DEFAULT_SETTINGS}…")))
                        .clicked()
                    {
                        choice = Some(Choice::Defaults);
                    }
                    ui.add_space(16.0);
                    if let Some(quick) = self.quick(ui) {
                        choice = Some(quick);
                    }
                });
            });
        choice
    }

    /// Quick connect: host, port and user, not saved.
    fn quick(&mut self, ui: &mut Ui) -> Option<Choice> {
        let mut go = false;
        theme::card(ui).show(ui, |ui| {
            ui.set_width(380.0);
            theme::section_label(ui, "Quick connect");
            egui::Grid::new("ssh-start")
                .num_columns(2)
                .spacing([12.0, 8.0])
                .show(ui, |ui| {
                    for (name, value, width, hint) in [
                        (
                            "Host",
                            &mut self.host,
                            250.0,
                            "server.example.com or user@server",
                        ),
                        ("Port", &mut self.port, 80.0, "22"),
                        ("User name", &mut self.user, 250.0, ""),
                    ] {
                        let label = ui.label(name);
                        let edit = egui::TextEdit::singleline(value).hint_text(hint);
                        let response = theme::sized_edit(ui, width, edit).labelled_by(label.id);
                        go |= response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                        ui.end_row();
                    }
                });
            if let Some(error) = &self.error {
                ui.add_space(6.0);
                theme::error(ui, error);
            }
            ui.add_space(8.0);
            theme::button_row(ui, |ui| {
                go |= ui.add(theme::primary_button(ui, "Connect")).clicked();
            });
        });
        if !go {
            return None;
        }
        match parse(&self.host, &self.port, &self.user) {
            Ok(target) => {
                self.error = None;
                Some(Choice::Quick(target))
            }
            Err(e) => {
                self.error = Some(e);
                None
            }
        }
    }
}

/// The indexes of the sessions whose name or address holds `search`, ignoring case; all when it is empty.
fn matching(sessions: &[SavedSession], search: &str) -> Vec<usize> {
    let search = search.trim().to_lowercase();
    sessions
        .iter()
        .enumerate()
        .filter(|(_, s)| {
            search.is_empty()
                || s.name.to_lowercase().contains(&search)
                || s.address().to_lowercase().contains(&search)
        })
        .map(|(i, _)| i)
        .collect()
}

/// The fields as a [`Target`]. The host may carry the user and port, as `ssh` takes them: `user@host:port`;
/// `[::1]:2222` for an IPv6 address with a port.
pub fn parse(host: &str, port: &str, user: &str) -> Result<Target, String> {
    let mut host = host.trim();
    let mut user = user.trim().to_owned();
    if let Some((name, rest)) = host.rsplit_once('@') {
        name.trim().clone_into(&mut user);
        host = rest.trim();
    }
    let mut port = port.trim().to_owned();
    if let Some(rest) = host.strip_prefix('[') {
        let (address, after) = rest
            .split_once(']')
            .ok_or("An IPv6 address in brackets needs its ]")?;
        if let Some(p) = after.strip_prefix(':') {
            p.clone_into(&mut port);
        }
        host = address;
    } else if host.matches(':').count() == 1
        && let Some((name, p)) = host.split_once(':')
    {
        host = name;
        p.clone_into(&mut port);
    }
    if host.is_empty() {
        return Err("Name the server to connect to.".to_owned());
    }
    if host.chars().any(char::is_whitespace) {
        return Err("A host name has no spaces.".to_owned());
    }
    if user.is_empty() {
        return Err("Fill in the user name to sign on as.".to_owned());
    }
    let port = if port.is_empty() {
        22
    } else {
        port.parse::<u16>()
            .ok()
            .filter(|p| *p > 0)
            .ok_or("The port is a number from 1 to 65535.")?
    };
    Ok(Target {
        host: host.to_owned(),
        port,
        user,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(host: &str, port: u16, user: &str) -> Target {
        Target {
            host: host.into(),
            port,
            user: user.into(),
        }
    }

    #[test]
    fn the_fields_make_a_target_r_ssh_2() {
        assert_eq!(
            parse(" server ", "22", "æøå"),
            Ok(target("server", 22, "æøå"))
        );
        assert_eq!(parse("server", "", "me"), Ok(target("server", 22, "me")));
    }

    #[test]
    fn the_host_may_carry_user_and_port_as_ssh_takes_them() {
        assert_eq!(
            parse("root@box:2222", "22", "me"),
            Ok(target("box", 2222, "root"))
        );
        assert_eq!(
            parse("[::1]:2200", "22", "me"),
            Ok(target("::1", 2200, "me"))
        );
        assert_eq!(
            parse("fe80::1", "22", "me"),
            Ok(target("fe80::1", 22, "me"))
        );
    }

    #[test]
    fn wrong_fields_are_said_in_plain_words() {
        assert!(parse("", "22", "me").is_err());
        assert!(parse("a b", "22", "me").is_err());
        assert!(parse("server", "22", "").is_err());
        assert!(parse("server", "0", "me").is_err());
        assert!(parse("server", "70000", "me").is_err());
        assert!(parse("[::1", "22", "me").is_err());
    }

    #[test]
    fn the_search_finds_names_and_addresses_ignoring_case_r_ssh_11() {
        let session = |name: &str, host: &str| SavedSession {
            name: name.into(),
            host: host.into(),
            user: "u".into(),
            ..SavedSession::default()
        };
        let sessions = [
            session("Prod Æøå", "web1"),
            session("Test", "db.lan"),
            session("Backup", "WEB2"),
        ];
        assert_eq!(matching(&sessions, ""), [0, 1, 2]);
        assert_eq!(matching(&sessions, "web"), [0, 2]);
        assert_eq!(matching(&sessions, "æØÅ"), [0]);
        assert_eq!(matching(&sessions, "nothing"), Vec::<usize>::new());
    }
}
