//! The editor's keys for changing text (R-SQL-52..59): comment, indent, move and duplicate
//! lines, brackets and quotes, keyword case, go to a line and zoom. The changes themselves are
//! `parley_sql_core::edit`'s; this turns keys into them and puts the result in the script.

use egui::{Event, Key, Modifiers, TextEdit, Ui};
use parley_sql_core::edit::{self, Case, Direction, Edit};
use parley_sql_core::text;

use super::{Scripts, SqlFiles, char_at};
use crate::keys::SqlAction;
use parley_ui_egui::theme;

/// The editor's text size, in percent: the least, the most and a step (R-SQL-59).
pub const ZOOM_LEAST: u16 = 60;
pub const ZOOM_MOST: u16 = 250;
const ZOOM_STEP: u16 = 10;

/// The Go to line bar (R-SQL-57).
#[derive(Debug, Clone, Default)]
pub(super) struct GoTo {
    line: String,
    /// Give the field the focus on the next frame.
    focus: bool,
}

impl Scripts {
    /// Keys that work while the editor's text has the focus; taken before the text field sees
    /// them.
    pub(super) fn editing_keys(&mut self, ui: &Ui) {
        let (outdent, tab) = ui.input_mut(|i| {
            (
                i.consume_key(Modifiers::SHIFT, Key::Tab),
                i.consume_key(Modifiers::NONE, Key::Tab),
            )
        });
        let text = self.text().to_owned();
        let selection = self.selection;
        // Tab stops are the Format SQL style's indentation (R-SQL-53, R-SQL-77).
        let width = usize::from(self.format.indent);
        let edit = if outdent {
            edit::outdent(&text, selection, width)
        } else if tab && selection.0 != selection.1 {
            Some(edit::indent(&text, selection, width))
        } else if tab {
            Some(edit::tab(&text, selection.0, width))
        } else if self.close_pairs {
            self.pair_keys(ui, &text)
        } else {
            None
        };
        if let Some(edit) = edit {
            self.apply(edit);
        }
    }

    /// Takes the keys of the SQL window's actions pressed this frame (R-SQL-60) and does the
    /// editor's; returns the window's own (Run, Run all, Cancel).
    /// None while "Save the script?" is asked: the keys are left to the question, and Esc
    /// cancels it (R-SQL-20).
    pub fn take_keys(&mut self, ui: &Ui, files: Option<&dyn SqlFiles>) -> Vec<SqlAction> {
        if self.closing.is_some() {
            if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
                self.closing = None;
            }
            return Vec::new();
        }
        let pressed = self
            .keys
            .take(ui, self.has_focus(ui.ctx()), self.takes_escape());
        let mut window = Vec::new();
        for action in pressed {
            if !self.act(action, files) {
                window.push(action);
            }
        }
        window
    }

    /// Does `action` when it is the editor's; `false` when it is the window's.
    fn act(&mut self, action: SqlAction, files: Option<&dyn SqlFiles>) -> bool {
        let text = self.text().to_owned();
        let selection = self.selection;
        let edit = match action {
            SqlAction::Run | SqlAction::RunAll | SqlAction::Cancel | SqlAction::SidePanel => {
                return false;
            }
            SqlAction::NewScript => {
                self.new_tab();
                None
            }
            SqlAction::CloseScript => {
                self.close_tab(self.shown);
                None
            }
            SqlAction::NextScript => {
                if !self.tabs.is_empty() {
                    self.shown = (self.shown + 1) % self.tabs.len();
                }
                None
            }
            SqlAction::Open => {
                if let Some(files) = files {
                    self.open(files);
                }
                None
            }
            SqlAction::Save => {
                if let Some(files) = files {
                    self.save(files, false);
                }
                None
            }
            SqlAction::Find => {
                self.open_find(false);
                None
            }
            SqlAction::Replace => {
                self.open_find(true);
                None
            }
            SqlAction::GoToLine => {
                self.open_go_to();
                None
            }
            SqlAction::Format => {
                self.format();
                None
            }
            SqlAction::UpperCase => {
                self.change_case(Case::Upper);
                None
            }
            SqlAction::LowerCase => {
                self.change_case(Case::Lower);
                None
            }
            SqlAction::Comment => edit::toggle_comment(&text, selection),
            SqlAction::DuplicateLines => Some(edit::duplicate_lines(&text, selection)),
            SqlAction::MoveLinesUp => edit::move_lines(&text, selection, Direction::Up),
            SqlAction::MoveLinesDown => edit::move_lines(&text, selection, Direction::Down),
            SqlAction::Complete => {
                self.open_completion();
                None
            }
            SqlAction::SelectNext => {
                if let Some(next) = edit::select_next(&text, selection) {
                    self.select_range(next);
                }
                None
            }
            SqlAction::ZoomIn => {
                self.zoom_by(1);
                None
            }
            SqlAction::ZoomOut => {
                self.zoom_by(-1);
                None
            }
            SqlAction::ZoomReset => {
                self.zoom = 100;
                None
            }
        };
        if let Some(edit) = edit {
            self.apply(edit);
        }
        true
    }

    /// A single `(`, `)`, `'` or `"` typed this frame, or Backspace, as R-SQL-55 changes them;
    /// the key is taken from the input when it does. More typed in one frame goes in as typed.
    fn pair_keys(&self, ui: &Ui, text: &str) -> Option<Edit> {
        let (typed, backspace, others) = ui.input(|i| {
            let mut typed = None;
            let mut backspace = false;
            let mut others = 0;
            for event in &i.events {
                match event {
                    Event::Text(t) => {
                        others += 1;
                        typed = t.chars().next().filter(|_| t.chars().count() == 1);
                    }
                    Event::Key {
                        key: Key::Backspace,
                        pressed: true,
                        modifiers,
                        ..
                    } if modifiers.is_none() => backspace = true,
                    Event::Paste(_) => others += 1,
                    _ => {}
                }
            }
            (typed, backspace, others)
        });
        if others == 1
            && let Some(c) = typed
            && let Some(edit) = edit::typed_pair(text, self.selection, c)
        {
            ui.input_mut(|i| i.events.retain(|e| !matches!(e, Event::Text(_))));
            return Some(edit);
        }
        if others == 0 && backspace && self.selection.0 == self.selection.1 {
            let edit = edit::backspace_pair(text, self.selection.0)?;
            ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Backspace));
            return Some(edit);
        }
        None
    }

    /// Puts `edit` in the shown script, with its selection, and shows the cursor.
    pub(super) fn apply(&mut self, edit: Edit) {
        let Some(tab) = self.tabs.get_mut(self.shown) else {
            return;
        };
        if tab.text != edit.text {
            tab.text = edit.text;
            self.generation += 1;
        }
        self.select_range(edit.selection);
        self.popup = None;
    }

    /// Selects byte range (start, end) of the shown script on the next frame, with the focus,
    /// and scrolls it into view.
    pub(super) fn select_range(&mut self, (start, end): (usize, usize)) {
        let text = self.text();
        let chars = (char_at(text, start), char_at(text, end));
        self.selection = (start, end);
        self.select_next = Some(chars);
        self.reveal = true;
    }

    /// Keywords in the selection, or the statement under the cursor, in `case` (R-SQL-58).
    pub(super) fn change_case(&mut self, case: Case) {
        let (start, end) = self.selection;
        let script = self.text();
        let range = if start < end {
            Some(start..end)
        } else {
            text::statement_at(script, start)
        };
        if let Some(range) = range {
            let changed = edit::keyword_case(script, range, case);
            self.apply(Edit {
                text: changed,
                selection: (start, end),
            });
        }
    }

    /// One step larger (`1`) or smaller (`-1`), within the limits (R-SQL-59).
    pub(super) fn zoom_by(&mut self, steps: i8) {
        let next = if steps > 0 {
            self.zoom.saturating_add(ZOOM_STEP)
        } else {
            self.zoom.saturating_sub(ZOOM_STEP)
        };
        self.zoom = next.clamp(ZOOM_LEAST, ZOOM_MOST);
    }

    /// Ctrl and the wheel over the editor: a step each time the pinch passes 10 % (R-SQL-59).
    pub(super) fn wheel_zoom(&mut self, ui: &Ui) {
        let delta = ui.input(egui::InputState::zoom_delta);
        if (delta - 1.0).abs() < f32::EPSILON {
            return;
        }
        self.pinch *= delta;
        if self.pinch >= 1.1 {
            self.zoom_by(1);
            self.pinch = 1.0;
        } else if self.pinch <= 1.0 / 1.1 {
            self.zoom_by(-1);
            self.pinch = 1.0;
        }
    }

    /// Opens the Go to line bar with the focus in it (Ctrl+G).
    pub(super) fn open_go_to(&mut self) {
        let mut bar = self.go_to.take().unwrap_or_default();
        bar.focus = true;
        self.go_to = Some(bar);
    }

    /// The Go to line bar (R-SQL-57): Enter goes there and closes it.
    pub(super) fn go_to_bar(&mut self, ui: &mut Ui) {
        let Some(mut bar) = self.go_to.take() else {
            return;
        };
        let lines = edit::line_count(self.text());
        let mut go = false;
        let mut close = false;
        theme::card(ui).show(ui, |ui| {
            ui.horizontal(|ui| {
                let label = ui.label("Line");
                let field = theme::sized_edit(ui, 80.0, TextEdit::singleline(&mut bar.line))
                    .labelled_by(label.id);
                if std::mem::take(&mut bar.focus) {
                    field.request_focus();
                }
                // Taken, so it does not also reach the editor that gets the focus now.
                go = field.lost_focus()
                    && ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter));
                theme::hint(ui, format!("of {lines}"));
                go |= ui.small_button("Go").clicked();
                theme::button_row(ui, |ui| {
                    close = ui
                        .add(theme::icon_button(ui, "🗙"))
                        .on_hover_text("Close (Esc)")
                        .clicked();
                });
            });
        });
        if go && let Ok(number) = bar.line.trim().parse::<usize>() {
            let at = edit::line_start(self.text(), number);
            self.select_range((at, at));
            close = true;
        }
        if !close {
            self.go_to = Some(bar);
        }
    }

    /// The Case menu beside Format (R-SQL-58).
    pub(super) fn case_menu(&mut self, ui: &mut Ui) {
        // Small, as the buttons beside it.
        let button = egui::Button::new("Case").small();
        egui::containers::menu::MenuButton::from_button(button)
            .ui(ui, |ui| {
                if ui
                    .button("Upper case")
                    .on_hover_text(self.keys.hint("Keywords in capitals", SqlAction::UpperCase))
                    .clicked()
                {
                    self.change_case(Case::Upper);
                }
                if ui
                    .button("Lower case")
                    .on_hover_text(
                        self.keys
                            .hint("Keywords in small letters", SqlAction::LowerCase),
                    )
                    .clicked()
                {
                    self.change_case(Case::Lower);
                }
            })
            .0
            .on_hover_text("The case of SQL's keywords in the selection or statement");
    }
}
