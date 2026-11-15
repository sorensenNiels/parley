//! The History tab (R-SQL-89..91, D35): every run, newest first under day headings, each as two
//! lines: a mark and the statement in the editor's colours, then when, how long and how it
//! ended. A search, All / Succeeded / Failed and Clear history above; a click selects, a
//! double-click or Enter adds the statement to the script.

use egui::text::TextWrapping;
use egui::{Key, Modifiers, Rect, Sense, TextEdit, Ui, vec2};
use parley_sql_core::history::{self, Entry, Show};

use super::editor::{Marks, coloured, gutter};
use parley_ui_egui::{dialog, icons, theme};

/// A row: the statement's line and the summary's.
const ROW_HEIGHT: f32 = 40.0;
/// Room for the mark left of the statement.
const MARK: f32 = 18.0;

/// What was asked of an entry, or of the history (R-SQL-91).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HistoryAction {
    /// Add the statement to the end of the script (double-click, Enter).
    Append(String),
    /// Insert it at the cursor.
    Insert(String),
    /// Open it in a new script tab.
    NewTab(String),
    Run(String),
    SaveSnippet(String),
    /// Remove this run from the history.
    Remove(Entry),
    /// Remove every run of this connection.
    Clear,
}

/// The list's search, filter and selection.
#[derive(Debug, Default)]
pub struct HistoryList {
    query: String,
    show: Show,
    /// The run selected; kept by value, so a new run does not move the selection.
    selected: Option<Entry>,
    /// Asking before Clear history.
    clearing: bool,
}

impl HistoryList {
    /// The tab for `entries`, oldest first; what was asked.
    pub fn show(&mut self, ui: &mut Ui, entries: &[Entry]) -> Vec<HistoryAction> {
        let mut actions = Vec::new();
        self.toolbar(ui, !entries.is_empty());
        ui.add_space(4.0);
        if entries.is_empty() {
            theme::hint(ui, "Statements you run are listed here.");
        }
        let query = self.query.trim().to_lowercase();
        let shown: Vec<&Entry> = entries
            .iter()
            .rev()
            .filter(|e| e.matches(&query, self.show))
            .collect();
        if shown.is_empty() && !entries.is_empty() {
            theme::hint(ui, "Nothing found.");
        }
        self.keys(ui, &shown, &mut actions);
        let today = history::today();
        egui::ScrollArea::vertical()
            .id_salt("sql-history")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let mut day = None;
                for (n, entry) in shown.iter().enumerate() {
                    let date = entry.date();
                    if n == 0 || date != day {
                        ui.add_space(if n == 0 { 0.0 } else { 6.0 });
                        theme::section_label(ui, &history::day_heading(date, today));
                        day = date;
                    }
                    self.row(ui, entry, &mut actions);
                }
            });
        if self.clearing {
            self.clear_dialog(ui.ctx(), &mut actions);
        }
        actions
    }

    /// Search, All / Succeeded / Failed and Clear history (R-SQL-90).
    fn toolbar(&mut self, ui: &mut Ui, any: bool) {
        ui.horizontal(|ui| {
            ui.add(
                TextEdit::singleline(&mut self.query)
                    .hint_text("Search history")
                    .desired_width(220.0),
            );
            theme::segmented(
                ui,
                &mut self.show,
                &[
                    (Show::All, "All"),
                    (Show::Succeeded, "Succeeded"),
                    (Show::Failed, "Failed"),
                ],
            );
            theme::button_row(ui, |ui| {
                let clear = icons::button(icons::ERASER, "Clear history…");
                if icons::add_named(ui, any, clear, "Clear history…").clicked() {
                    self.clearing = true;
                }
            });
        });
    }

    /// The arrow keys move the selection, Enter uses it (R-SQL-91); not while a field has the
    /// keyboard.
    fn keys(&mut self, ui: &Ui, shown: &[&Entry], actions: &mut Vec<HistoryAction>) {
        let Some(selected) = self.selected.clone() else {
            return;
        };
        if ui.ctx().egui_wants_keyboard_input() {
            return;
        }
        let at = shown.iter().position(|e| **e == selected);
        let (down, up, enter) = ui.input_mut(|i| {
            (
                i.consume_key(Modifiers::NONE, Key::ArrowDown),
                i.consume_key(Modifiers::NONE, Key::ArrowUp),
                i.consume_key(Modifiers::NONE, Key::Enter),
            )
        });
        let next = match (at, down, up) {
            (Some(at), true, _) => shown.get(at + 1),
            (Some(at), _, true) => at.checked_sub(1).and_then(|i| shown.get(i)),
            _ => None,
        };
        if let Some(next) = next {
            self.selected = Some((*next).clone());
        }
        if enter && at.is_some() {
            actions.push(HistoryAction::Append(selected.statement.clone()));
        }
    }

    /// One run on two lines; its buttons while selected, its menu on a right-click.
    fn row(&mut self, ui: &mut Ui, entry: &Entry, actions: &mut Vec<HistoryAction>) {
        let p = theme::palette(ui);
        let (rect, response) =
            ui.allocate_exact_size(vec2(ui.available_width(), ROW_HEIGHT), Sense::click());
        let selected = self.selected.as_ref() == Some(entry);
        response.widget_info(|| {
            egui::WidgetInfo::selected(
                egui::accesskit::Role::ListItem,
                true,
                selected,
                &entry.statement,
            )
        });
        if selected {
            ui.painter()
                .rect_filled(rect, 4.0, p.accent.gamma_multiply(0.14));
        } else if response.hovered() {
            ui.painter().rect_filled(rect, 4.0, p.surface_hover);
        }
        let top = Rect::from_min_size(rect.min, vec2(rect.width(), ROW_HEIGHT / 2.0));
        if let Some(outcome) = &entry.outcome {
            gutter::paint_mark(
                ui,
                Rect::from_min_size(top.min, vec2(MARK, top.height())),
                outcome,
            );
        }
        let text_width = (rect.width() - MARK - 4.0).max(10.0);
        let x = rect.left() + MARK;
        Self::statement_line(ui, entry, x, top, text_width);
        let colour = if entry.succeeded() == Some(false) {
            p.fail
        } else {
            p.muted
        };
        let buttons = if selected { 230.0 } else { 0.0 };
        let summary = ui.painter().layout_job(one_line(
            egui::text::LayoutJob::simple_singleline(
                entry.summary(),
                egui::TextStyle::Small.resolve(ui.style()),
                colour,
            ),
            (text_width - buttons).max(10.0),
        ));
        let below = top.bottom() + (ROW_HEIGHT / 2.0 - summary.size().y) / 2.0;
        ui.painter().galley(egui::pos2(x, below), summary, colour);
        if selected {
            let area =
                Rect::from_min_max(egui::pos2(rect.right() - buttons, top.bottom()), rect.max);
            ui.scope_builder(egui::UiBuilder::new().max_rect(area), |ui| {
                theme::button_row(ui, |ui| Self::row_buttons(ui, entry, actions));
            });
        }
        let response = response.on_hover_text(&entry.statement);
        if response.clicked() {
            self.selected = Some(entry.clone());
        }
        if response.double_clicked() {
            actions.push(HistoryAction::Append(entry.statement.clone()));
        }
        response.context_menu(|ui| self.menu(ui, entry, actions));
    }

    /// The statement on one line in the editor's colours, cut at `width`.
    fn statement_line(ui: &Ui, entry: &Entry, x: f32, top: Rect, width: f32) {
        let line = entry
            .statement
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        let font = egui::TextStyle::Monospace.resolve(ui.style());
        let job = one_line(coloured(ui, &line, &font, width, &Marks::default()), width);
        let galley = ui.painter().layout_job(job);
        let y = top.top() + (top.height() - galley.size().y) / 2.0;
        ui.painter()
            .galley(egui::pos2(x, y), galley, ui.visuals().text_color());
    }

    /// Insert, Run again and Copy on the selected row (R-SQL-91).
    fn row_buttons(ui: &mut Ui, entry: &Entry, actions: &mut Vec<HistoryAction>) {
        let statement = &entry.statement;
        if icons::add_named(ui, true, icons::button(icons::COPY, "Copy"), "Copy").clicked() {
            ui.ctx().copy_text(statement.clone());
        }
        let run = icons::button(icons::PLAY, "Run again");
        if icons::add_named(ui, true, run, "Run again").clicked() {
            actions.push(HistoryAction::Run(statement.clone()));
        }
        if icons::add_named(ui, true, egui::Button::new("Insert"), "Insert")
            .on_hover_text("At the cursor")
            .clicked()
        {
            actions.push(HistoryAction::Insert(statement.clone()));
        }
    }

    /// The right-click menu (R-SQL-91).
    fn menu(&mut self, ui: &mut Ui, entry: &Entry, actions: &mut Vec<HistoryAction>) {
        let statement = &entry.statement;
        let mut item = |ui: &mut Ui, label: &str, action: Option<HistoryAction>| {
            if ui.button(label).clicked() {
                if let Some(action) = action {
                    actions.push(action);
                } else {
                    ui.ctx().copy_text(statement.clone());
                }
                ui.close();
            }
        };
        item(ui, "Insert", Some(HistoryAction::Insert(statement.clone())));
        item(
            ui,
            "Open in a new script tab",
            Some(HistoryAction::NewTab(statement.clone())),
        );
        item(ui, "Run again", Some(HistoryAction::Run(statement.clone())));
        item(ui, "Copy", None);
        item(
            ui,
            "Save as snippet…",
            Some(HistoryAction::SaveSnippet(statement.clone())),
        );
        ui.separator();
        item(
            ui,
            "Remove from history",
            Some(HistoryAction::Remove(entry.clone())),
        );
        self.selected = Some(entry.clone());
    }

    /// Asks before Clear history.
    fn clear_dialog(&mut self, ctx: &egui::Context, actions: &mut Vec<HistoryAction>) {
        let (close, (cancel, clear)) = dialog::dialog(
            ctx,
            "sql-history-clear",
            "Clear the history?",
            400.0,
            |ui| {
                ui.label("Every statement run on this connection is removed from the history.");
                dialog::actions(ui, theme::danger_button(ui, "Clear"), true)
            },
        );
        if close || cancel || clear {
            self.clearing = false;
        }
        if clear {
            self.selected = None;
            actions.push(HistoryAction::Clear);
        }
    }
}

/// `job` on one line of at most `width`, cut with "…".
fn one_line(mut job: egui::text::LayoutJob, width: f32) -> egui::text::LayoutJob {
    job.wrap = TextWrapping::truncate_at_width(width);
    job
}

#[cfg(test)]
mod tests {
    use egui_kittest::kittest::Queryable;
    use parley_sql_core::history::Outcome;

    use super::super::tests::{drawn_with_history, window};
    use super::*;

    fn run(statement: &str, outcome: Outcome) -> Entry {
        Entry::now(statement, std::time::Duration::from_millis(310), outcome)
    }

    fn opened(history: Vec<Entry>) -> egui_kittest::Harness<'static, super::super::SqlWindow> {
        let mut harness = drawn_with_history(window(), history);
        harness.get_by_label("History").click();
        harness.run();
        harness
    }

    /// R-SQL-90: runs are listed newest first under Today, each with how it ended; old
    /// entries under Earlier.
    #[test]
    fn runs_are_listed_by_day_with_their_outcome_r_sql_90() {
        let old = Entry {
            statement: "VALUES 'gammel'".to_owned(),
            at: None,
            took_ms: None,
            outcome: None,
        };
        let harness = opened(vec![
            old,
            run(
                "SELECT * FROM KUNDER",
                Outcome::Rows {
                    count: 128,
                    more: true,
                },
            ),
            run(
                "SELECT * FROM NOPE",
                Outcome::Failed("SQL0204 NOPE æøå".to_owned()),
            ),
        ]);
        harness.get_by_label("TODAY");
        harness.get_by_label("EARLIER");
        let nope = harness.get_by_label("SELECT * FROM NOPE").rect();
        let kunder = harness.get_by_label("SELECT * FROM KUNDER").rect();
        assert!(nope.top() < kunder.top(), "newest first");
        let gammel = harness.get_by_label("VALUES 'gammel'").rect();
        assert!(kunder.top() < gammel.top(), "earlier last");
    }

    /// R-SQL-90: the search and Failed narrow the list; nothing found says so.
    #[test]
    fn search_and_failed_narrow_the_list_r_sql_90() {
        let mut harness = opened(vec![
            run("SELECT * FROM KUNDER", Outcome::Done),
            run(
                "SELECT * FROM NOPE",
                Outcome::Failed("SQL0204 NOPE".to_owned()),
            ),
        ]);
        harness.get_by_label("Failed").click();
        harness.run();
        assert!(harness.query_by_label("SELECT * FROM KUNDER").is_none());
        harness.get_by_label("SELECT * FROM NOPE");
        harness.get_by_label("All").click();
        harness.run();
        harness.state_mut().history_list.query = "ordre".to_owned();
        harness.run();
        harness.get_by_label("Nothing found.");
    }

    /// R-SQL-91: the selected run offers Insert, Run again and Copy; Insert puts it at the
    /// cursor, and the arrow keys move the selection.
    #[test]
    fn the_selected_run_can_be_inserted_and_the_arrows_move_r_sql_91() {
        let mut harness = opened(vec![
            run("SELECT 1", Outcome::Done),
            run("SELECT 2", Outcome::Done),
        ]);
        assert!(
            harness.query_by_label("Run again").is_none(),
            "nothing selected"
        );
        harness.get_by_label("SELECT 2").click();
        harness.run();
        harness.get_by_label("Run again");
        harness.key_press(egui::Key::ArrowDown);
        harness.run();
        assert_eq!(
            harness
                .state()
                .history_list
                .selected
                .as_ref()
                .map(|e| e.statement.as_str()),
            Some("SELECT 1")
        );
        harness.get_by_label("Insert").click();
        harness.run();
        assert!(harness.state().scripts.text().contains("SELECT 1"));
    }

    /// R-SQL-90: Clear history asks first, then asks the program to clear it.
    #[test]
    fn clear_history_asks_first_r_sql_90() {
        let mut harness = opened(vec![run("SELECT 1", Outcome::Done)]);
        harness.get_by_label("Clear history…").click();
        harness.run();
        harness.get_by_label("Clear the history?");
        crate::test_support::keeps_its_place(&mut harness, "Clear");
        assert_eq!(harness.state().history_edits, []);
        harness.get_by_label("Clear").click();
        harness.run();
        assert_eq!(harness.state().history_edits, [HistoryAction::Clear]);
    }
}
