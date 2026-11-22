//! The start screen (R-SSH-2, R-SSH-11): the saved sessions with a search, the defaults, and a quick connect to a
//! host, port and user name that is not saved; the hosts of `~/.ssh/config` below the saved sessions (R-SSH-42).
//! For many sessions (R-SSH-65): the favourites first (R-SSH-63), the recent connections (R-SSH-64), the rest under
//! headings that fold, a search at the top that the keyboard drives, and quick connect beside the list.

use egui::{Key, Ui};
use parley_ssh_core::net::Target;
use parley_ssh_core::openssh_config::Host;
use parley_ui_egui::fuzzy::fit;
use parley_ui_egui::theme;

use crate::openssh;
use crate::palette;
use crate::sessions::{DEFAULT_SETTINGS, SavedSession};
use crate::settings::Settings;

use parley_ui_egui::connect::list::{self, Clicked, Fold, Row, Star};

/// From this many saved sessions and config hosts on, the list has a search field.
const SEARCH_FROM: usize = 4;
/// How many recent connections are listed (R-SSH-64).
const RECENT_SHOWN: usize = 5;
/// The config's hosts start folded when there are more than this and something is shown above them (R-SSH-65).
const FOLD_FROM: usize = 10;

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
    /// The host of `~/.ssh/config` at this index (R-SSH-42).
    ConfigConnect(usize),
    /// Keep the host of `~/.ssh/config` at this index as a saved session.
    ConfigSave(usize),
    /// Mark the saved session at this index as a favourite, or no longer (R-SSH-63).
    Favourite(usize),
    /// The recent connection at this index (R-SSH-64).
    Recent(usize),
    /// Keep the recent connection at this index, not a saved session, as one.
    RecentSave(usize),
    /// Open or fold "All sessions", kept in the settings (R-SSH-65).
    AllSessionsOpen(bool),
}

/// The heading of the hosts from OpenSSH's config.
const CONFIG_GROUP: &str = "From ~/.ssh/config";

/// The start screen's fields, as typed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Form {
    pub host: String,
    pub port: String,
    pub user: String,
    pub search: String,
    pub error: Option<String>,
    /// The match chosen with Up and Down (R-SSH-65).
    selected: usize,
    /// The chosen match is to be scrolled into view.
    scroll: bool,
    /// The search is to get the keyboard once the pane is in front.
    focus: bool,
}

/// What one row of the start screen stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Entry {
    /// A saved session.
    Session(usize),
    /// A host of `~/.ssh/config`.
    Host(usize),
    /// A recent connection, and the saved session it was made with while that is still saved.
    Recent(usize, Option<usize>),
}

impl Entry {
    /// What a click in its row chooses.
    fn choice(self, clicked: Clicked) -> Option<Choice> {
        match (self, clicked) {
            (Self::Session(i), Clicked::Connect) => Some(Choice::Connect(i)),
            (Self::Session(i) | Self::Recent(_, Some(i)), Clicked::Second) => Some(Choice::Edit(i)),
            (Self::Session(i) | Self::Recent(_, Some(i)), Clicked::Star) => {
                Some(Choice::Favourite(i))
            }
            (Self::Host(i), Clicked::Connect) => Some(Choice::ConfigConnect(i)),
            (Self::Host(i), Clicked::Second) => Some(Choice::ConfigSave(i)),
            (Self::Recent(r, _), Clicked::Connect) => Some(Choice::Recent(r)),
            (Self::Recent(r, None), Clicked::Second) => Some(Choice::RecentSave(r)),
            (Self::Host(_) | Self::Recent(_, None), Clicked::Star) => None,
        }
    }
}

/// What the lists are made from.
#[derive(Clone, Copy)]
pub struct Lists<'a> {
    pub sessions: &'a [SavedSession],
    pub recent: &'a [SavedSession],
    pub openssh: &'a openssh::Hosts,
    /// "All sessions" is open (R-SSH-65).
    pub all_open: bool,
}

impl Lists<'_> {
    /// The line that shows `entry`, chosen or not.
    fn row(&self, entry: Entry, selected: bool) -> Option<Row> {
        let saved = |s: &SavedSession| Row {
            name: s.name.clone(),
            address: s.address(),
            star: if s.favourite { Star::On } else { Star::Off },
            primary: "Connect",
            second: "Edit…",
            selected,
        };
        Some(match entry {
            Entry::Session(i) | Entry::Recent(_, Some(i)) => saved(self.sessions.get(i)?),
            Entry::Host(i) => {
                let host = self.openssh.hosts().get(i)?;
                Row {
                    name: host.alias.clone(),
                    address: address(host),
                    star: Star::None,
                    primary: "Connect",
                    second: "Save as session…",
                    selected,
                }
            }
            Entry::Recent(r, None) => Row {
                name: self.recent.get(r)?.address(),
                address: "Not saved".to_owned(),
                star: Star::None,
                primary: "Connect",
                second: "Save as session…",
                selected,
            },
        })
    }

    /// The rows of `entries`, the `selected`-th marked; what was clicked.
    fn rows(
        &self,
        ui: &mut Ui,
        entries: &[Entry],
        selected: Option<usize>,
        scroll: bool,
    ) -> Option<Choice> {
        let mut choice = None;
        for (n, &entry) in entries.iter().enumerate() {
            let Some(row) = self.row(entry, selected == Some(n)) else {
                continue;
            };
            if let Some(clicked) = list::row(ui, &row, scroll) {
                choice = entry.choice(clicked);
            }
            ui.add_space(2.0);
        }
        choice
    }

    fn saved_sessions_and_hosts(&self) -> usize {
        self.sessions.len() + self.openssh.hosts().len()
    }
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
            selected: 0,
            scroll: false,
            focus: true,
        }
    }

    /// The start screen; `active` when its pane is the one in front.
    pub fn show(&mut self, ui: &mut Ui, lists: Lists<'_>, active: bool) -> Option<Choice> {
        let mut choice = None;
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(ui.available_height() * 0.06);
                    ui.heading("Connect to a server");
                    ui.add_space(16.0);
                });
                choice = list::columns(
                    ui,
                    self,
                    |form, ui| form.lists(ui, lists, active),
                    Form::side,
                );
                ui.add_space(24.0);
            });
        choice
    }

    /// Quick connect, then New session… and Default Settings….
    fn side(&mut self, ui: &mut Ui) -> Option<Choice> {
        let mut choice = self.quick(ui);
        ui.add_space(12.0);
        let width = ui.available_width();
        if ui
            .add(egui::Button::new("New session…").min_size(egui::vec2(width, 34.0)))
            .clicked()
        {
            choice = Some(Choice::New);
        }
        ui.add_space(4.0);
        ui.vertical_centered(|ui| {
            if ui
                .add(theme::ghost_button(format!("{DEFAULT_SETTINGS}…")))
                .clicked()
            {
                choice = Some(Choice::Defaults);
            }
        });
        choice
    }

    /// The search, then its matches or, without one, the sections.
    fn lists(&mut self, ui: &mut Ui, lists: Lists<'_>, active: bool) -> Option<Choice> {
        let mut choice = None;
        if lists.saved_sessions_and_hosts() >= SEARCH_FROM {
            choice = self.search(ui, lists, active);
            ui.add_space(12.0);
        } else if active {
            self.focus = false;
        }
        let shown = if self.search.trim().is_empty() {
            sections(ui, lists)
        } else {
            let found = found(lists, &self.search);
            if found.is_empty() {
                ui.vertical_centered(|ui| theme::hint(ui, "No saved session or host matches."));
            }
            let shown = lists.rows(ui, &found, Some(self.selected), self.scroll);
            self.scroll = false;
            shown
        };
        shown.or(choice)
    }

    /// The search field: Up and Down choose a match, Enter connects it, Escape empties it (R-SSH-65).
    fn search(&mut self, ui: &mut Ui, lists: Lists<'_>, active: bool) -> Option<Choice> {
        let mut focus = active && self.focus;
        let keys = list::search_field(
            ui,
            "ssh-start-search",
            &mut self.search,
            "Find a session",
            &mut focus,
        );
        if active {
            self.focus = focus;
        }
        if keys.changed || keys.escape {
            self.selected = 0;
        }
        let found = found(lists, &self.search);
        if keys.down {
            self.selected = (self.selected + 1).min(found.len().saturating_sub(1));
            self.scroll = true;
        }
        if keys.up {
            self.selected = self.selected.saturating_sub(1);
            self.scroll = true;
        }
        if !keys.enter {
            return None;
        }
        found
            .get(self.selected)
            .and_then(|&entry| entry.choice(Clicked::Connect))
    }

    /// Quick connect: host, port and user, not saved.
    fn quick(&mut self, ui: &mut Ui) -> Option<Choice> {
        let mut go = false;
        theme::card(ui).show(ui, |ui| {
            ui.set_width(ui.available_width());
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

/// Without a search: the favourites, the recent connections, every other saved session and the config's hosts.
fn sections(ui: &mut Ui, lists: Lists<'_>) -> Option<Choice> {
    let mut choice = None;
    let mut pick = |c: Option<Choice>| {
        if c.is_some() {
            choice = c;
        }
    };
    // Every saved session is under All sessions, the favourites too (the owner, 2026-11-20).
    let all: Vec<Entry> = (0..lists.sessions.len()).map(Entry::Session).collect();
    let favourites: Vec<Entry> = all
        .iter()
        .copied()
        .filter(|&e| matches!(e, Entry::Session(i) if lists.sessions[i].favourite))
        .collect();
    let recent: Vec<Entry> = lists
        .recent
        .iter()
        .take(RECENT_SHOWN)
        .enumerate()
        .map(|(r, s)| Entry::Recent(r, palette::recent_saved(lists.sessions, s)))
        .collect();
    if lists.sessions.is_empty() {
        ui.vertical_centered(|ui| theme::hint(ui, "No saved sessions yet."));
        ui.add_space(8.0);
    }
    let mut above = false;
    for (label, entries) in [("Favourites", &favourites), ("Recent", &recent)] {
        if entries.is_empty() {
            continue;
        }
        theme::section_label(ui, label);
        pick(lists.rows(ui, entries, None, false));
        ui.add_space(12.0);
        above = true;
    }
    if !all.is_empty() {
        let open = list::all_open(lists.all_open, above);
        let fold = Fold::Kept(open);
        let (shown, clicked) =
            list::section(ui, "ssh-start-all", "All sessions", all.len(), fold, |ui| {
                lists.rows(ui, &all, None, false)
            });
        pick(shown.flatten());
        if clicked {
            pick(Some(Choice::AllSessionsOpen(!open)));
        }
        ui.add_space(12.0);
        above = true;
    }
    pick(config_section(ui, lists, above));
    choice
}

/// The hosts of `~/.ssh/config`, with a hint for the lines that were skipped.
fn config_section(ui: &mut Ui, lists: Lists<'_>, above: bool) -> Option<Choice> {
    let hosts = lists.openssh.hosts();
    let problems = lists.openssh.problems();
    let mut choice = None;
    if !hosts.is_empty() {
        let entries: Vec<Entry> = (0..hosts.len()).map(Entry::Host).collect();
        let fold = Fold::Starts(!above || hosts.len() <= FOLD_FROM);
        choice = list::section(
            ui,
            "ssh-start-config",
            CONFIG_GROUP,
            hosts.len(),
            fold,
            |ui| lists.rows(ui, &entries, None, false),
        )
        .0
        .flatten();
    }
    if let Some(first) = problems.first() {
        let more = match problems.len() {
            1 => String::new(),
            n => format!(" (and {} more)", n - 1),
        };
        theme::warning(ui, format!("Skipped: {first}{more}"));
    }
    choice
}

/// The saved sessions and config hosts that fit `search`, the best fits first, then the favourites, then as listed
/// (R-SSH-65).
fn found(lists: Lists<'_>, search: &str) -> Vec<Entry> {
    let sessions = lists.sessions.iter().enumerate().filter_map(|(i, s)| {
        let other = [
            s.host.as_str(),
            s.user.as_str(),
            s.proxy_jump.as_str(),
            &s.address(),
        ];
        let fit = fit(search, &s.name, &other)?;
        Some((fit, !s.favourite, Entry::Session(i)))
    });
    let hosts = lists
        .openssh
        .hosts()
        .iter()
        .enumerate()
        .filter_map(|(i, h)| {
            let other = [
                h.host_name.as_str(),
                h.user.as_deref().unwrap_or_default(),
                h.proxy_jump.as_deref().unwrap_or_default(),
            ];
            let fit = fit(search, &h.alias, &other)?;
            Some((fit, true, Entry::Host(i)))
        });
    let mut found: Vec<(u32, bool, Entry)> = sessions.chain(hosts).collect();
    // Stable: equal fits keep the sessions before the hosts, each as listed.
    found.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    found.into_iter().map(|(.., entry)| entry).collect()
}

/// A config host's line beside its name: `user@` and `:port` when the config sets them, and its jump host.
fn address(host: &Host) -> String {
    let user = host
        .user
        .as_ref()
        .map_or_else(String::new, |u| format!("{u}@"));
    let port = host
        .port
        .filter(|&p| p != 22)
        .map_or_else(String::new, |p| format!(":{p}"));
    let jump = host
        .proxy_jump
        .as_ref()
        .map_or_else(String::new, |j| format!("  ·  via {j}"));
    format!("{user}{}{port}{jump}", host.host_name)
}

/// `[user@]host[:port]` as `ssh` takes it, `[::1]:2222` for an IPv6 address with a port: the user and the port
/// when they are in it, and the host.
pub fn split(host: &str) -> Result<(Option<String>, String, Option<String>), String> {
    let mut host = host.trim();
    let mut user = None;
    if let Some((name, rest)) = host.rsplit_once('@') {
        user = Some(name.trim().to_owned());
        host = rest.trim();
    }
    let mut port = None;
    if let Some(rest) = host.strip_prefix('[') {
        let (address, after) = rest
            .split_once(']')
            .ok_or("An IPv6 address in brackets needs its ]")?;
        port = after.strip_prefix(':').map(str::to_owned);
        host = address;
    } else if host.matches(':').count() == 1
        && let Some((name, p)) = host.split_once(':')
    {
        host = name;
        port = Some(p.to_owned());
    }
    Ok((user, host.to_owned(), port))
}

/// The fields as a [`Target`]. The host may carry the user and port ([`split`]), which win over the fields.
pub fn parse(host: &str, port: &str, user: &str) -> Result<Target, String> {
    let (typed_user, host, typed_port) = split(host)?;
    let user = typed_user.unwrap_or_else(|| user.trim().to_owned());
    let port = typed_port.unwrap_or_else(|| port.trim().to_owned());
    let port = port.trim();
    let host = host.as_str();
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

    fn session(name: &str, host: &str, user: &str) -> SavedSession {
        SavedSession {
            name: name.into(),
            host: host.into(),
            user: user.into(),
            ..SavedSession::default()
        }
    }

    fn found_in(sessions: &[SavedSession], search: &str) -> Vec<Entry> {
        let openssh = openssh::Hosts::default();
        let lists = Lists {
            sessions,
            recent: &[],
            openssh: &openssh,
            all_open: false,
        };
        found(lists, search)
    }

    #[test]
    fn the_search_finds_names_hosts_users_and_jump_hosts_ignoring_case_r_ssh_65() {
        let mut jumped = session("Inner", "10.0.0.5", "ops");
        jumped.proxy_jump = "bastion.lan".into();
        let sessions = [
            session("Prod Æøå", "web1", "u"),
            session("Test", "db.lan", "deploy"),
            session("Backup", "WEB2", "u"),
            jumped,
        ];
        let at = |search: &str| found_in(&sessions, search);
        assert_eq!(at("web"), [Entry::Session(0), Entry::Session(2)]);
        assert_eq!(at("æØÅ"), [Entry::Session(0)]);
        assert_eq!(at("deploy"), [Entry::Session(1)], "the user name");
        assert_eq!(at("bastion"), [Entry::Session(3)], "the jump host");
        assert_eq!(at("nothing"), Vec::<Entry>::new());
    }

    #[test]
    fn the_search_forgives_a_typo_in_a_name_and_puts_the_best_fit_first_r_ssh_65() {
        let sessions = [
            session("Old production box", "a", "u"),
            session("Production", "b", "u"),
        ];
        assert_eq!(
            found_in(&sessions, "prodcution"),
            [Entry::Session(0), Entry::Session(1)],
            "a swapped pair of letters still finds both"
        );
        assert_eq!(
            found_in(&sessions, "production")[0],
            Entry::Session(1),
            "the name that starts with it first"
        );
    }

    #[test]
    fn among_equal_fits_the_favourites_come_first_r_ssh_63() {
        let mut sessions = [session("Web one", "a", "u"), session("Web two", "b", "u")];
        sessions[1].favourite = true;
        assert_eq!(
            found_in(&sessions, "web"),
            [Entry::Session(1), Entry::Session(0)]
        );
    }

    #[test]
    fn a_click_in_a_row_chooses_what_the_row_stands_for_r_ssh_63_r_ssh_64() {
        use Clicked::{Connect, Second, Star};
        assert_eq!(Entry::Session(2).choice(Star), Some(Choice::Favourite(2)));
        assert_eq!(Entry::Session(2).choice(Second), Some(Choice::Edit(2)));
        assert_eq!(
            Entry::Host(1).choice(Star),
            None,
            "a config host has no star"
        );
        assert_eq!(Entry::Host(1).choice(Second), Some(Choice::ConfigSave(1)));
        assert_eq!(
            Entry::Recent(0, Some(3)).choice(Connect),
            Some(Choice::Recent(0)),
            "a recent saved session connects through the recent list"
        );
        assert_eq!(
            Entry::Recent(0, Some(3)).choice(Star),
            Some(Choice::Favourite(3))
        );
        assert_eq!(
            Entry::Recent(4, None).choice(Second),
            Some(Choice::RecentSave(4))
        );
        assert_eq!(Entry::Recent(4, None).choice(Star), None);
    }
}
