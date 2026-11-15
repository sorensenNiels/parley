//! Writing and running: the editor with the results below or beside it (R-SQL-49, R-SQL-50),
//! the Run buttons and keys (R-SQL-7, R-SQL-60), the queue, and what is sent to the helper.

use egui::Ui;
use parley_sql_core::params;
use parley_sql_core::protocol::Request;
use parley_sql_core::text;
use parley_ui_egui::{icons, theme};

use super::{
    Around, BELOW_THE_TEXT, SqlFiles, SqlWindow, Tab, WAIT, WRITING, Waiting, download, keys,
    layout, parameters, results, show_link,
};

impl SqlWindow {
    /// The editor with its Run buttons, and the results below or beside it (R-SQL-49).
    pub(crate) fn work(&mut self, ui: &mut Ui, around: Around<'_>) {
        self.scripts.close_pairs = around.close_pairs;
        self.scripts.format = around.format;
        let keys = around.keys.cloned().unwrap_or_default();
        if keys != self.scripts.keys {
            self.scripts.keys = keys;
        }
        self.keys(ui, around.files);
        self.complete();
        self.signatures(ui.ctx());
        self.describe();
        match around.results.arrangement(ui.available_width()) {
            layout::Arrangement::Stacked => self.stacked(ui, around),
            layout::Arrangement::SideBySide => self.side_by_side(ui, around),
        }
    }

    /// The editor above, the results below, a divider between them (R-SQL-50).
    pub(crate) fn stacked(&mut self, ui: &mut Ui, around: Around<'_>) {
        let height =
            layout::editor_height(self.editor_height, ui.available_height(), BELOW_THE_TEXT);
        let shown = ui
            .scope_builder(egui::UiBuilder::new().id_salt(WRITING), |ui| {
                self.writing(ui, height, around)
            })
            .inner;
        let (divider, _) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), layout::DIVIDER),
            egui::Sense::hover(),
        );
        match layout::divider(ui, divider, layout::Axis::Rows) {
            Some(layout::Drag::Moved(by)) => self.editor_height = Some(shown + by),
            Some(layout::Drag::Reset) => self.editor_height = None,
            None => {}
        }
        self.results_and_messages(ui, around);
    }

    /// The editor on the left, the results on the right, a divider between them (R-SQL-49).
    pub(crate) fn side_by_side(&mut self, ui: &mut Ui, around: Around<'_>) {
        let full = ui.available_rect_before_wrap();
        let width = layout::editor_width(self.editor_share, full.width());
        let left = egui::Rect::from_min_size(full.min, egui::vec2(width, full.height()));
        {
            let mut left_ui = ui.new_child(egui::UiBuilder::new().id_salt(WRITING).max_rect(left));
            // The text fills the column, less what was around it the last frame: the Ask row,
            // the tab strip, the find bar, the Run buttons and a line of error (R-SQL-49).
            let text = (left.height() - self.around_the_text).max(90.0);
            let height = layout::TextHeight {
                least: text,
                most: text,
            };
            let shown = self.writing(&mut left_ui, height, around);
            self.around_the_text = (left_ui.min_rect().height() - shown).max(0.0);
        }
        let divider = egui::Rect::from_min_size(
            egui::pos2(left.right(), full.top()),
            egui::vec2(layout::DIVIDER, full.height()),
        );
        match layout::divider(ui, divider, layout::Axis::Columns) {
            Some(layout::Drag::Moved(by)) if full.width() > 0.0 => {
                self.editor_share = ((width + by) / full.width()).clamp(0.0, 1.0);
            }
            Some(layout::Drag::Reset) => self.editor_share = layout::DEFAULT_SHARE,
            Some(layout::Drag::Moved(_)) | None => {}
        }
        let right = egui::Rect::from_min_max(egui::pos2(divider.right(), full.top()), full.max);
        let mut right_ui = ui.new_child(egui::UiBuilder::new().max_rect(right));
        self.results_and_messages(&mut right_ui, around);
        ui.allocate_rect(full, egui::Sense::hover());
    }

    /// The Ask row, the editor with its text `height` tall, the Run buttons and the error line;
    /// returns how high the text was drawn.
    pub(crate) fn writing(
        &mut self,
        ui: &mut Ui,
        height: layout::TextHeight,
        around: Around<'_>,
    ) -> f32 {
        if self.assistant.on {
            self.assistant_row(ui);
        }
        let shown = self.scripts.show(ui, height, around.files);
        ui.add_space(6.0);
        self.run_buttons(ui);
        if let Some(error) = &self.error_line {
            let mut show = false;
            ui.horizontal_wrapped(|ui| {
                theme::error(ui, error);
                show = self.scripts.error_place().is_some() && show_link(ui);
            });
            if show {
                self.scripts.go_to_error();
            }
        }
        shown
    }

    /// The Results, Messages and History tabs (R-SQL-85), and the System views open
    /// (R-SQL-78..80), which close.
    pub(crate) fn results_and_messages(&mut self, ui: &mut Ui, around: Around<'_>) {
        if let Tab::View(view) = self.tab
            && !self.system.tabs.contains(&view)
        {
            self.tab = Tab::Results;
        }
        let mut tabs = vec![
            (Tab::Results, "Results"),
            (Tab::Messages, "Messages"),
            (Tab::History, "History"),
        ];
        tabs.extend(self.system.tabs.iter().map(|v| (Tab::View(*v), v.title())));
        ui.horizontal(|ui| {
            theme::segmented(ui, &mut self.tab, &tabs);
            if let Tab::View(view) = self.tab {
                let name = format!("Close {}", view.title());
                let close = ui.add(theme::icon_button(ui, "🗙")).on_hover_text(&name);
                close.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::accesskit::Role::Button, true, &name)
                });
                if close.clicked() {
                    self.system.close_tab(view);
                    self.tab = Tab::Results;
                }
            }
        });
        ui.add_space(4.0);
        match self.tab {
            Tab::Results => self.results(ui, around.files),
            Tab::Messages => self.messages(ui),
            Tab::History => self.history(ui, around.history),
            Tab::View(view) => self.system.tab(ui, view, &mut self.database),
        }
    }

    /// The window's shortcuts (R-SQL-60), taken before the editor sees them: Run, Run all and
    /// Cancel here, the rest in the editor.
    pub(crate) fn keys(&mut self, ui: &Ui, files: Option<&dyn SqlFiles>) {
        for action in self.scripts.take_keys(ui, files) {
            match action {
                keys::SqlAction::Run => self.run_current(),
                keys::SqlAction::RunAll => self.run_all(),
                keys::SqlAction::Cancel => self.cancel(),
                keys::SqlAction::SidePanel => self.toggle_side_panel(),
                // The editor's own: done in `take_keys`, never handed to the window.
                keys::SqlAction::NewScript
                | keys::SqlAction::CloseScript
                | keys::SqlAction::NextScript
                | keys::SqlAction::Open
                | keys::SqlAction::Save
                | keys::SqlAction::Find
                | keys::SqlAction::Replace
                | keys::SqlAction::GoToLine
                | keys::SqlAction::Format
                | keys::SqlAction::UpperCase
                | keys::SqlAction::LowerCase
                | keys::SqlAction::Comment
                | keys::SqlAction::DuplicateLines
                | keys::SqlAction::MoveLinesUp
                | keys::SqlAction::MoveLinesDown
                | keys::SqlAction::Complete
                | keys::SqlAction::SelectNext
                | keys::SqlAction::ZoomIn
                | keys::SqlAction::ZoomOut
                | keys::SqlAction::ZoomReset => {}
            }
        }
    }

    pub(crate) fn run_buttons(&mut self, ui: &mut Ui) {
        let idle = self.waiting.is_none() && self.queue.is_empty();
        ui.horizontal(|ui| {
            // R-SQL-28: says what Run does now.
            let (start, end) = self.scripts.selection();
            let label = if start < end { "Run selection" } else { "Run" };
            let run = icons::button(icons::PLAY, label)
                .shortcut_text(self.scripts.keys.label(keys::SqlAction::Run));
            if icons::add_named(ui, idle, run, label).clicked() {
                self.run_current();
            }
            let all = egui::Button::new("Run all")
                .shortcut_text(self.scripts.keys.label(keys::SqlAction::RunAll));
            if icons::add_named(ui, idle, all, "Run all").clicked() {
                self.run_all();
            }
            let cancel = icons::button(icons::STOP, "Cancel").shortcut_text("Esc");
            if icons::add_named(ui, !idle, cancel, "Cancel").clicked() {
                self.cancel();
            }
            if !idle {
                ui.spinner();
                match (self.transfer.progress(), self.upload.progress()) {
                    (Some(rows), _) => theme::hint(ui, format!("Downloading… {rows} rows")),
                    (None, Some((sent, of))) => {
                        theme::hint(ui, format!("Uploading… {sent} of {of} rows"));
                    }
                    (None, None) => theme::hint(ui, "Running…"),
                }
            }
            theme::button_row(ui, |ui| {
                // R-SQL-25: what Run would run, kept as a snippet.
                let current = self.scripts.current_statement();
                let save = icons::button(icons::BOOKMARK_SIMPLE, "Save as snippet…");
                let clicked = icons::add_named(ui, current.is_some(), save, "Save as snippet…")
                    .on_hover_text("The selection, or the statement under the cursor")
                    .clicked();
                if clicked && let Some(sql) = current {
                    self.snippets.save_as(&sql);
                    self.show_snippets();
                }
            });
        });
    }

    /// The selection, or the statement under the cursor (R-SQL-7).
    pub(crate) fn run_current(&mut self) {
        if self.waiting.is_some() || !self.queue.is_empty() {
            return;
        }
        if let Some(statement) = self.scripts.current_statement() {
            self.queue.push_back(statement);
            self.next_statement();
        }
    }

    /// Every statement in the editor, in order, stopping at the first error.
    pub(crate) fn run_all(&mut self) {
        if self.waiting.is_some() || !self.queue.is_empty() {
            return;
        }
        let script = self.scripts.text();
        let statements: Vec<String> = text::statements(script)
            .into_iter()
            .filter_map(|r| script.get(r).map(str::to_owned))
            .collect();
        self.queue.extend(statements);
        self.next_statement();
    }

    /// Runs the next statement queued, after asking when it changes much (R-SQL-9).
    pub(crate) fn next_statement(&mut self) {
        if self.waiting.is_some() || self.confirm.is_some() || self.parameters.is_some() {
            return;
        }
        let Some(statement) = self.queue.pop_front() else {
            return;
        };
        if let Some(risk) = text::risk(&statement) {
            self.confirm = Some((statement, risk));
            return;
        }
        self.start(statement);
    }

    /// Runs `statement`, after asking for its markers' values when it has any (R-SQL-40).
    pub(crate) fn start(&mut self, statement: String) {
        self.error_line = None;
        self.scripts.clear_error_mark();
        if text::cl_command(&statement).is_none()
            && let Some(markers) = params::markers(&statement)
        {
            let remembered = self.param_values.get(&statement);
            self.parameters = Some(parameters::ParamForm::new(statement, markers, remembered));
            return;
        }
        let run = self.run_request(&statement);
        self.send(run, Waiting::Run(statement));
    }

    /// The request that runs `statement`, with the rows at a time and the timeout (R-SQL-18).
    /// A `CL:` command runs through `QSYS2.QCMDEXC` (R-SQL-30).
    pub(crate) fn run_request(&self, statement: &str) -> Request {
        let sql = text::cl_command(statement).map_or_else(|| statement.to_owned(), text::cl_call);
        Request::Run {
            sql,
            max_rows: self.rows,
            timeout: self.timeout,
            params: Vec::new(),
        }
    }

    /// The parameter form while open: Run sends the marked statement with the values given
    /// and remembers them; Cancel stops what was queued (R-SQL-40).
    pub(crate) fn parameters_dialog(&mut self, ctx: &egui::Context) {
        let Some(form) = &mut self.parameters else {
            return;
        };
        let Some(outcome) = form.show(ctx) else {
            return;
        };
        let Some(form) = self.parameters.take() else {
            return;
        };
        match outcome {
            parameters::Outcome::Run(values) => {
                self.param_values
                    .insert(form.statement.clone(), form.values.clone());
                let run = Request::Run {
                    sql: form.markers.sql,
                    max_rows: self.rows,
                    timeout: self.timeout,
                    params: values,
                };
                self.send(run, Waiting::Run(form.statement));
            }
            parameters::Outcome::Cancel => self.queue.clear(),
        }
    }

    pub(crate) fn cancel(&mut self) {
        self.queue.clear();
        self.cancel_download();
        self.cancel_upload();
        if let Some(runner) = &self.runner
            && self.waiting.is_some()
        {
            runner.cancel();
        }
    }

    /// Whether a statement, a download or an upload is under way, or a statement waits for
    /// its confirmation or its parameters: nothing else may be sent until it ends.
    pub(crate) fn busy(&self) -> bool {
        self.waiting.is_some()
            || !self.queue.is_empty()
            || self.confirm.is_some()
            || self.parameters.is_some()
            || self.transfer.running.is_some()
            || self.upload.running.is_some()
    }

    /// Sends `request`, unless another is in flight: there is one place for the answer waited
    /// for, and a second request would take it, losing the first's answer. True when sent.
    pub(crate) fn send(&mut self, request: Request, waiting: Waiting) -> bool {
        if self.waiting.is_some() {
            self.fail(WAIT.to_owned());
            return false;
        }
        if matches!(waiting, Waiting::Run(_)) {
            self.run_started = Some(std::time::Instant::now());
        }
        if let Some(runner) = &mut self.runner {
            let tag = runner.send(request);
            self.waiting = Some((tag, waiting));
            return true;
        }
        false
    }

    pub(crate) fn results(&mut self, ui: &mut Ui, saver: Option<&dyn SqlFiles>) {
        let idle = self.waiting.is_none();
        let asked = self.results.show(ui, idle, &mut self.header, saver);
        match asked {
            Some(results::Ask::Fetch(cursor)) => {
                let fetch = Request::Fetch {
                    cursor,
                    max_rows: self.rows,
                };
                self.send(fetch, Waiting::Fetch(cursor));
            }
            Some(results::Ask::Download) => self.transfer.ask(download::Source::Tab),
            None => {}
        }
    }
}
