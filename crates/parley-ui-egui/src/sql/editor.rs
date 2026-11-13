//! The SQL window's editor (R-SQL-19..23): scripts in tabs, kept per saved session; SQL in
//! colour; open and save `.sql` files; format; find and replace.

use std::ops::Range;
use std::sync::Arc;

use egui::text::{CCursor, CCursorRange, LayoutJob, TextFormat};
use egui::{Color32, Key, Modifiers, ScrollArea, TextEdit, Ui};
use parley_core::sql::text::{self, Kind};
use serde::{Deserialize, Serialize};

use super::SqlFiles;
use crate::{dialog, icons, theme};

/// Text longer than this is not coloured while it is edited: it would be scanned every frame.
const MAX_COLOURED: usize = 200_000;

/// A script as the settings keep it between runs of Parley (R-SQL-20).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SavedScript {
    pub title: String,
    /// The file it was opened from or saved to.
    pub path: Option<String>,
    pub text: String,
    /// Changed since it was opened or saved.
    pub changed: bool,
}

/// One script in a tab.
#[derive(Debug, Clone)]
struct Script {
    title: String,
    path: Option<String>,
    text: String,
    /// The text when last opened or saved; `None` for a script never saved.
    saved: Option<String>,
    /// Stable for the tab's life, for its text field's id.
    number: u32,
}

impl Script {
    fn changed(&self) -> bool {
        self.saved
            .as_deref()
            .map_or(!self.text.is_empty(), |s| s != self.text)
    }
}

/// The find bar (R-SQL-23).
#[derive(Debug, Clone, Default)]
struct Find {
    query: String,
    replacement: String,
    match_case: bool,
    replacing: bool,
    /// Give the query field the focus on the next frame.
    focus: bool,
    /// Said after Replace all.
    note: Option<String>,
}

/// Something the editor asks the window to do with a file's outcome, or a question shown.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Closing {
    /// Asking whether to save the tab before it closes.
    Asking(usize),
}

/// The answer to "Save the script?".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Answer {
    Save,
    Discard,
    Cancel,
}

/// The scripts of a SQL window.
#[derive(Debug)]
pub struct Scripts {
    tabs: Vec<Script>,
    shown: usize,
    next_number: u32,
    find: Option<Find>,
    closing: Option<Closing>,
    /// The selection in the shown script last frame, as byte offsets.
    selection: (usize, usize),
    /// A selection to give the text field on the next frame, as character offsets.
    select_next: Option<(usize, usize)>,
    /// Counts changes, for the window to know when to keep the scripts.
    generation: u64,
    /// Said under the editor after Open, Save or a failure.
    pub note: Option<String>,
}

impl Scripts {
    /// The scripts kept for a session, or one empty script.
    pub fn restore(saved: &[SavedScript]) -> Self {
        let mut scripts = Self {
            tabs: Vec::new(),
            shown: 0,
            next_number: 1,
            find: None,
            closing: None,
            selection: (0, 0),
            select_next: None,
            generation: 0,
            note: None,
        };
        for s in saved {
            let number = scripts.take_number();
            scripts.tabs.push(Script {
                title: s.title.clone(),
                path: s.path.clone(),
                text: s.text.clone(),
                saved: (!s.changed).then(|| s.text.clone()),
                number,
            });
        }
        if scripts.tabs.is_empty() {
            scripts.new_tab();
        }
        scripts
    }

    /// The scripts as the settings keep them.
    pub fn to_saved(&self) -> Vec<SavedScript> {
        self.tabs
            .iter()
            .map(|t| SavedScript {
                title: t.title.clone(),
                path: t.path.clone(),
                text: t.text.clone(),
                changed: t.changed(),
            })
            .collect()
    }

    pub const fn generation(&self) -> u64 {
        self.generation
    }

    fn take_number(&mut self) -> u32 {
        let n = self.next_number;
        self.next_number += 1;
        n
    }

    fn new_tab(&mut self) {
        let number = self.take_number();
        self.tabs.push(Script {
            title: format!("Script {number}"),
            path: None,
            text: String::new(),
            saved: None,
            number,
        });
        self.shown = self.tabs.len() - 1;
        self.generation += 1;
    }

    /// The shown script's text.
    pub fn text(&self) -> &str {
        self.tabs.get(self.shown).map_or("", |t| t.text.as_str())
    }

    /// The selection in the shown script, as byte offsets (start, end).
    pub const fn selection(&self) -> (usize, usize) {
        self.selection
    }

    /// Puts `statement` at the end of the shown script, on a line of its own (the history).
    pub fn append(&mut self, statement: &str) {
        if let Some(tab) = self.tabs.get_mut(self.shown) {
            if !tab.text.trim().is_empty() {
                let end = tab.text.trim_end().len();
                tab.text.truncate(end);
                tab.text.push_str("\n\n");
            }
            tab.text.push_str(statement);
            self.generation += 1;
        }
    }

    /// The shown script's text and the cursor, as a test sets them.
    #[cfg(test)]
    pub fn set_text(&mut self, text: &str, cursor: usize) {
        if let Some(tab) = self.tabs.get_mut(self.shown) {
            text.clone_into(&mut tab.text);
        }
        self.selection = (cursor, cursor);
    }

    /// Whether the find bar takes Esc (the window's Cancel does otherwise).
    pub const fn finding(&self) -> bool {
        self.find.is_some()
    }

    /// The tabs, the actions and the editor, `height` high for the text.
    pub fn show(&mut self, ui: &mut Ui, height: f32, files: Option<&dyn SqlFiles>) {
        self.keys(ui, files);
        self.tab_strip(ui, files);
        if self.find.is_some() {
            self.find_bar(ui);
        }
        self.text_field(ui, height);
        if let Some(note) = self.note.as_deref().filter(|n| !n.is_empty()) {
            theme::hint(ui, note);
        }
        self.closing_dialog(ui.ctx(), files);
    }

    // ---------------------------------------------------------------- keys and actions

    fn keys(&mut self, ui: &Ui, files: Option<&dyn SqlFiles>) {
        let command = Modifiers::COMMAND;
        let shift = Modifiers::COMMAND | Modifiers::SHIFT;
        let (new, close, next, open, save, format, find, replace) = ui.input_mut(|i| {
            (
                i.consume_key(command, Key::T),
                i.consume_key(command, Key::W),
                i.consume_key(Modifiers::CTRL, Key::Tab),
                i.consume_key(command, Key::O),
                i.consume_key(command, Key::S),
                i.consume_key(shift, Key::F),
                i.consume_key(command, Key::F),
                i.consume_key(command, Key::H),
            )
        });
        if new {
            self.new_tab();
        }
        if close {
            self.close_tab(self.shown);
        }
        if next && !self.tabs.is_empty() {
            self.shown = (self.shown + 1) % self.tabs.len();
        }
        if open && let Some(files) = files {
            self.open(files);
        }
        if save && let Some(files) = files {
            self.save(files, false);
        }
        if format {
            self.format();
        }
        if find || replace {
            let mut bar = self.find.take().unwrap_or_default();
            bar.replacing = replace || bar.replacing;
            bar.focus = true;
            if let Some(selected) = self.selected_text().filter(|s| !s.contains('\n')) {
                bar.query = selected;
            }
            self.find = Some(bar);
        }
        if self.find.is_some() && ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            self.find = None;
        }
    }

    fn tab_strip(&mut self, ui: &mut Ui, files: Option<&dyn SqlFiles>) {
        let items: Vec<(usize, String, Option<String>)> = self
            .tabs
            .iter()
            .enumerate()
            .map(|(i, t)| (i, t.title.clone(), t.changed().then(|| "·".to_owned())))
            .collect();
        ui.horizontal(|ui| {
            let width = (ui.available_width() - 320.0).max(120.0);
            ui.allocate_ui(egui::vec2(width, 0.0), |ui| {
                ScrollArea::horizontal()
                    .id_salt("sql-script-tabs")
                    .show(ui, |ui| {
                        theme::tabs(ui, &mut self.shown, &items);
                    });
            });
            let new = icons::button(icons::PLUS, "").small();
            if icons::add_named(ui, true, new, "New script")
                .on_hover_text("New script (Ctrl+T)")
                .clicked()
            {
                self.new_tab();
            }
            if ui
                .add(theme::icon_button(ui, "🗙"))
                .on_hover_text("Close this script (Ctrl+W)")
                .clicked()
            {
                self.close_tab(self.shown);
            }
            theme::button_row(ui, |ui| {
                if let Some(files) = files {
                    if ui.small_button("Save as…").clicked() {
                        self.save(files, true);
                    }
                    if ui.small_button("Save").on_hover_text("Ctrl+S").clicked() {
                        self.save(files, false);
                    }
                    if ui.small_button("Open…").on_hover_text("Ctrl+O").clicked() {
                        self.open(files);
                    }
                }
                if ui
                    .small_button("Format")
                    .on_hover_text("Ctrl+Shift+F")
                    .clicked()
                {
                    self.format();
                }
            });
        });
    }

    /// Closes tab `index`, asking first when its changes are not saved; the last tab leaves an
    /// empty script.
    fn close_tab(&mut self, index: usize) {
        match self.tabs.get(index) {
            Some(tab) if tab.changed() => self.closing = Some(Closing::Asking(index)),
            Some(_) => self.remove(index),
            None => {}
        }
    }

    fn remove(&mut self, index: usize) {
        if index < self.tabs.len() {
            self.tabs.remove(index);
            if self.tabs.is_empty() {
                self.new_tab();
            }
            self.shown = self.shown.min(self.tabs.len() - 1);
            self.generation += 1;
        }
    }

    /// Open… (R-SQL-21): a file already open shows its tab.
    fn open(&mut self, files: &dyn SqlFiles) {
        match files.open_script() {
            Ok(Some((path, text))) => {
                if let Some(i) = self
                    .tabs
                    .iter()
                    .position(|t| t.path.as_deref() == Some(path.as_str()))
                {
                    self.shown = i;
                    return;
                }
                let number = self.take_number();
                // An empty, unchanged script gives way to the file.
                let reuse = self
                    .tabs
                    .get(self.shown)
                    .is_some_and(|t| t.text.is_empty() && t.path.is_none());
                let script = Script {
                    title: file_name(&path),
                    path: Some(path),
                    saved: Some(text.clone()),
                    text,
                    number,
                };
                if reuse && let Some(slot) = self.tabs.get_mut(self.shown) {
                    *slot = script;
                } else {
                    self.tabs.push(script);
                    self.shown = self.tabs.len() - 1;
                }
                self.note = None;
                self.generation += 1;
            }
            Ok(None) => {}
            Err(e) => self.note = Some(format!("Not opened: {e}")),
        }
    }

    /// Save (or Save as… with `ask`): the file the script has, or a new one.
    fn save(&mut self, files: &dyn SqlFiles, ask: bool) -> bool {
        let Some(tab) = self.tabs.get_mut(self.shown) else {
            return false;
        };
        let path = if ask { None } else { tab.path.as_deref() };
        let suggested = format!("{}.sql", tab.title.trim_end_matches(".sql"));
        match files.save_script(path, &suggested, &tab.text) {
            Ok(Some(saved)) => {
                tab.title = file_name(&saved);
                tab.path = Some(saved);
                tab.saved = Some(tab.text.clone());
                self.generation += 1;
                true
            }
            Ok(None) => false,
            Err(e) => {
                self.note = Some(format!("Not saved: {e}"));
                false
            }
        }
    }

    /// Format (R-SQL-22): the selection, or the statement under the cursor.
    fn format(&mut self) {
        let (start, end) = self.selection;
        let Some(tab) = self.tabs.get_mut(self.shown) else {
            return;
        };
        let range = if start < end {
            Some(start..end)
        } else {
            text::statement_at(&tab.text, start)
        };
        let Some(range) = range else {
            return;
        };
        let Some(original) = tab.text.get(range.clone()) else {
            return;
        };
        let formatted = text::format(original);
        if formatted != original {
            tab.text.replace_range(range, &formatted);
            self.generation += 1;
        }
    }

    fn selected_text(&self) -> Option<String> {
        let (start, end) = self.selection;
        (start < end)
            .then(|| self.text().get(start..end).map(str::to_owned))
            .flatten()
    }

    // ---------------------------------------------------------------- the text

    fn text_field(&mut self, ui: &mut Ui, height: f32) {
        let Some(tab) = self.tabs.get_mut(self.shown) else {
            return;
        };
        let id = ui.make_persistent_id(("sql-script", tab.number));
        if let Some((start, end)) = self.select_next.take()
            && let Some(mut state) = TextEdit::load_state(ui.ctx(), id)
        {
            state.cursor.set_char_range(Some(CCursorRange::two(
                CCursor::new(start),
                CCursor::new(end),
            )));
            state.store(ui.ctx(), id);
            ui.memory_mut(|m| m.request_focus(id));
        }
        let mut layouter =
            |ui: &Ui, buffer: &dyn egui::TextBuffer, wrap_width: f32| -> Arc<egui::Galley> {
                let job = coloured(ui, buffer.as_str(), wrap_width);
                ui.fonts_mut(|f| f.layout_job(job))
            };
        let before = tab.text.len();
        let mut changed = false;
        ScrollArea::vertical()
            .id_salt(("sql-editor", tab.number))
            .max_height(height)
            .min_scrolled_height(height)
            .show(ui, |ui| {
                let output = TextEdit::multiline(&mut tab.text)
                    .id(id)
                    .code_editor()
                    .layouter(&mut layouter)
                    .desired_width(f32::INFINITY)
                    .desired_rows(8)
                    .hint_text("SELECT * FROM QSYS2.SYSTABLES FETCH FIRST 10 ROWS ONLY")
                    .show(ui);
                changed = output.response.changed();
                if let Some(range) = output.cursor_range {
                    let (a, b) = (range.primary.index.0, range.secondary.index.0);
                    self.selection = (byte_at(&tab.text, a.min(b)), byte_at(&tab.text, a.max(b)));
                }
            });
        if changed || tab.text.len() != before {
            self.generation += 1;
        }
    }

    // ---------------------------------------------------------------- find and replace

    fn find_bar(&mut self, ui: &mut Ui) {
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
                if field.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                    go = Some(!ui.input(|i| i.modifiers.shift));
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
            let chars = (char_at(text, range.start), char_at(text, range.end));
            self.selection = (range.start, range.end);
            self.select_next = Some(chars);
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

    // ---------------------------------------------------------------- closing a changed tab

    fn closing_dialog(&mut self, ctx: &egui::Context, files: Option<&dyn SqlFiles>) {
        let Some(Closing::Asking(index)) = self.closing.clone() else {
            return;
        };
        let title = self
            .tabs
            .get(index)
            .map_or_else(String::new, |t| t.title.clone());
        let (close, choice) =
            dialog::dialog(ctx, "sql-close-script", "Save the script?", 400.0, |ui| {
                ui.label(format!("{title} has changes that are not saved."));
                ui.add_space(12.0);
                theme::button_row(ui, |ui| {
                    if files.is_some() && ui.add(theme::primary_button(ui, "Save")).clicked() {
                        Some(Answer::Save)
                    } else if ui.button("Don't save").clicked() {
                        Some(Answer::Discard)
                    } else if ui.button("Cancel").clicked() {
                        Some(Answer::Cancel)
                    } else {
                        None
                    }
                })
            });
        match choice {
            Some(Answer::Save) => {
                self.shown = index;
                if let Some(files) = files
                    && self.save(files, false)
                {
                    self.remove(index);
                }
                self.closing = None;
            }
            Some(Answer::Discard) => {
                self.remove(index);
                self.closing = None;
            }
            Some(Answer::Cancel) => self.closing = None,
            None if close => self.closing = None,
            None => {}
        }
    }
}

/// `text` laid out for the editor, in colour unless it is very long (R-SQL-19), as Tortuga's
/// code colours.
fn coloured(ui: &Ui, text: &str, wrap_width: f32) -> LayoutJob {
    let p = theme::palette(ui);
    let plain = ui.visuals().text_color();
    let font = egui::TextStyle::Monospace.resolve(ui.style());
    let format = |color: Color32| TextFormat::simple(font.clone(), color);
    let spans = if text.len() <= MAX_COLOURED {
        text::highlight(text)
    } else {
        Vec::new()
    };
    let mut job = LayoutJob::default();
    let mut at = 0;
    for (range, kind) in spans {
        let (Some(before), Some(part)) = (text.get(at..range.start), text.get(range.clone()))
        else {
            continue;
        };
        job.append(before, 0.0, format(plain));
        let color = match kind {
            Kind::Keyword => p.blue,
            Kind::String => p.ok,
            Kind::Name => p.purple,
            Kind::Number => p.redirect,
            Kind::Comment => p.muted,
        };
        job.append(part, 0.0, format(color));
        at = range.end;
    }
    job.append(text.get(at..).unwrap_or_default(), 0.0, format(plain));
    job.wrap.max_width = wrap_width;
    job
}

/// The end of `query` if it matches `text` at byte `at`.
fn matches_at(text: &str, at: usize, query: &str, match_case: bool) -> Option<usize> {
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
fn find_next(text: &str, query: &str, from: usize, match_case: bool) -> Option<Range<usize>> {
    text.char_indices()
        .map(|(i, _)| i)
        .filter(|&i| i >= from)
        .find_map(|i| matches_at(text, i, query, match_case).map(|end| i..end))
}

/// The last match of `query` that ends at or before byte `before`.
fn find_previous(text: &str, query: &str, before: usize, match_case: bool) -> Option<Range<usize>> {
    text.char_indices()
        .rev()
        .map(|(i, _)| i)
        .filter_map(|i| matches_at(text, i, query, match_case).map(|end| i..end))
        .find(|r| r.end <= before)
}

/// `text` with every match of `query` replaced, and how many.
fn replace_everywhere(
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

/// A file's name from its path, for a tab.
fn file_name(path: &str) -> String {
    path.rsplit(['/', '\\']).next().unwrap_or(path).to_owned()
}

/// The byte offset of character `index` in `text` (egui counts characters).
pub fn byte_at(text: &str, index: usize) -> usize {
    text.char_indices()
        .nth(index)
        .map_or(text.len(), |(byte, _)| byte)
}

/// The character index at byte `byte` of `text`.
fn char_at(text: &str, byte: usize) -> usize {
    text.get(..byte).map_or(0, |s| s.chars().count())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_ignores_case_also_for_danish_letters_unless_asked_r_sql_23() {
        let text = "select æøå from T; SELECT ÆØÅ from t";
        // "æøå" is 6 bytes: the second "ÆØÅ" starts at byte 29, the second SELECT at 22.
        assert_eq!(find_next(text, "ÆØÅ", 0, false), Some(7..13));
        assert_eq!(find_next(text, "ÆØÅ", 0, true), Some(29..35));
        assert_eq!(find_next(text, "æøå", 14, false), Some(29..35));
        assert_eq!(
            find_previous(text, "select", text.len(), false),
            Some(22..28)
        );
        assert_eq!(find_next(text, "", 0, false), None);
        assert_eq!(find_next("", "x", 0, false), None);
    }

    #[test]
    fn replace_all_counts_and_keeps_the_rest_r_sql_23() {
        let (text, count) = replace_everywhere("a.æ, A.æ, b.æ", "a.", "x.", false);
        assert_eq!(text, "x.æ, x.æ, b.æ");
        assert_eq!(count, 2);
        let (text, count) = replace_everywhere("aaa", "aa", "b", true);
        assert_eq!((text.as_str(), count), ("ba", 1), "matches do not overlap");
    }

    #[test]
    fn scripts_are_kept_with_whether_they_changed_and_come_back_r_sql_20() {
        let mut scripts = Scripts::restore(&[]);
        assert_eq!(scripts.tabs.len(), 1, "one empty script to start");
        scripts.tabs[0].text = "VALUES 'æøå'".to_owned();
        let saved = scripts.to_saved();
        assert_eq!(saved[0].title, "Script 1");
        assert!(saved[0].changed);
        let back = Scripts::restore(&saved);
        assert_eq!(back.text(), "VALUES 'æøå'");
        assert!(back.tabs[0].changed(), "still not saved to a file");
        let file = SavedScript {
            title: "jobs.sql".to_owned(),
            path: Some("/home/u/jobs.sql".to_owned()),
            text: "VALUES 1".to_owned(),
            changed: false,
        };
        assert!(!Scripts::restore(&[file]).tabs[0].changed());
    }

    #[test]
    fn closing_an_unchanged_tab_closes_it_and_the_last_leaves_an_empty_script_r_sql_20() {
        let mut scripts = Scripts::restore(&[]);
        scripts.new_tab();
        assert_eq!(scripts.tabs.len(), 2);
        scripts.close_tab(1);
        assert_eq!(scripts.tabs.len(), 1);
        scripts.close_tab(0);
        assert_eq!(scripts.tabs.len(), 1, "an empty script stays");
        assert_eq!(scripts.tabs[0].title, "Script 3");
    }

    #[test]
    fn closing_a_changed_tab_asks_first_r_sql_20() {
        let mut scripts = Scripts::restore(&[]);
        scripts.tabs[0].text = "VALUES 1".to_owned();
        scripts.close_tab(0);
        assert_eq!(scripts.closing, Some(Closing::Asking(0)));
        assert_eq!(scripts.tabs.len(), 1, "not closed yet");
    }

    #[test]
    fn format_rewrites_the_statement_under_the_cursor_only_r_sql_22() {
        let mut scripts = Scripts::restore(&[]);
        scripts.tabs[0].text = "values 1;\nselect a from t where b = 'x'".to_owned();
        scripts.selection = (12, 12);
        scripts.format();
        assert_eq!(
            scripts.text(),
            "values 1;\nSELECT\n    a\nFROM\n    t\nWHERE\n    b = 'x'"
        );
    }

    #[test]
    fn a_statement_from_the_history_goes_on_its_own_line_r_sql_7() {
        let mut scripts = Scripts::restore(&[]);
        scripts.append("VALUES 1");
        scripts.append("VALUES 2");
        assert_eq!(scripts.text(), "VALUES 1\n\nVALUES 2");
    }

    #[test]
    fn a_file_name_is_the_end_of_its_path_on_any_system_r_sql_21() {
        assert_eq!(file_name("/home/u/jobs.sql"), "jobs.sql");
        assert_eq!(file_name("C:\\Users\\u\\æøå.sql"), "æøå.sql");
        assert_eq!(file_name("plain.sql"), "plain.sql");
    }
}
