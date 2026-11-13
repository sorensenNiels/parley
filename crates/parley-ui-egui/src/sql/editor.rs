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

mod complete;
pub use complete::{Item, MAX_ITEMS};

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
    /// The completion list while a name is typed (R-SQL-36).
    popup: Option<complete::Popup>,
    /// The name under the pointer and where it is (R-SQL-37).
    pointed: Option<(Name, usize)>,
    /// What the window said about a name, shown while the pointer is on it.
    described: Option<(Name, String)>,
}

/// A name in the text, and the name before a `.` in front of it.
pub type Name = (Option<String>, String);

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
            popup: None,
            pointed: None,
            described: None,
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

    /// A new script tab named `title` holding `text`, shown (R-SQL-35).
    pub fn open_new(&mut self, title: &str, text: &str) {
        self.new_tab();
        if let Some(tab) = self.tabs.last_mut() {
            title.clone_into(&mut tab.title);
            text.clone_into(&mut tab.text);
        }
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

    /// Puts `text` in the shown script in place of the selection, or at the cursor, and the
    /// cursor after it (a snippet, R-SQL-25).
    pub fn insert(&mut self, text: &str) {
        let (start, end) = self.selection;
        let Some(tab) = self.tabs.get_mut(self.shown) else {
            return;
        };
        let start = start.min(tab.text.len());
        let end = end.clamp(start, tab.text.len());
        if !tab.text.is_char_boundary(start) || !tab.text.is_char_boundary(end) {
            return;
        }
        tab.text.replace_range(start..end, text);
        let after = start + text.len();
        self.selection = (after, after);
        let chars = tab.text.get(..after).map_or(0, |t| t.chars().count());
        self.select_next = Some((chars, chars));
        self.generation += 1;
    }

    /// The selection, or the statement under the cursor: what Run runs (R-SQL-7).
    pub fn current_statement(&self) -> Option<String> {
        let (start, end) = self.selection;
        let script = self.text();
        let chosen = if start < end {
            script
                .get(start..end)
                .map(str::trim)
                .map(|s| s.trim_end_matches(';').trim().to_owned())
        } else {
            text::statement_at(script, start)
                .and_then(|r| script.get(r))
                .map(str::to_owned)
        };
        chosen.filter(|s| !s.is_empty())
    }

    /// The shown script's text and the cursor, as a test sets them.
    #[cfg(test)]
    pub fn set_text(&mut self, text: &str, cursor: usize) {
        if let Some(tab) = self.tabs.get_mut(self.shown) {
            text.clone_into(&mut tab.text);
        }
        self.selection = (cursor, cursor);
    }

    /// The selection, as a test sets it.
    #[cfg(test)]
    pub fn select(&mut self, start: usize, end: usize) {
        self.selection = (start, end);
    }

    /// Whether the editor takes Esc, for the find bar or the completion list (the window's
    /// Cancel does otherwise).
    pub fn takes_escape(&self) -> bool {
        self.find.is_some() || self.popup.as_ref().is_some_and(|p| !p.items.is_empty())
    }

    /// The name being typed and the statement it is in, while completion is wanted (R-SQL-36).
    pub fn completing(&self) -> Option<(parley_core::sql::complete::Typed, String)> {
        let popup = self.popup.as_ref()?;
        let script = self.text();
        let statement = text::statement_at(script, popup.typed.end)
            .and_then(|r| script.get(r))
            .unwrap_or_default();
        Some((popup.typed.clone(), statement.to_owned()))
    }

    /// The name under the pointer and the statement it is in (R-SQL-37).
    pub fn pointed(&self) -> Option<(Name, String)> {
        let (name, at) = self.pointed.clone()?;
        let script = self.text();
        let statement = text::statement_at(script, at)
            .and_then(|r| script.get(r))
            .unwrap_or_default();
        Some((name, statement.to_owned()))
    }

    /// What hovering `name` shows; `None` for nothing.
    pub fn describe(&mut self, name: Name, text: Option<String>) {
        self.described = text.map(|t| (name, t));
    }

    /// What to offer for the name being typed.
    pub fn offer(&mut self, items: Vec<Item>) {
        if let Some(popup) = &mut self.popup {
            popup.offer(items);
        }
    }

    /// Puts offered item `index` in place of what was typed.
    fn accept(&mut self, index: usize) {
        let Some(popup) = self.popup.take() else {
            return;
        };
        if let Some(item) = popup.items.get(index) {
            self.selection = (popup.typed.start, popup.typed.end);
            self.insert(&item.insert);
        }
    }

    /// After the text field: a list while a name is being typed, gone when the cursor leaves it.
    fn follow_typing(&mut self, changed: bool, cursor_at: Option<egui::Pos2>) {
        let (start, end) = self.selection;
        if changed && start == end {
            let typed = parley_core::sql::complete::typed_at(self.text(), start)
                .filter(|t| t.qualifier.is_some() || t.prefix.chars().count() >= 2);
            self.popup = typed.map(|typed| match self.popup.take() {
                Some(mut popup) => {
                    popup.typed = typed;
                    popup
                }
                None => complete::Popup::new(typed),
            });
        } else if self
            .popup
            .as_ref()
            .is_some_and(|p| start != end || p.typed.end != start)
        {
            self.popup = None;
        }
        if let Some(popup) = &mut self.popup {
            popup.at = cursor_at.or(popup.at);
        }
    }

    /// The tabs, the actions and the editor, `height` high for the text.
    pub fn show(&mut self, ui: &mut Ui, height: f32, files: Option<&dyn SqlFiles>) {
        self.keys(ui, files);
        self.tab_strip(ui, files);
        if self.find.is_some() {
            self.find_bar(ui);
        }
        self.text_field(ui, height);
        if let Some(index) = self.popup.as_ref().and_then(|p| p.show(ui)) {
            self.accept(index);
        }
        if let Some(note) = self.note.as_deref().filter(|n| !n.is_empty()) {
            theme::hint(ui, note);
        }
        self.closing_dialog(ui.ctx(), files);
    }

    // ---------------------------------------------------------------- keys and actions

    fn keys(&mut self, ui: &Ui, files: Option<&dyn SqlFiles>) {
        // R-SQL-36: the list takes its keys before the text field and the window see them.
        let action = self.popup.as_mut().map(|p| (p.keys(ui), p.chosen));
        match action {
            Some((complete::KeyAction::Accept, chosen)) => self.accept(chosen),
            Some((complete::KeyAction::Close, _)) => self.popup = None,
            Some((complete::KeyAction::None, _)) | None => {}
        }
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
        // R-SQL-28: without a selection, the statement Run would execute is shown faintly.
        let (start, end) = self.selection;
        let current = if start < end {
            None
        } else {
            text::statement_at(&tab.text, start)
        };
        let mut layouter =
            |ui: &Ui, buffer: &dyn egui::TextBuffer, wrap_width: f32| -> Arc<egui::Galley> {
                let text = buffer.as_str();
                // Only while the text is the one the range was found in.
                let current = current.as_ref().filter(|r| r.end <= text.len());
                let job = coloured(ui, text, wrap_width, current);
                ui.fonts_mut(|f| f.layout_job(job))
            };
        let before = tab.text.len();
        let mut changed = false;
        let mut cursor_at = None;
        let mut pointer_at = None;
        let described = self.described.as_ref().filter(|(name, _)| {
            self.pointed
                .as_ref()
                .is_some_and(|(pointed, _)| pointed == name)
        });
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
                let response = &output.response.response;
                if response.hovered()
                    && let Some(pos) = response.hover_pos()
                {
                    let cursor = output.galley.cursor_from_pos(pos - output.galley_pos);
                    pointer_at = Some(byte_at(&tab.text, cursor.index.0));
                    if let Some((_, text)) = described {
                        response.clone().on_hover_text(text.as_str());
                    }
                }
                if let Some(range) = output.cursor_range {
                    let below = output.galley.pos_from_cursor(range.primary).left_bottom();
                    cursor_at = Some(output.galley_pos + below.to_vec2());
                    let (a, b) = (range.primary.index.0, range.secondary.index.0);
                    self.selection = (byte_at(&tab.text, a.min(b)), byte_at(&tab.text, a.max(b)));
                }
            });
        let changed = changed || tab.text.len() != before;
        if changed {
            self.generation += 1;
        }
        self.follow_typing(changed, cursor_at);
        self.pointed = pointer_at.and_then(|at| {
            let name = parley_core::sql::complete::name_at(self.text(), at)?;
            Some((name, at))
        });
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
fn coloured(ui: &Ui, text: &str, wrap_width: f32, current: Option<&Range<usize>>) -> LayoutJob {
    let p = theme::palette(ui);
    let plain = ui.visuals().text_color();
    let font = egui::TextStyle::Monospace.resolve(ui.style());
    let spans = if text.len() <= MAX_COLOURED {
        text::highlight(text)
    } else {
        Vec::new()
    };
    let mut job = LayoutJob::default();
    // Appends `range` of `text` in `color`, the part inside `current` on a faint background.
    let mut append = |range: Range<usize>, color: Color32| {
        let cuts = current.map_or(vec![range.clone()], |c| {
            let a = c.start.clamp(range.start, range.end);
            let b = c.end.clamp(range.start, range.end);
            vec![range.start..a, a..b, b..range.end]
        });
        for cut in cuts.into_iter().filter(|c| !c.is_empty()) {
            let Some(piece) = text.get(cut.clone()) else {
                continue;
            };
            let mut format = TextFormat::simple(font.clone(), color);
            if current
                .as_ref()
                .is_some_and(|c| c.start <= cut.start && cut.end <= c.end)
            {
                format.background = p.surface_hover;
            }
            job.append(piece, 0.0, format);
        }
    };
    let mut at = 0;
    for (range, kind) in spans {
        if range.start < at {
            continue;
        }
        append(at..range.start, plain);
        let color = match kind {
            Kind::Keyword => p.blue,
            Kind::String => p.ok,
            Kind::Name => p.purple,
            Kind::Number => p.redirect,
            Kind::Comment => p.muted,
        };
        append(range.clone(), color);
        at = range.end;
    }
    append(at..text.len(), plain);
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

    #[test]
    fn the_statement_run_would_execute_is_on_a_faint_background_r_sql_28() {
        let text = "VALUES 1;\nSELECT 'æøå' FROM T;\n";
        let current = text::statement_at(text, 12).expect("a statement");
        let mut harness = egui_kittest::Harness::new_ui(move |ui| {
            crate::test_support::install_fonts(ui);
            let job = coloured(ui, text, 400.0, Some(&current));
            let shaded: String = job
                .sections
                .iter()
                .filter(|s| s.format.background != Color32::TRANSPARENT)
                .filter_map(|s| text.get(s.byte_range.start.0..s.byte_range.end.0))
                .collect();
            assert_eq!(shaded, "SELECT 'æøå' FROM T");
            let plain = coloured(ui, text, 400.0, None);
            assert!(
                plain
                    .sections
                    .iter()
                    .all(|s| s.format.background == Color32::TRANSPARENT)
            );
        });
        harness.run();
    }

    fn offered(label: &str) -> Item {
        Item {
            label: label.to_owned(),
            detail: "column".to_owned(),
            insert: label.to_owned(),
        }
    }

    #[test]
    fn typing_a_name_opens_the_list_and_a_choice_replaces_it_r_sql_36() {
        let mut scripts = Scripts::restore(&[]);
        let text = "SELECT k.na FROM qgpl.kunder k";
        scripts.set_text(text, 11);
        scripts.follow_typing(true, None);
        let (typed, statement) = scripts.completing().expect("completing");
        assert_eq!(
            (typed.prefix.as_str(), typed.qualifier.as_deref()),
            ("NA", Some("K"))
        );
        assert_eq!(statement, text);
        scripts.offer(vec![offered("NAVN"), offered("NAVN_ÆØÅ")]);
        assert!(
            scripts.takes_escape(),
            "Esc closes the list, not the window's Cancel"
        );
        scripts.accept(1);
        assert_eq!(scripts.text(), "SELECT k.NAVN_ÆØÅ FROM qgpl.kunder k");
        assert!(scripts.completing().is_none());
    }

    #[test]
    fn the_list_closes_when_the_cursor_leaves_the_name_r_sql_36() {
        let mut scripts = Scripts::restore(&[]);
        scripts.set_text("SELECT ka", 9);
        scripts.follow_typing(true, None);
        assert!(scripts.completing().is_some());
        scripts.select(3, 3);
        scripts.follow_typing(false, None);
        assert!(scripts.completing().is_none());
        // One letter is not enough to offer anything without a qualifier.
        scripts.set_text("SELECT k", 8);
        scripts.follow_typing(true, None);
        assert!(scripts.completing().is_none());
    }

    #[test]
    fn down_and_enter_choose_from_the_list_r_sql_36() {
        let mut scripts = Scripts::restore(&[]);
        scripts.set_text("SELECT na", 9);
        scripts.follow_typing(true, Some(egui::pos2(40.0, 40.0)));
        scripts.offer(vec![offered("NAVN"), offered("NAVN2")]);
        let mut harness = egui_kittest::Harness::builder()
            .with_size(egui::vec2(600.0, 400.0))
            .build_ui_state(
                |ui, s: &mut Scripts| {
                    crate::test_support::install_fonts(ui);
                    s.show(ui, 200.0, None);
                },
                scripts,
            );
        harness.run_steps(2);
        harness.key_press(Key::ArrowDown);
        harness.key_press(Key::Enter);
        harness.run_steps(2);
        assert_eq!(harness.state().text(), "SELECT NAVN2");
    }
}
