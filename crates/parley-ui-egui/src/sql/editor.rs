//! The SQL window's editor (R-SQL-19..23): scripts in tabs, kept per saved session; SQL in
//! colour; open and save `.sql` files; format; find and replace.

use std::sync::Arc;

use crate::sql::keys::SqlAction;
use egui::text::{CCursor, CCursorRange};
use egui::{Key, Modifiers, ScrollArea, TextEdit, Ui};
use parley_core::sql::text;
use serde::{Deserialize, Serialize};

use super::SqlFiles;
use super::layout::TextHeight;
use crate::{dialog, icons, theme};

mod colour;
mod complete;
mod editing;
mod error_mark;
mod find;
pub(super) use find::find_next;
pub mod gutter;
mod signature;
pub use complete::{Item, arrange};

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
    find: Option<find::Find>,
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
    /// The byte under the pointer last frame.
    pointer: Option<usize>,
    /// Where the last error stopped (R-SQL-75).
    error_mark: Option<error_mark::ErrorMark>,
    /// The Go to line bar (R-SQL-57).
    go_to: Option<editing::GoTo>,
    /// The editor's text size in percent, while the window is open (R-SQL-59).
    zoom: u16,
    /// Ctrl and the wheel since the last step of zoom.
    pinch: f32,
    /// Scroll the cursor into view on the next frame.
    reveal: bool,
    /// Brackets and quotes close themselves (R-SQL-55); from the settings.
    pub close_pairs: bool,
    /// How Format SQL writes (R-SQL-77).
    pub format: text::Style,
    /// The window's shortcuts (R-SQL-60); from the settings.
    pub keys: crate::sql::keys::SqlKeys,
    /// The shown script's text field's id last frame, to know whether it has the focus.
    field: Option<egui::Id>,
    /// The tab the completion list and selection belong to.
    shown_number: Option<u32>,
    /// What the statements run in the window gave, for the gutter (R-SQL-62).
    pub outcomes: gutter::Outcomes,
    /// A statement's ▶ was clicked: the window runs the selection (R-SQL-62).
    run_asked: bool,
    /// Below the cursor last frame, in screen points.
    cursor_at: Option<egui::Pos2>,
    /// A snippet chosen in the completion list, for the window to put in (R-SQL-64).
    snippet_chosen: Option<usize>,
    /// A routine's parameters while it is called (R-SQL-65).
    help: signature::Help,
}

/// What the text field did this frame.
#[derive(Debug, Default)]
struct FieldOut {
    changed: bool,
    hovered: bool,
    /// Below the cursor, in screen points: where the completion list goes.
    cursor_at: Option<egui::Pos2>,
    /// The byte under the pointer.
    pointer_at: Option<usize>,
    /// The selection, as byte offsets, while the field has a cursor.
    selection: Option<(usize, usize)>,
    /// The statement whose ▶ in the gutter was clicked (R-SQL-62).
    run: Option<std::ops::Range<usize>>,
}

impl FieldOut {
    /// Reads `output` of the field showing `text`: scrolls to the cursor when `reveal`, and
    /// shows `described` on hover (R-SQL-37).
    fn read(
        ui: &Ui,
        output: &egui::text_edit::TextEditOutput,
        text: &str,
        reveal: bool,
        described: Option<&str>,
    ) -> Self {
        let mut out = Self {
            changed: output.response.changed(),
            ..Self::default()
        };
        let response = &output.response.response;
        out.hovered = response.hovered();
        if response.hovered()
            && let Some(pos) = response.hover_pos()
        {
            let cursor = output.galley.cursor_from_pos(pos - output.galley_pos);
            out.pointer_at = Some(byte_at(text, cursor.index.0));
            if let Some(described) = described {
                response.clone().on_hover_text(described);
            }
        }
        if let Some(range) = output.cursor_range {
            let cursor = output.galley.pos_from_cursor(range.primary);
            out.cursor_at = Some(output.galley_pos + cursor.left_bottom().to_vec2());
            if reveal {
                let rect = cursor.translate(output.galley_pos.to_vec2());
                ui.scroll_to_rect(rect, Some(egui::Align::Center));
            }
            let (a, b) = (range.primary.index.0, range.secondary.index.0);
            out.selection = Some((byte_at(text, a.min(b)), byte_at(text, a.max(b))));
        }
        out
    }
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
            pointer: None,
            error_mark: None,
            go_to: None,
            zoom: 100,
            pinch: 1.0,
            reveal: false,
            close_pairs: true,
            format: text::Style::default(),
            keys: crate::sql::keys::SqlKeys::default(),
            field: None,
            shown_number: None,
            outcomes: gutter::Outcomes::default(),
            run_asked: false,
            cursor_at: None,
            snippet_chosen: None,
            help: signature::Help::default(),
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
        scripts.shown_number = scripts.tabs.get(scripts.shown).map(|t| t.number);
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
    /// The new tab gets the keyboard, with the cursor at the end (an assistant's answer).
    pub fn open_new(&mut self, title: &str, text: &str) {
        self.new_tab();
        if let Some(tab) = self.tabs.last_mut() {
            title.clone_into(&mut tab.title);
            text.clone_into(&mut tab.text);
        }
        self.select_range((text.len(), text.len()));
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
        self.find.is_some()
            || self.go_to.is_some()
            || self.help_shown()
            || self.popup.as_ref().is_some_and(|p| !p.items.is_empty())
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
            match item.snippet {
                // What was typed goes; the window puts the snippet in (R-SQL-64).
                Some(snippet) => {
                    self.insert("");
                    self.snippet_chosen = Some(snippet);
                }
                None => self.insert(&item.insert),
            }
        }
    }

    /// The snippet chosen in the completion list since last asked (R-SQL-64).
    pub fn take_snippet_chosen(&mut self) -> Option<usize> {
        self.snippet_chosen.take()
    }

    /// Ctrl+Space (R-SQL-63): the completion list at the cursor, also with nothing typed.
    pub(super) fn open_completion(&mut self) {
        let (start, end) = self.selection;
        if start != end {
            return;
        }
        let typed = parley_core::sql::complete::typed_at(self.text(), start).unwrap_or(
            parley_core::sql::complete::Typed {
                start,
                end: start,
                prefix: String::new(),
                qualifier: None,
            },
        );
        let mut popup = complete::Popup::new(typed);
        popup.at = self.cursor_at;
        self.popup = Some(popup);
    }

    /// After the text field: a list while a name is being typed, gone when the cursor leaves it.
    fn follow_typing(&mut self, changed: bool, cursor_at: Option<egui::Pos2>) {
        let (start, end) = self.selection;
        if changed && start == end {
            // An open list stays open while typing goes on, however short the name (R-SQL-63).
            let open = self.popup.is_some();
            let typed = parley_core::sql::complete::typed_at(self.text(), start)
                .filter(|t| open || t.qualifier.is_some() || t.prefix.chars().count() >= 2);
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

    /// The tabs, the actions and the editor, with the text `height` high; returns how high the
    /// text was drawn.
    pub fn show(&mut self, ui: &mut Ui, height: TextHeight, files: Option<&dyn SqlFiles>) -> f32 {
        self.follow_tab();
        self.keys(ui);
        self.tab_strip(ui, files);
        self.follow_tab();
        if self.find.is_some() {
            self.find_bar(ui);
        }
        if self.go_to.is_some() {
            self.go_to_bar(ui);
        }
        let shown = self.text_field(ui, height);
        self.help_box(ui);
        if let Some(index) = self.popup.as_ref().and_then(|p| p.show(ui)) {
            self.accept(index);
        }
        if let Some(note) = self.note.as_deref().filter(|n| !n.is_empty()) {
            theme::hint(ui, note);
        }
        self.closing_dialog(ui.ctx(), files);
        shown
    }

    // ---------------------------------------------------------------- keys and actions

    fn keys(&mut self, ui: &Ui) {
        // R-SQL-36: the list takes its keys before the text field and the window see them, and
        // only while the text has the keyboard.
        let focused = self.has_focus(ui.ctx());
        let action = self
            .popup
            .as_mut()
            .filter(|_| focused)
            .map(|p| (p.keys(ui), p.chosen));
        match action {
            Some((complete::KeyAction::Accept, chosen)) => self.accept(chosen),
            Some((complete::KeyAction::Close, _)) => self.popup = None,
            Some((complete::KeyAction::None, _)) | None => {}
        }
        if self.has_focus(ui.ctx()) {
            self.editing_keys(ui);
        }
        if (self.find.is_some() || self.go_to.is_some() || self.help_shown())
            && ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape))
        {
            // The parameters box first, then the Go to line bar, the newer of the two bars.
            if self.help_shown() {
                self.hide_help();
            } else if self.go_to.take().is_none() {
                self.find = None;
            }
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
            let width = (ui.available_width() - 380.0).max(120.0);
            ui.allocate_ui(egui::vec2(width, 0.0), |ui| {
                ScrollArea::horizontal()
                    .id_salt("sql-script-tabs")
                    .show(ui, |ui| {
                        theme::tabs(ui, &mut self.shown, &items);
                    });
            });
            let new = icons::button(icons::PLUS, "").small();
            if icons::add_named(ui, true, new, "New script")
                .on_hover_text(self.keys.hint("New script", SqlAction::NewScript))
                .clicked()
            {
                self.new_tab();
            }
            if ui
                .add(theme::icon_button(ui, "🗙"))
                .on_hover_text(self.keys.hint("Close this script", SqlAction::CloseScript))
                .clicked()
            {
                self.close_tab(self.shown);
            }
            theme::button_row(ui, |ui| {
                if let Some(files) = files {
                    if ui.small_button("Save as…").clicked() {
                        self.save(files, true);
                    }
                    let save = self.keys.hint("Save", SqlAction::Save);
                    if ui.small_button("Save").on_hover_text(save).clicked() {
                        self.save(files, false);
                    }
                    let open = self.keys.hint("Open a file", SqlAction::Open);
                    if ui.small_button("Open…").on_hover_text(open).clicked() {
                        self.open(files);
                    }
                }
                self.case_menu(ui);
                if ui
                    .small_button("Format")
                    .on_hover_text(self.keys.hint("Format", SqlAction::Format))
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
        // R-SQL-68: no blanks at the ends of lines, in the file and the script.
        let (trimmed, cursor) =
            parley_core::sql::edit::trim_trailing(self.text(), self.selection.0);
        if trimmed != self.text() {
            self.apply(parley_core::sql::edit::Edit {
                text: trimmed,
                selection: (cursor, cursor),
            });
        }
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
        let formatted = text::format_with(original, self.format);
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

    /// The shown script's text field's id.
    fn field_id(&self, ui: &Ui) -> Option<egui::Id> {
        let tab = self.tabs.get(self.shown)?;
        Some(ui.make_persistent_id(("sql-script", tab.number)))
    }

    /// When another tab is shown, the completion list and the selection of the one before
    /// go: the list would take the arrows in a text it was not opened for.
    fn follow_tab(&mut self) {
        let number = self.tabs.get(self.shown).map(|t| t.number);
        if number != self.shown_number {
            self.shown_number = number;
            self.popup = None;
            if self.select_next.is_none() {
                self.selection = (0, 0);
            }
        }
    }

    /// Whether a statement's ▶ was clicked since last asked (R-SQL-62); it is selected.
    pub fn take_run_asked(&mut self) -> bool {
        std::mem::take(&mut self.run_asked)
    }

    /// Whether the shown script's text has the keyboard (as of last frame's field).
    pub fn has_focus(&self, ctx: &egui::Context) -> bool {
        self.field.is_some_and(|id| ctx.memory(|m| m.has_focus(id)))
    }

    fn text_field(&mut self, ui: &mut Ui, height: TextHeight) -> f32 {
        let Some(id) = self.field_id(ui) else {
            return 0.0;
        };
        self.field = Some(id);
        // R-SQL-75: read before the tab is borrowed for the field.
        let error = self.error_place();
        let error_message = self
            .pointer
            .and_then(|at| self.error_at(at))
            .map(str::to_owned);
        let Some(tab) = self.tabs.get_mut(self.shown) else {
            return 0.0;
        };
        if let Some((start, end)) = self.select_next.take() {
            // A tab never shown has no state yet: it gets one.
            let mut state = TextEdit::load_state(ui.ctx(), id).unwrap_or_default();
            state.cursor.set_char_range(Some(CCursorRange::two(
                CCursor::new(start),
                CCursor::new(end),
            )));
            state.store(ui.ctx(), id);
            ui.memory_mut(|m| m.request_focus(id));
        }
        let mut marks = colour::Marks::at(&tab.text, self.selection);
        marks.error = error;
        let font = {
            let base = egui::TextStyle::Monospace.resolve(ui.style());
            egui::FontId::new(base.size * f32::from(self.zoom) / 100.0, base.family)
        };
        let mut layouter =
            |ui: &Ui, buffer: &dyn egui::TextBuffer, wrap_width: f32| -> Arc<egui::Galley> {
                let job = colour::coloured(ui, buffer.as_str(), &font, wrap_width, &marks);
                ui.fonts_mut(|f| f.layout_job(job))
            };
        let reveal = std::mem::take(&mut self.reveal);
        let before = tab.text.len();
        let described = self
            .described
            .as_ref()
            .filter(|(name, _)| {
                self.pointed
                    .as_ref()
                    .is_some_and(|(pointed, _)| pointed == name)
            })
            .map(|(_, text)| text.as_str())
            .or(error_message.as_deref());
        let gutter = gutter::width(ui, &tab.text, &font);
        let mut out = FieldOut::default();
        let shown = ScrollArea::vertical()
            .id_salt(("sql-editor", tab.number))
            .max_height(height.most)
            .min_scrolled_height(height.most)
            .show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    // The gutter is painted here once the text is laid out (R-SQL-61).
                    ui.add_space(gutter);
                    let edit = TextEdit::multiline(&mut tab.text)
                        .id(id)
                        .code_editor()
                        .font(font.clone())
                        .layouter(&mut layouter)
                        .desired_width(f32::INFINITY)
                        .desired_rows(8)
                        // As high as the divider was dragged to (R-SQL-50).
                        .min_size(egui::vec2(0.0, height.least))
                        .hint_text("SELECT * FROM QSYS2.SYSTABLES FETCH FIRST 10 ROWS ONLY");
                    let output = edit.show(ui);
                    out = FieldOut::read(ui, &output, &tab.text, reveal, described);
                    let lines = gutter::Lines {
                        text: &tab.text,
                        cursor: out.selection.map_or(0, |(start, _)| start),
                        outcomes: &self.outcomes,
                        font: &font,
                    };
                    out.run = gutter::paint(ui, &output.galley, output.galley_pos, gutter, &lines);
                });
            });
        if let Some(selection) = out.selection {
            self.selection = selection;
        }
        if let Some(statement) = out.run.take() {
            self.select_range((statement.start, statement.end));
            self.run_asked = true;
        }
        let (changed, hovered, cursor_at, pointer_at) =
            (out.changed, out.hovered, out.cursor_at, out.pointer_at);
        self.cursor_at = cursor_at.or(self.cursor_at);
        // The text's own height, or the area's when it scrolls.
        let shown = shown.content_size.y.min(shown.inner_rect.height());
        let changed = changed || self.text().len() != before;
        if changed {
            self.generation += 1;
        }
        self.follow_typing(changed, cursor_at);
        self.pointer = pointer_at;
        self.pointed = pointer_at.and_then(|at| {
            let name = parley_core::sql::complete::name_at(self.text(), at)?;
            Some((name, at))
        });
        if hovered {
            self.wheel_zoom(ui);
        }
        shown
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
    use super::find::{find_next, find_previous, replace_everywhere};
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
    fn format_writes_in_the_style_chosen_r_sql_77() {
        let mut scripts = Scripts::restore(&[]);
        scripts.tabs[0].text = "select a, 'æøå' from t".to_owned();
        scripts.format = text::Style {
            case: text::KeywordCase::Lower,
            indent: 2,
            commas_first: true,
        };
        scripts.format();
        assert_eq!(scripts.text(), "select\n  a\n  , 'æøå'\nfrom\n  t");
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

    /// An editor showing `text`, focused, with byte range `selection` selected.
    fn focused(text: &str, selection: (usize, usize)) -> egui_kittest::Harness<'static, Scripts> {
        use egui_kittest::kittest::Queryable;

        let mut scripts = Scripts::restore(&[]);
        scripts.set_text(text, 0);
        let mut harness = egui_kittest::Harness::builder()
            .with_size(egui::vec2(700.0, 400.0))
            .build_ui_state(
                |ui, s: &mut Scripts| {
                    crate::test_support::install_fonts(ui);
                    // As the window does, before the editor is drawn (R-SQL-60).
                    s.take_keys(ui, None);
                    s.show(
                        ui,
                        super::TextHeight {
                            least: 0.0,
                            most: 200.0,
                        },
                        None,
                    );
                },
                scripts,
            );
        harness.run_steps(2);
        harness
            .get_by_role(egui::accesskit::Role::MultilineTextInput)
            .focus();
        harness.run_steps(2);
        harness.state_mut().select_range(selection);
        harness.run_steps(2);
        harness
    }

    /// R-SQL-44 (a fix): an assistant's answer opens in a tab that has the keyboard, and the
    /// completion list of the tab before does not come with it, so the arrows move the cursor.
    #[test]
    fn the_arrows_move_the_cursor_in_an_answer_opened_with_a_list_open_r_sql_44() {
        let mut harness = focused("SELECT na", (9, 9));
        harness
            .state_mut()
            .follow_typing(true, Some(egui::pos2(40.0, 40.0)));
        harness
            .state_mut()
            .offer(vec![offered("NAVN"), offered("NAVN2")]);
        harness.run_steps(2);
        let answer = "-- Kunderne i Århus\nSELECT 1\nFROM t";
        harness.state_mut().open_new("Assistant", answer);
        harness.run_steps(2);
        assert!(harness.state().popup.is_none(), "the list stayed behind");
        assert_eq!(harness.state().selection(), (answer.len(), answer.len()));
        press(&mut harness, Modifiers::NONE, Key::ArrowUp);
        let below = answer.find("SELECT").expect("a line") + 6;
        assert_eq!(harness.state().selection(), (below, below));
        assert_eq!(harness.state().text(), answer);
    }

    fn press(
        harness: &mut egui_kittest::Harness<'static, Scripts>,
        modifiers: Modifiers,
        key: Key,
    ) {
        harness.key_press_modifiers(modifiers, key);
        harness.run_steps(2);
    }

    #[test]
    fn the_editing_keys_reach_the_text_r_sql_52_r_sql_53_r_sql_54() {
        let mut harness = focused("SELECT a,\nb\nFROM æ", (0, 11));
        press(&mut harness, Modifiers::COMMAND, Key::Slash);
        assert_eq!(harness.state().text(), "-- SELECT a,\n-- b\nFROM æ");
        press(&mut harness, Modifiers::COMMAND, Key::Slash);
        assert_eq!(harness.state().text(), "SELECT a,\nb\nFROM æ");
        press(&mut harness, Modifiers::NONE, Key::Tab);
        assert_eq!(harness.state().text(), "    SELECT a,\n    b\nFROM æ");
        press(&mut harness, Modifiers::SHIFT, Key::Tab);
        assert_eq!(harness.state().text(), "SELECT a,\nb\nFROM æ");
        press(&mut harness, Modifiers::ALT, Key::ArrowDown);
        assert_eq!(harness.state().text(), "FROM æ\nSELECT a,\nb");
        press(&mut harness, Modifiers::COMMAND | Modifiers::SHIFT, Key::D);
        assert_eq!(harness.state().text(), "FROM æ\nSELECT a,\nb\nSELECT a,\nb");
    }

    #[test]
    fn a_typed_bracket_closes_itself_only_when_the_setting_is_on_r_sql_55() {
        let mut harness = focused("SELECT ", (7, 7));
        harness.state_mut().close_pairs = true;
        harness.event(egui::Event::Text("(".to_owned()));
        harness.run_steps(2);
        harness.event(egui::Event::Text("1".to_owned()));
        harness.run_steps(2);
        assert_eq!(harness.state().text(), "SELECT (1)");
        let mut harness = focused("SELECT ", (7, 7));
        harness.state_mut().close_pairs = false;
        harness.event(egui::Event::Text("(".to_owned()));
        harness.run_steps(2);
        assert_eq!(harness.state().text(), "SELECT (");
    }

    #[test]
    fn ctrl_g_goes_to_a_line_r_sql_57() {
        use egui_kittest::kittest::Queryable;

        let mut harness = focused("a\nb\næ\nd", (0, 0));
        press(&mut harness, Modifiers::COMMAND, Key::G);
        harness.get_by_label("Line").type_text("3");
        harness.run_steps(2);
        press(&mut harness, Modifiers::NONE, Key::Enter);
        assert!(harness.query_by_label("Line").is_none(), "the bar closes");
        assert_eq!(harness.state().text(), "a\nb\næ\nd");
        assert_eq!(harness.state().selection(), (4, 4));
    }

    /// R-SQL-23 (a fix): Enter in the find bar selects the match and is not also typed into the
    /// editor, which gets the focus with it.
    #[test]
    fn enter_in_the_find_bar_finds_without_typing_a_line_break_r_sql_23() {
        use egui_kittest::kittest::Queryable;

        let mut harness = focused("æ x æ", (0, 0));
        press(&mut harness, Modifiers::COMMAND, Key::F);
        harness
            .get_by_role(egui::accesskit::Role::TextInput)
            .type_text("x");
        harness.run_steps(2);
        press(&mut harness, Modifiers::NONE, Key::Enter);
        assert_eq!(harness.state().text(), "æ x æ");
        assert_eq!(harness.state().selection(), (3, 4));
    }

    #[test]
    fn keyword_case_and_zoom_keys_r_sql_58_r_sql_59() {
        let mut harness = focused("select a from t", (0, 0));
        press(&mut harness, Modifiers::COMMAND | Modifiers::SHIFT, Key::U);
        assert_eq!(harness.state().text(), "SELECT a FROM t");
        press(&mut harness, Modifiers::COMMAND, Key::Plus);
        press(&mut harness, Modifiers::COMMAND, Key::Plus);
        assert_eq!(harness.state().zoom, 120);
        press(&mut harness, Modifiers::COMMAND, Key::Num0);
        assert_eq!(harness.state().zoom, 100);
        for _ in 0..30 {
            harness.state_mut().zoom_by(-1);
        }
        assert_eq!(harness.state().zoom, editing::ZOOM_LEAST);
    }

    /// A file window that saves without asking and keeps what it was given.
    #[derive(Debug, Default)]
    struct KeptFile(std::sync::Mutex<String>);

    impl SqlFiles for KeptFile {
        fn save_text(&self, _: &str, _: &str) -> Result<Option<String>, String> {
            Ok(None)
        }
        fn export_csv(&self, _: &str, _: &[u8]) -> Result<Option<String>, String> {
            Ok(None)
        }
        fn open_script(&self) -> Result<Option<(String, String)>, String> {
            Ok(None)
        }
        fn open_csv(&self) -> Result<Option<(String, String)>, String> {
            Ok(None)
        }
        fn download_path(
            &self,
            _: &str,
            _: parley_core::sql::transfer::Format,
        ) -> Result<Option<std::path::PathBuf>, String> {
            Ok(None)
        }
        fn save_script(
            &self,
            _: Option<&str>,
            _: &str,
            text: &str,
        ) -> Result<Option<String>, String> {
            let mut kept = self.0.lock().map_err(|e| e.to_string())?;
            text.clone_into(&mut kept);
            Ok(Some("/tmp/x.sql".to_owned()))
        }
    }

    #[test]
    fn saving_trims_blanks_at_line_ends_in_the_file_and_the_script_r_sql_68() {
        let mut scripts = Scripts::restore(&[]);
        scripts.set_text("SELECT 'æ  \nø'  \nFROM t \t", 0);
        let file = KeptFile::default();
        assert!(scripts.save(&file, false));
        let saved = file.0.lock().expect("kept").clone();
        assert_eq!(saved, "SELECT 'æ  \nø'\nFROM t");
        assert_eq!(scripts.text(), saved);
    }

    #[test]
    fn ctrl_space_opens_the_list_and_it_stays_open_while_typing_r_sql_63() {
        let mut harness = focused("SELECT ", (7, 7));
        press(&mut harness, Modifiers::COMMAND, Key::Space);
        let (typed, _) = harness.state().completing().expect("a list");
        assert_eq!(typed.prefix, "");
        harness.state_mut().offer(vec![offered("NAVN")]);
        harness.event(egui::Event::Text("n".to_owned()));
        harness.run_steps(2);
        let (typed, _) = harness.state().completing().expect("still a list");
        assert_eq!(typed.prefix, "N");
    }

    #[test]
    fn a_snippet_chosen_in_the_list_takes_the_place_of_what_was_typed_r_sql_64() {
        let mut harness = focused("SELECT ak", (9, 9));
        harness
            .state_mut()
            .follow_typing(true, Some(egui::pos2(40.0, 40.0)));
        harness.state_mut().offer(vec![Item {
            detail: "snippet".to_owned(),
            snippet: Some(4),
            ..offered("Aktive job")
        }]);
        harness.run_steps(2);
        press(&mut harness, Modifiers::NONE, Key::Enter);
        assert_eq!(harness.state().text(), "SELECT ");
        assert_eq!(harness.state_mut().take_snippet_chosen(), Some(4));
    }

    #[test]
    fn ctrl_d_selects_the_name_then_the_next_place_r_sql_67() {
        let mut harness = focused("a navn, b NAVN", (4, 4));
        press(&mut harness, Modifiers::COMMAND, Key::D);
        assert_eq!(harness.state().selection(), (2, 6));
        press(&mut harness, Modifiers::COMMAND, Key::D);
        assert_eq!(harness.state().selection(), (10, 14));
    }

    #[test]
    fn the_call_at_the_cursor_is_found_and_esc_hides_its_parameters_r_sql_65() {
        use parley_core::sql::catalog::{Parameter, Signature};

        let text = "SELECT 1;\nCALL qgpl.opdater(1, ";
        let mut harness = focused(text, (text.len(), text.len()));
        let call = harness
            .state()
            .calling(&harness.ctx)
            .expect("a call at the cursor");
        assert_eq!((call.name.as_str(), call.argument), ("OPDATER", 1));
        let signature = Signature {
            specific_name: "OPDATER".to_owned(),
            parameters: vec![Parameter {
                mode: "IN".to_owned(),
                name: "KUNDE".to_owned(),
                data_type: "INTEGER".to_owned(),
            }],
        };
        harness
            .state_mut()
            .set_signatures(Some((call.clone(), vec![signature.clone()])));
        harness.run_steps(2);
        assert!(harness.state().takes_escape(), "Esc hides the box first");
        press(&mut harness, Modifiers::NONE, Key::Escape);
        assert!(!harness.state().help_shown());
        harness
            .state_mut()
            .set_signatures(Some((call, vec![signature])));
        assert!(
            !harness.state().help_shown(),
            "hidden while in the same call"
        );
        harness.state_mut().set_signatures(None);
        assert!(!harness.state().help_shown());
    }

    fn offered(label: &str) -> Item {
        Item {
            label: label.to_owned(),
            detail: "column".to_owned(),
            insert: label.to_owned(),
            snippet: None,
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
        // The list takes the keys while the text has the keyboard.
        let mut harness = focused("SELECT na", (9, 9));
        harness
            .state_mut()
            .follow_typing(true, Some(egui::pos2(40.0, 40.0)));
        harness
            .state_mut()
            .offer(vec![offered("NAVN"), offered("NAVN2")]);
        harness.run_steps(2);
        harness.key_press(Key::ArrowDown);
        harness.key_press(Key::Enter);
        harness.run_steps(2);
        assert_eq!(harness.state().text(), "SELECT NAVN2");
    }
}
