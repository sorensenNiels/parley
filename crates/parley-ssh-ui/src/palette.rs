//! The command palette (R-SSH-48): one box (Ctrl+Shift+P, Cmd+Shift+P on macOS) that finds saved sessions, hosts of
//! `~/.ssh/config`, open tabs, recent connections, snippets and every menu action as one types, forgiving a typo;
//! the ones chosen lately first. Enter runs the one chosen.

mod items;

pub use items::remember_connection;

use egui::{Context, Key, RichText};
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

/// How well `query` fits `text`, higher better; `None` when it does not. Whole words first, then the letters in
/// order (`nwtb` finds "New tab"), then a word with one typo (two in a long query).
pub fn score(query: &str, text: &str) -> Option<u32> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return Some(0);
    }
    let text = text.to_lowercase();
    if let Some(at) = text.find(&query) {
        let at = u32::try_from(at).unwrap_or(u32::MAX).min(500);
        let starts_word = at == 0 || !text[..at as usize].ends_with(char::is_alphanumeric);
        return Some(3000 - at + if starts_word { 500 } else { 0 });
    }
    if let Some(gaps) = in_order(&query, &text) {
        return Some(2000_u32.saturating_sub(gaps));
    }
    let q: Vec<char> = query.chars().collect();
    if q.len() < 4 {
        return None;
    }
    let allowed = if q.len() >= 8 { 2 } else { 1 };
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .filter_map(|word| {
            let w: Vec<char> = word.chars().collect();
            // The query against the word's start of about the same length.
            let near = w.len().min(q.len() + 1);
            (q.len().saturating_sub(1)..=near)
                .filter(|n| *n <= w.len())
                .map(|n| distance(&q, &w[..n]))
                .min()
        })
        .min()
        .filter(|d| *d <= allowed)
        .map(|d| 1000 - u32::try_from(d).unwrap_or(1) * 100)
}

/// The letters of `query` in order in `text`: how many letters lie between them.
fn in_order(query: &str, text: &str) -> Option<u32> {
    let mut gaps = 0;
    let mut rest = text.chars();
    for q in query.chars().filter(|c| !c.is_whitespace()) {
        let mut skipped = 0;
        loop {
            match rest.next() {
                Some(c) if c == q => break,
                Some(_) => skipped += 1,
                None => return None,
            }
        }
        gaps += skipped;
    }
    Some(gaps)
}

/// Optimal string alignment distance: insertions, deletions, changes and two letters swapped.
fn distance(a: &[char], b: &[char]) -> usize {
    let mut rows = vec![vec![0; b.len() + 1]; a.len() + 1];
    for (i, row) in rows.iter_mut().enumerate() {
        row[0] = i;
    }
    for (j, cell) in rows[0].iter_mut().enumerate() {
        *cell = j;
    }
    for i in 1..=a.len() {
        for j in 1..=b.len() {
            let change = usize::from(a[i - 1] != b[j - 1]);
            let mut best = (rows[i - 1][j] + 1)
                .min(rows[i][j - 1] + 1)
                .min(rows[i - 1][j - 1] + change);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                best = best.min(rows[i - 2][j - 2] + 1);
            }
            rows[i][j] = best;
        }
    }
    rows[a.len()][b.len()]
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
    fn words_letters_in_order_and_one_typo_are_found_best_first_r_ssh_48() {
        assert!(score("new tab", "New tab") > score("tab", "New tab"));
        assert!(score("tab", "New tab").is_some());
        assert!(score("nwtb", "New tab").is_some(), "letters in order");
        assert!(
            score("prdo", "Production web").is_some(),
            "two letters swapped"
        );
        assert!(
            score("prodcution", "Production web").is_some(),
            "a long query, two typos"
        );
        assert!(score("xyz", "New tab").is_none());
        assert!(score("ab", "Bab").is_some());
        assert!(
            score("set", "Session").is_none(),
            "no typo in three letters"
        );
        assert!(score("æøå", "Server æøå").is_some());
        assert_eq!(score("  ", "anything"), Some(0));
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
}
