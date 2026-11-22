//! The command palette (R-SSH-48): one box (Ctrl+Shift+P, Cmd+Shift+P on macOS) that finds saved sessions, hosts of
//! `~/.ssh/config`, open tabs, recent connections, snippets and every menu action as one types, forgiving a typo;
//! the ones chosen lately first. Enter runs the one chosen.

mod items;

pub use items::{recent_saved, recent_source, remember_connection};

use egui::{Context, Key, RichText};
use parley_ui_egui::fuzzy::score;
use parley_ui_egui::{dialog, theme};

use crate::bars::Action;

/// How many chosen items the palette remembers, newest first.
pub const RECENT: usize = 20;
/// The most rows shown.
const SHOWN: usize = 12;

/// What an item does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    Action(Action),
    /// The saved session at this index.
    Session(usize),
    /// The host of `~/.ssh/config` at this index.
    ConfigHost(usize),
    /// The tab at this index.
    Tab(usize),
    /// The recent connection at this index.
    Recent(usize),
}

/// One row of the palette.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    /// Kept in the recent list: stays the same while the item does.
    pub key: String,
    /// What it is, e.g. "Session", "Action".
    pub kind: &'static str,
    pub label: String,
    /// Under or beside the label: an address, a command.
    pub detail: String,
    pub shortcut: String,
    pub choice: Choice,
}

/// The items that fit `query`, the recently chosen first, then the best fits, then as listed.
pub fn ranked<'a>(items: &'a [Item], query: &str, recent: &[String]) -> Vec<&'a Item> {
    let mut found: Vec<(usize, u32, usize, &Item)> = items
        .iter()
        .enumerate()
        .filter_map(|(i, item)| {
            // The label as `score` reads it; the detail and kind only when they hold the query.
            let text = format!("{} {}", item.detail, item.kind).to_lowercase();
            let holds = text
                .contains(query.trim().to_lowercase().as_str())
                .then_some(1500);
            let fit = score(query, &item.label).map(|s| s + 100).max(holds)?;
            let rank = recent
                .iter()
                .position(|k| *k == item.key)
                .unwrap_or(usize::MAX);
            Some((rank, fit, i, item))
        })
        .collect();
    found.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)).then(a.2.cmp(&b.2)));
    found.into_iter().map(|(.., item)| item).collect()
}

/// Puts `key` first in the recent list, keeping at most [`RECENT`].
pub fn remember(recent: &mut Vec<String>, key: &str) {
    recent.retain(|k| k != key);
    recent.insert(0, key.to_owned());
    recent.truncate(RECENT);
}

/// The palette while open.
#[derive(Debug, Default)]
pub struct Palette {
    query: String,
    selected: usize,
    focused: bool,
}

/// How the palette closed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Closed {
    Cancel,
    /// The item with this key, doing this.
    Chosen(String, Choice),
}

impl Palette {
    pub fn show(&mut self, ctx: &Context, items: &[Item], recent: &[String]) -> Option<Closed> {
        let found = ranked(items, &self.query, recent);
        let count = found.len().min(SHOWN);
        self.selected = self.selected.min(count.saturating_sub(1));
        let (down, up, enter) = ctx.input(|i| {
            (
                i.key_pressed(Key::ArrowDown),
                i.key_pressed(Key::ArrowUp),
                i.key_pressed(Key::Enter),
            )
        });
        if down && self.selected + 1 < count {
            self.selected += 1;
        }
        if up {
            self.selected = self.selected.saturating_sub(1);
        }
        let (close, clicked) = dialog::dialog(ctx, "ssh-palette", "Go to…", 560.0, |ui| {
            let search = ui
                .horizontal(|ui| {
                    let label = ui.label("Find");
                    let edit = egui::TextEdit::singleline(&mut self.query)
                        .hint_text("a session, host, tab, snippet or action");
                    theme::sized_edit(ui, 480.0, edit).labelled_by(label.id)
                })
                .inner;
            if search.changed() {
                self.selected = 0;
            }
            if !std::mem::replace(&mut self.focused, true) {
                search.request_focus();
            }
            ui.add_space(6.0);
            let mut clicked = None;
            if found.is_empty() {
                theme::hint(ui, "Nothing matches.");
            }
            for (row, item) in found.iter().take(SHOWN).enumerate() {
                ui.horizontal(|ui| {
                    ui.set_width(520.0);
                    let text = if item.detail.is_empty() {
                        item.label.clone()
                    } else {
                        format!("{}  —  {}", item.label, item.detail)
                    };
                    let response = ui.selectable_label(row == self.selected, text);
                    if response.clicked() {
                        clicked = Some(row);
                    }
                    theme::button_row(ui, |ui| {
                        if !item.shortcut.is_empty() {
                            theme::kbd(ui, &item.shortcut);
                        }
                        ui.label(RichText::new(item.kind).small().weak());
                    });
                });
            }
            if found.len() > SHOWN {
                theme::hint(
                    ui,
                    format!("{} more: type more to narrow.", found.len() - SHOWN),
                );
            }
            clicked
        });
        if close || ctx.input(|i| i.key_pressed(Key::Escape)) {
            return Some(Closed::Cancel);
        }
        let row = clicked.or(enter.then_some(self.selected))?;
        found
            .get(row)
            .map(|item| Closed::Chosen(item.key.clone(), item.choice))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(key: &str, kind: &'static str, label: &str, detail: &str) -> Item {
        Item {
            key: key.into(),
            kind,
            label: label.into(),
            detail: detail.into(),
            shortcut: String::new(),
            choice: Choice::Tab(0),
        }
    }

    #[test]
    fn recently_chosen_items_come_first_then_the_best_fits_r_ssh_48() {
        let items = [
            item("a:split", "Action", "Split right", ""),
            item("s:prod", "Session", "Prod", "me@prod.lan"),
            item("a:settings", "Action", "Settings", ""),
        ];
        let labels = |q: &str, recent: &[String]| {
            ranked(&items, q, recent)
                .iter()
                .map(|i| i.label.as_str())
                .collect::<Vec<_>>()
        };
        assert_eq!(labels("", &[]), ["Split right", "Prod", "Settings"]);
        assert_eq!(labels("set", &[]), ["Settings"]);
        assert_eq!(
            labels("", &["a:settings".into()]),
            ["Settings", "Split right", "Prod"]
        );
        assert_eq!(
            labels("prod.lan", &[]),
            ["Prod"],
            "the detail is searched too"
        );
        let mut recent = vec!["x".to_owned(); RECENT];
        remember(&mut recent, "a:split");
        remember(&mut recent, "x");
        assert_eq!(recent.len(), 2, "each key once");
        assert_eq!(recent[0], "x");
    }

    #[test]
    fn a_saved_session_stays_recent_by_name_and_connects_as_it_is_now_r_ssh_64() {
        use crate::sessions::SavedSession;
        let saved = SavedSession {
            name: "Prod æøå".into(),
            host: "box.lan".into(),
            user: "me".into(),
            ..SavedSession::default()
        };
        let quick = SavedSession {
            name: "typed".into(),
            host: "other.lan".into(),
            user: "me".into(),
            ..SavedSession::default()
        };
        let mut recent = Vec::new();
        remember_connection(&mut recent, &saved, true);
        remember_connection(&mut recent, &quick, false);
        assert_eq!(
            recent[0].name, "",
            "a connection not saved is kept without a name"
        );
        assert_eq!(recent[1].name, "Prod æøå");

        let mut sessions = vec![SavedSession {
            port: 2200,
            ..saved.clone()
        }];
        let (source, is_saved) = recent_source(&sessions, &recent, 1).expect("recent");
        assert!(is_saved);
        assert_eq!(source.port, 2200, "the saved session as it is now");

        sessions[0].name = "Renamed".into();
        let (source, is_saved) = recent_source(&sessions, &recent, 1).expect("recent");
        assert!(!is_saved, "renamed: the connection as it was made");
        assert_eq!((source.name.as_str(), source.port), ("", 22));

        let unnamed = SavedSession {
            name: String::new(),
            ..saved.clone()
        };
        sessions[0].port = 22;
        let (source, is_saved) = recent_source(&sessions, &[unnamed], 0).expect("recent");
        assert!(
            is_saved,
            "kept without a name: the saved session at its address"
        );
        assert_eq!(source.name, "Renamed");
        assert_eq!(recent_source(&sessions, &recent, 9), None);
    }
}
