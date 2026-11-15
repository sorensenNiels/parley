//! The find bar (R-SQL-23): find, previous and next, match case, replace and replace all.

use std::ops::Range;

use egui::{Key, Modifiers, TextEdit, Ui};

use super::Scripts;
use crate::{icons, theme};

/// The find bar (R-SQL-23).
#[derive(Debug, Clone, Default)]
pub(super) struct Find {
    pub(super) query: String,
    pub(super) replacement: String,
    pub(super) match_case: bool,
    pub(super) replacing: bool,
    /// Give the query field the focus on the next frame.
    pub(super) focus: bool,
    /// Said after Replace all.
    pub(super) note: Option<String>,
}

impl Scripts {
    /// Opens the find bar with the focus in it, with Replace when `replacing` (R-SQL-23);
    /// a selection on one line is what it looks for.
    pub(super) fn open_find(&mut self, replacing: bool) {
        let mut bar = self.find.take().unwrap_or_default();
        bar.replacing = replacing || bar.replacing;
        bar.focus = true;
        if let Some(selected) = self.selected_text().filter(|s| !s.contains('\n')) {
            bar.query = selected;
        }
        self.find = Some(bar);
    }

    pub(super) fn find_bar(&mut self, ui: &mut Ui) {
        let Some(mut bar) = self.find.take() else {
            return;
        };
        let mut go: Option<bool> = None; // Some(forward)
        let mut replace_one = false;
        let mut replace_all = false;
        let mut close = false;
        theme::card(ui).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(icons::icon(icons::MAGNIFYING_GLASS).color(theme::palette(ui).muted));
                let field = theme::sized_edit(
                    ui,
                    220.0,
                    TextEdit::singleline(&mut bar.query).hint_text("Find"),
                );
                if std::mem::take(&mut bar.focus) {
                    field.request_focus();
                }
                // Taken, so it does not also reach the editor that gets the focus now.
                if field.lost_focus() {
                    let shift = ui.input(|i| i.modifiers.shift);
                    if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter)) {
                        go = Some(!shift);
                    }
                }
                if ui.small_button("Previous").clicked() {
                    go = Some(false);
                }
                if ui.small_button("Next").clicked() {
                    go = Some(true);
                }
                let switch = theme::switch(ui, &mut bar.match_case);
                let label = ui.label("Match case");
                switch.labelled_by(label.id);
                theme::button_row(ui, |ui| {
                    close = ui
                        .add(theme::icon_button(ui, "🗙"))
                        .on_hover_text("Close (Esc)")
                        .clicked();
                    if !bar.replacing && ui.small_button("Replace…").clicked() {
                        bar.replacing = true;
                    }
                });
            });
            if bar.replacing {
                ui.horizontal(|ui| {
                    ui.add_space(22.0);
                    theme::sized_edit(
                        ui,
                        220.0,
                        TextEdit::singleline(&mut bar.replacement).hint_text("Replace with"),
                    );
                    replace_one = ui.small_button("Replace").clicked();
                    replace_all = ui.small_button("Replace all").clicked();
                    if let Some(note) = &bar.note {
                        theme::hint(ui, note);
                    }
                });
            }
        });
        if replace_one {
            self.replace_selection(&bar);
            go = Some(true);
        }
        if replace_all && let Some(tab) = self.tabs.get_mut(self.shown) {
            let (text, count) =
                replace_everywhere(&tab.text, &bar.query, &bar.replacement, bar.match_case);
            tab.text = text;
            bar.note = Some(format!("{count} replaced"));
            self.generation += 1;
        }
        if let Some(forward) = go {
            self.go_to_match(&bar, forward);
        }
        if !close {
            self.find = Some(bar);
        }
    }

    /// Selects the next match after the selection, or the one before it.
    fn go_to_match(&mut self, bar: &Find, forward: bool) {
        let text = self.text();
        let (start, end) = self.selection;
        let found = if forward {
            find_next(text, &bar.query, end, bar.match_case)
                .or_else(|| find_next(text, &bar.query, 0, bar.match_case))
        } else {
            find_previous(text, &bar.query, start, bar.match_case)
                .or_else(|| find_previous(text, &bar.query, text.len(), bar.match_case))
        };
        if let Some(range) = found {
            self.select_range((range.start, range.end));
        }
    }

    /// Replace: the selection, when it is a match.
    fn replace_selection(&mut self, bar: &Find) {
        let (start, end) = self.selection;
        let Some(tab) = self.tabs.get_mut(self.shown) else {
            return;
        };
        let is_match = matches_at(&tab.text, start, &bar.query, bar.match_case) == Some(end);
        if start < end && is_match {
            tab.text.replace_range(start..end, &bar.replacement);
            self.selection = (start, start + bar.replacement.len());
            self.generation += 1;
        }
    }
}

/// The end of `query` if it matches `text` at byte `at`.
pub(super) fn matches_at(text: &str, at: usize, query: &str, match_case: bool) -> Option<usize> {
    if query.is_empty() {
        return None;
    }
    let rest = text.get(at..)?;
    let mut end = at;
    let mut chars = rest.chars();
    for q in query.chars() {
        let c = chars.next()?;
        let same = if match_case {
            c == q
        } else {
            c.to_lowercase().eq(q.to_lowercase())
        };
        if !same {
            return None;
        }
        end += c.len_utf8();
    }
    Some(end)
}

/// The first match of `query` at or after byte `from`.
pub(in crate::sql) fn find_next(
    text: &str,
    query: &str,
    from: usize,
    match_case: bool,
) -> Option<Range<usize>> {
    text.char_indices()
        .map(|(i, _)| i)
        .filter(|&i| i >= from)
        .find_map(|i| matches_at(text, i, query, match_case).map(|end| i..end))
}

/// The last match of `query` that ends at or before byte `before`.
pub(super) fn find_previous(
    text: &str,
    query: &str,
    before: usize,
    match_case: bool,
) -> Option<Range<usize>> {
    text.char_indices()
        .rev()
        .map(|(i, _)| i)
        .filter_map(|i| matches_at(text, i, query, match_case).map(|end| i..end))
        .find(|r| r.end <= before)
}

/// `text` with every match of `query` replaced, and how many.
pub(super) fn replace_everywhere(
    text: &str,
    query: &str,
    replacement: &str,
    match_case: bool,
) -> (String, usize) {
    let mut out = String::with_capacity(text.len());
    let mut count = 0;
    let mut at = 0;
    while at < text.len() {
        if let Some(end) = matches_at(text, at, query, match_case) {
            out.push_str(replacement);
            count += 1;
            at = end;
        } else if let Some(c) = text.get(at..).and_then(|r| r.chars().next()) {
            out.push(c);
            at += c.len_utf8();
        } else {
            break;
        }
    }
    (out, count)
}
