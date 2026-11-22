//! The start screen while not connected (D64): Parley's saved sessions (R-UI-5, R-UI-40..42) and Parley SQL's saved
//! connections (R-SQL-46, R-SQL-105), as Parley SSH lists its own (R-SSH-63..65): the favourites first, the recent
//! connections, a search at the top that the keyboard drives, "All sessions" folding, Parley's session groups, and
//! quick connect beside the list.

pub mod list;

use egui::Ui;

use crate::profile::Profile;
use crate::{fuzzy, theme};
use list::{Clicked, Fold, Row, Star};

/// From this many saved sessions on, the list has a search field (R-UI-42).
const SEARCH_FROM: usize = 4;
/// Quick connect's host field, for screen readers and its tooltip.
pub const QUICK_HOST: &str = "Quick connect host";
/// How many recent connections are kept and listed (R-UI-41).
pub const RECENT_KEPT: usize = 5;

/// What the start screen calls the things it lists, and what it offers.
#[derive(Debug, Clone, Copy)]
pub struct Words {
    /// Where egui keeps the screen's own state: different for each program.
    pub id: &'static str,
    /// The screen's heading: "Connect to IBM i".
    pub heading: &'static str,
    /// Said when there are none: "No sessions yet."
    pub none: &'static str,
    /// The button that adds one: "New session…"
    pub new: &'static str,
    /// The heading over every saved one: "All sessions".
    pub all: &'static str,
    /// The search field's hint: "Find a session".
    pub find: &'static str,
    /// Said when the search finds nothing.
    pub no_match: &'static str,
    /// Keeps a recent connection that is not saved: "Save as session…".
    pub save_as: &'static str,
    /// Quick connect takes `host:port` (Parley); Parley SQL's helper finds the port itself.
    pub port: bool,
}

/// Parley's words: saved sessions.
pub const SESSIONS: Words = Words {
    id: "parley-start",
    heading: "Connect to IBM i",
    none: "No sessions yet.",
    new: "New session…",
    all: "All sessions",
    find: "Find a session",
    no_match: "No saved session matches.",
    save_as: "Save as session…",
    port: true,
};

/// One saved session or connection as the screen shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Saved {
    pub name: String,
    /// The line beside the name: `host:port · TLS`.
    pub address: String,
    pub favourite: bool,
}

impl Saved {
    /// A saved session: its name, host:port and TLS.
    pub fn session(profile: &Profile) -> Self {
        let config = &profile.config;
        Self {
            name: profile.name.clone(),
            address: address(&config.host, Some(config.port), config.tls),
            favourite: profile.favourite,
        }
    }
}

/// The line beside a name: `host:port · TLS`, without the port when there is none.
pub fn address(host: &str, port: Option<u16>, tls: bool) -> String {
    let port = port.map_or_else(String::new, |p| format!(":{p}"));
    let tls = if tls { "  ·  TLS" } else { "" };
    format!("{host}{port}{tls}")
}

/// One recent connection (R-UI-41).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recent {
    /// Shown when it is not a saved one.
    pub address: String,
    /// The saved one it was made with, while that is still saved.
    pub saved: Option<usize>,
}

/// One of Parley's session groups (R-UI-31).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    pub name: String,
    /// Its sessions' names, as one line.
    pub sessions: String,
}

/// What the lists are made from.
#[derive(Debug, Clone, Copy)]
pub struct Lists<'a> {
    pub saved: &'a [Saved],
    /// Newest first.
    pub recent: &'a [Recent],
    /// Parley's session groups; `None` where there are none to have (Parley SQL).
    pub groups: Option<&'a [Group]>,
    /// "All sessions" is open (R-UI-42).
    pub all_open: bool,
    /// Why the last connection ended.
    pub last_error: Option<&'a str>,
}

/// What the user chose on the start screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Choice {
    /// The saved one at this index.
    Connect(usize),
    Edit(usize),
    New,
    /// Mark the saved one at this index as a favourite, or no longer (R-UI-40).
    Favourite(usize),
    /// The recent connection at this index (R-UI-41).
    Recent(usize),
    /// Keep the recent connection at this index, not a saved one, as one.
    RecentSave(usize),
    /// Quick connect (R-UI-41).
    Quick(Quick),
    /// Open or fold "All sessions", to be kept in the settings (R-UI-42).
    AllOpen(bool),
    /// Start the group at this index (R-UI-32).
    StartGroup(usize),
    /// Edit the group at this index, or the groups (R-UI-31).
    EditGroup(Option<usize>),
}

/// A connection quick connect asks for: not saved (R-UI-41).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Quick {
    pub host: String,
    /// The port typed with the host; `None` for TLS's or telnet's own.
    pub port: Option<u16>,
    pub tls: bool,
}

/// What one row of the start screen stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Entry {
    Saved(usize),
    /// A recent connection, and the saved one it was made with while that is still saved.
    Recent(usize, Option<usize>),
    Group(usize),
}

impl Entry {
    /// What a click in its row chooses.
    fn choice(self, clicked: Clicked) -> Option<Choice> {
        match (self, clicked) {
            (Self::Saved(i), Clicked::Connect) => Some(Choice::Connect(i)),
            (Self::Saved(i) | Self::Recent(_, Some(i)), Clicked::Second) => Some(Choice::Edit(i)),
            (Self::Saved(i) | Self::Recent(_, Some(i)), Clicked::Star) => {
                Some(Choice::Favourite(i))
            }
            (Self::Recent(r, _), Clicked::Connect) => Some(Choice::Recent(r)),
            (Self::Recent(r, None), Clicked::Second) => Some(Choice::RecentSave(r)),
            (Self::Group(g), Clicked::Connect) => Some(Choice::StartGroup(g)),
            (Self::Group(g), Clicked::Second) => Some(Choice::EditGroup(Some(g))),
            (Self::Recent(_, None) | Self::Group(_), Clicked::Star) => None,
        }
    }
}

impl Lists<'_> {
    /// The line that shows `entry`, chosen or not.
    fn row(&self, words: Words, entry: Entry, selected: bool) -> Option<Row> {
        Some(match entry {
            Entry::Saved(i) | Entry::Recent(_, Some(i)) => {
                let saved = self.saved.get(i)?;
                Row {
                    name: saved.name.clone(),
                    address: saved.address.clone(),
                    star: if saved.favourite { Star::On } else { Star::Off },
                    primary: "Connect",
                    second: "Edit…",
                    selected,
                }
            }
            Entry::Recent(r, None) => Row {
                name: self.recent.get(r)?.address.clone(),
                address: "Not saved".to_owned(),
                star: Star::None,
                primary: "Connect",
                second: words.save_as,
                selected,
            },
            Entry::Group(g) => {
                let group = self.groups?.get(g)?;
                Row {
                    name: group.name.clone(),
                    address: group.sessions.clone(),
                    star: Star::None,
                    primary: "Start",
                    second: "Edit…",
                    selected,
                }
            }
        })
    }

    /// The rows of `entries`, the `selected`-th marked; what was clicked.
    fn rows(
        &self,
        ui: &mut Ui,
        words: Words,
        entries: &[Entry],
        selected: Option<usize>,
        scroll: bool,
    ) -> Option<Choice> {
        let mut choice = None;
        for (n, &entry) in entries.iter().enumerate() {
            let Some(row) = self.row(words, entry, selected == Some(n)) else {
                continue;
            };
            if let Some(clicked) = list::row(ui, &row, scroll) {
                choice = entry.choice(clicked);
            }
            ui.add_space(2.0);
        }
        choice
    }
}

/// The start screen's fields, as typed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Form {
    /// Quick connect's host, `host:port` for another port.
    pub host: String,
    pub tls: bool,
    /// What is wrong with what quick connect was given.
    pub error: Option<String>,
    search: String,
    /// The match chosen with Up and Down (R-UI-42).
    selected: usize,
    /// The chosen match is to be scrolled into view.
    scroll: bool,
    /// The search is to get the keyboard.
    focus: bool,
}

impl Default for Form {
    /// TLS on, as R-P-3 asks of new sessions; the search gets the keyboard.
    fn default() -> Self {
        Self {
            host: String::new(),
            tls: true,
            error: None,
            search: String::new(),
            selected: 0,
            scroll: false,
            focus: true,
        }
    }
}

impl Form {
    /// The search gets the keyboard the next time the screen shows: when it shows again after a session.
    pub fn focus_search(&mut self) {
        self.focus = true;
    }

    /// The start screen.
    pub fn show(&mut self, ui: &mut Ui, words: Words, lists: Lists<'_>) -> Option<Choice> {
        egui::ScrollArea::vertical()
            .id_salt(words.id)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(ui.available_height() * 0.06);
                    ui.heading(words.heading);
                    ui.add_space(12.0);
                    if let Some(error) = lists.last_error {
                        theme::error(ui, error);
                        ui.add_space(8.0);
                    }
                });
                let choice = list::columns(
                    ui,
                    self,
                    |form, ui| form.lists(ui, words, lists),
                    |form, ui| form.side(ui, words, lists),
                );
                ui.add_space(24.0);
                choice
            })
            .inner
    }

    /// Quick connect, then New session… and, in Parley, Session groups….
    fn side(&mut self, ui: &mut Ui, words: Words, lists: Lists<'_>) -> Option<Choice> {
        let mut choice = self.quick(ui, words);
        ui.add_space(12.0);
        let width = ui.available_width();
        if ui
            .add(egui::Button::new(words.new).min_size(egui::vec2(width, 34.0)))
            .clicked()
        {
            choice = Some(Choice::New);
        }
        if lists.groups.is_some() {
            ui.add_space(4.0);
            ui.vertical_centered(|ui| {
                if ui.add(theme::ghost_button("Session groups…")).clicked() {
                    choice = Some(Choice::EditGroup(None));
                }
            });
        }
        choice
    }

    /// The search, then its matches or, without one, the sections.
    fn lists(&mut self, ui: &mut Ui, words: Words, lists: Lists<'_>) -> Option<Choice> {
        let mut choice = None;
        if lists.saved.len() >= SEARCH_FROM {
            choice = self.search(ui, words, lists);
            ui.add_space(12.0);
        } else {
            self.focus = false;
        }
        let shown = if self.search.trim().is_empty() {
            sections(ui, words, lists)
        } else {
            let found = found(lists, &self.search);
            if found.is_empty() {
                ui.vertical_centered(|ui| theme::hint(ui, words.no_match));
            }
            let shown = lists.rows(ui, words, &found, Some(self.selected), self.scroll);
            self.scroll = false;
            shown
        };
        shown.or(choice)
    }

    /// The search field: Up and Down choose a match, Enter connects it, Escape empties it (R-UI-42).
    fn search(&mut self, ui: &mut Ui, words: Words, lists: Lists<'_>) -> Option<Choice> {
        let id = format!("{}-search", words.id);
        let keys = list::search_field(ui, &id, &mut self.search, words.find, &mut self.focus);
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

    /// Quick connect: a host and TLS, not saved (R-UI-41).
    fn quick(&mut self, ui: &mut Ui, words: Words) -> Option<Choice> {
        let mut go = false;
        theme::card(ui).show(ui, |ui| {
            ui.set_width(ui.available_width());
            theme::section_label(ui, "Quick connect");
            ui.add_space(4.0);
            let hint = if words.port {
                "host, or host:port"
            } else {
                "host name or address"
            };
            let edit = egui::TextEdit::singleline(&mut self.host).hint_text(hint);
            // Named apart from the session dialog's Host for screen readers.
            let response =
                theme::sized_edit(ui, ui.available_width(), edit).on_hover_text(QUICK_HOST);
            response
                .widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, QUICK_HOST));
            go |= response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            ui.add_space(4.0);
            theme::switch_row(ui, "Use TLS", &mut self.tls);
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
        match parse(&self.host, self.tls, words.port) {
            Ok(quick) => {
                self.error = None;
                Some(Choice::Quick(quick))
            }
            Err(e) => {
                self.error = Some(e);
                None
            }
        }
    }
}

/// Without a search: the favourites, the recent connections, every saved one, and Parley's groups (R-UI-42).
fn sections(ui: &mut Ui, words: Words, lists: Lists<'_>) -> Option<Choice> {
    let mut choice = None;
    let mut pick = |c: Option<Choice>| {
        if c.is_some() {
            choice = c;
        }
    };
    // Every saved one is under All sessions, the favourites too (R-UI-40).
    let all: Vec<Entry> = (0..lists.saved.len()).map(Entry::Saved).collect();
    let favourites: Vec<Entry> = lists
        .saved
        .iter()
        .enumerate()
        .filter(|(_, s)| s.favourite)
        .map(|(i, _)| Entry::Saved(i))
        .collect();
    let recent: Vec<Entry> = lists
        .recent
        .iter()
        .take(RECENT_KEPT)
        .enumerate()
        .map(|(r, recent)| Entry::Recent(r, recent.saved))
        .collect();
    if lists.saved.is_empty() {
        ui.vertical_centered(|ui| theme::hint(ui, words.none));
        ui.add_space(8.0);
    }
    let mut above = false;
    for (label, entries) in [("Favourites", &favourites), ("Recent", &recent)] {
        if entries.is_empty() {
            continue;
        }
        theme::section_label(ui, label);
        pick(lists.rows(ui, words, entries, None, false));
        ui.add_space(12.0);
        above = true;
    }
    if !all.is_empty() {
        let id = format!("{}-all", words.id);
        let fold = Fold::Kept(list::all_open(lists.all_open, above));
        let (shown, clicked) = list::section(ui, &id, words.all, all.len(), fold, |ui| {
            lists.rows(ui, words, &all, None, false)
        });
        pick(shown.flatten());
        if clicked {
            pick(Some(Choice::AllOpen(!list::all_open(
                lists.all_open,
                above,
            ))));
        }
        ui.add_space(12.0);
    }
    if let Some(groups) = lists.groups.filter(|g| !g.is_empty()) {
        let id = format!("{}-groups", words.id);
        let entries: Vec<Entry> = (0..groups.len()).map(Entry::Group).collect();
        let fold = Fold::Starts(true);
        let (shown, _) = list::section(ui, &id, "Groups", groups.len(), fold, |ui| {
            lists.rows(ui, words, &entries, None, false)
        });
        pick(shown.flatten());
    }
    choice
}

/// The saved ones that fit `search`, the best fits first, then the favourites, then as listed (R-UI-42).
fn found(lists: Lists<'_>, search: &str) -> Vec<Entry> {
    let mut found: Vec<(u32, bool, Entry)> = lists
        .saved
        .iter()
        .enumerate()
        .filter_map(|(i, s)| {
            let fit = fuzzy::fit(search, &s.name, &[s.address.as_str()])?;
            Some((fit, !s.favourite, Entry::Saved(i)))
        })
        .collect();
    // Stable: equal fits keep the order they are listed in.
    found.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    found.into_iter().map(|(.., entry)| entry).collect()
}

/// Quick connect's host as a [`Quick`]: `host`, `host:port` when `with_port`, `[::1]:992` for an IPv6 address with a
/// port (R-UI-41).
pub fn parse(host: &str, tls: bool, with_port: bool) -> Result<Quick, String> {
    let typed = host.trim();
    let (name, port) = if let Some(rest) = typed.strip_prefix('[') {
        let (address, after) = rest
            .split_once(']')
            .ok_or("An IPv6 address in brackets needs its ]")?;
        (address, after.strip_prefix(':'))
    } else if let Some((name, port)) = typed.split_once(':')
        && !port.contains(':')
    {
        (name, Some(port))
    } else {
        (typed, None)
    };
    let name = name.trim();
    if name.is_empty() || name.contains(char::is_whitespace) {
        return Err("Name the host to connect to.".to_owned());
    }
    let port = match port.map(str::trim) {
        None => None,
        Some(_) if !with_port => {
            return Err(
                "Name the host without a port: Parley SQL finds the port itself.".to_owned(),
            );
        }
        Some(p) => match p.parse::<u16>() {
            Ok(p) if p > 0 => Some(p),
            _ => return Err(format!("\"{p}\" is not a port: a number from 1 to 65535.")),
        },
    };
    Ok(Quick {
        host: name.to_owned(),
        port,
        tls,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn saved(name: &str, address: &str, favourite: bool) -> Saved {
        Saved {
            name: name.to_owned(),
            address: address.to_owned(),
            favourite,
        }
    }

    fn names(lists: Lists<'_>, search: &str) -> Vec<String> {
        found(lists, search)
            .into_iter()
            .map(|e| match e {
                Entry::Saved(i) => lists.saved[i].name.clone(),
                Entry::Recent(..) | Entry::Group(_) => {
                    unreachable!("the search finds saved ones only")
                }
            })
            .collect()
    }

    fn lists<'a>(saved: &'a [Saved], recent: &'a [Recent]) -> Lists<'a> {
        Lists {
            saved,
            recent,
            groups: None,
            all_open: false,
            last_error: None,
        }
    }

    #[test]
    fn the_search_looks_in_names_and_addresses_forgives_a_typo_and_puts_favourites_first_r_ui_42() {
        let all = [
            saved("Test", "test.lan:992  ·  TLS", false),
            saved("Produktion", "prod.lan:992  ·  TLS", false),
            saved("Prod old", "10.0.0.5:23", true),
            saved("Æøå", "æøå.dk:23", false),
        ];
        let l = lists(&all, &[]);
        let typo = names(l, "prdo");
        assert_eq!(typo.len(), 2, "a typo: {typo:?}");
        assert_eq!(names(l, "10.0.0"), ["Prod old"]);
        assert_eq!(names(l, "test.lan"), ["Test"]);
        assert_eq!(names(l, "æøå"), ["Æøå"]);
        assert_eq!(names(l, "xyz"), Vec::<String>::new());
    }

    #[test]
    fn equal_fits_list_the_favourites_first_r_ui_42() {
        let all = [
            saved("Prod a", "a.lan:23", false),
            saved("Prod b", "b.lan:23", true),
        ];
        assert_eq!(names(lists(&all, &[]), "prod"), ["Prod b", "Prod a"]);
    }

    #[test]
    fn a_click_in_each_kind_of_row_chooses_what_it_says_r_ui_40_r_ui_41() {
        let star = Entry::Saved(2).choice(Clicked::Star);
        assert_eq!(star, Some(Choice::Favourite(2)));
        let recent = Entry::Recent(0, Some(3));
        assert_eq!(recent.choice(Clicked::Connect), Some(Choice::Recent(0)));
        assert_eq!(recent.choice(Clicked::Second), Some(Choice::Edit(3)));
        let unsaved = Entry::Recent(1, None);
        assert_eq!(unsaved.choice(Clicked::Second), Some(Choice::RecentSave(1)));
        assert_eq!(unsaved.choice(Clicked::Star), None);
        let group = Entry::Group(1);
        assert_eq!(group.choice(Clicked::Connect), Some(Choice::StartGroup(1)));
        assert_eq!(
            group.choice(Clicked::Second),
            Some(Choice::EditGroup(Some(1)))
        );
    }

    #[test]
    fn quick_connect_takes_a_host_a_port_and_an_ipv6_address_r_ui_41() {
        let quick = |host: &str, port, tls| {
            Ok(Quick {
                host: host.to_owned(),
                port,
                tls,
            })
        };
        assert_eq!(
            parse(" prod.lan ", true, true),
            quick("prod.lan", None, true)
        );
        assert_eq!(
            parse("prod.lan:2323", false, true),
            quick("prod.lan", Some(2323), false)
        );
        assert_eq!(
            parse("[::1]:992", true, true),
            quick("::1", Some(992), true)
        );
        assert_eq!(parse("fe80::1", true, true), quick("fe80::1", None, true));
        assert_eq!(parse("æøå.dk", true, true), quick("æøå.dk", None, true));
        assert!(parse("", true, true).is_err());
        assert!(parse("prod.lan:", true, true).is_err());
        assert!(parse("prod.lan:99999", true, true).is_err());
        assert!(parse("[::1", true, true).is_err());
        assert!(
            parse("prod.lan:8471", true, false).is_err(),
            "Parley SQL takes no port"
        );
        assert_eq!(
            parse("prod.lan", true, false),
            quick("prod.lan", None, true)
        );
    }
}
