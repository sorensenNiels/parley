//! The System node's views (R-SQL-78..80, D32): the signed-on user's spooled files, message
//! queue and jobs, read on the second connection (D21) and listed in tabs under the editor in
//! the result grid; a spooled file's text and a job log open in viewer windows of their own.
//! Nothing changes on the host.

use std::collections::BTreeMap;

use egui::{ScrollArea, TextEdit, Ui, ViewportBuilder, ViewportId};
use parley_sql_core::protocol::{Column, Row};
use parley_sql_core::services::{self, View};

use super::SqlFiles;
use super::database::{Answered, Ask, Database};
use super::editor::find_next;
use super::grid::ResultView;
use parley_ui_egui::{icons, theme};

/// A list, being read, read, or not readable.
#[derive(Debug)]
enum Listed {
    Reading,
    Ready(Box<ResultView>),
    Failed(String),
}

/// A viewer's text.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Text {
    Reading,
    Ready(String),
    Failed(String),
}

/// A viewer window: a spooled file's text or a job log (R-SQL-78, R-SQL-80).
#[derive(Debug)]
struct Viewer {
    id: u64,
    /// What it shows: a spooled file's text, or a job's job log.
    view: View,
    title: String,
    text: Text,
    find: String,
    /// Where the next Find starts, a byte in the text.
    from: usize,
    /// The match to select on the next frame, in characters.
    select: Option<(usize, usize)>,
    note: Option<String>,
    closed: bool,
}

/// What a click in the System node asks for (R-SQL-78..80).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemPick {
    /// Show the view's tab, opening it.
    Show(View),
    /// Read the lists again.
    Refresh,
}

/// The System node's views and viewers.
#[derive(Debug, Default)]
pub struct System {
    lists: BTreeMap<View, Listed>,
    /// The views open as tabs under the editor, in the order opened.
    pub(super) tabs: Vec<View>,
    /// The time of the newest message seen in the Message queue tab (R-SQL-79).
    seen: Option<String>,
    viewers: Vec<Viewer>,
    next_viewer: u64,
}

/// The column the window adds to Jobs, marking its own two (R-SQL-80).
const WINDOW_COLUMN: &str = "This window";

fn column(heading: &str, numeric: bool) -> Column {
    Column {
        name: heading.to_owned(),
        type_name: if numeric { "DECIMAL" } else { "VARCHAR" }.to_owned(),
        length: 0,
        scale: 0,
        nullable: true,
        numeric,
        field: None,
        schema: None,
        table: None,
        heading: None,
        text: None,
    }
}

/// The grid for `view`'s `rows`; Jobs gets a first column naming the window's `own` jobs:
/// the SQL job, then the catalog job.
fn list_view(view: View, mut rows: Vec<Row>, own: [Option<&str>; 2]) -> ResultView {
    let mut columns: Vec<Column> = view
        .columns()
        .iter()
        .map(|c| column(c.heading, c.numeric))
        .collect();
    if view == View::Jobs {
        columns.insert(0, column(WINDOW_COLUMN, false));
        for row in &mut rows {
            let name = row
                .get(services::jobs::NAME)
                .and_then(Option::as_deref)
                .unwrap_or_default();
            let mark = if Some(name) == own[0] {
                Some("SQL job".to_owned())
            } else if Some(name) == own[1] {
                Some("Catalog job".to_owned())
            } else {
                None
            };
            row.insert(0, mark);
        }
    }
    ResultView::new(columns, rows, false, 0)
}

/// The cell `n` of `row` as text.
fn cell(row: &Row, n: usize) -> String {
    row.get(n).cloned().flatten().unwrap_or_default()
}

impl System {
    /// Whether the lists were asked for in this window.
    pub fn is_read(&self) -> bool {
        !self.lists.is_empty()
    }

    /// Reads every list again (R-SQL-78..80).
    pub fn read(&mut self, db: &mut Database) {
        for view in View::ALL {
            self.lists.insert(view, Listed::Reading);
            db.ask(Ask::Service(view));
        }
    }

    /// Takes the answers that are the System node's, given the window's `own` jobs; the rest
    /// are returned for the schema browser.
    pub fn receive(&mut self, answers: Vec<Answered>, own: [Option<&str>; 2]) -> Vec<Answered> {
        let mut rest = Vec::new();
        for (ask, rows) in answers {
            match ask {
                Ask::Service(view) => {
                    let listed = match rows {
                        Ok(rows) => Listed::Ready(Box::new(list_view(view, rows, own))),
                        Err(why) => Listed::Failed(why),
                    };
                    self.lists.insert(view, listed);
                }
                Ask::Text { viewer, .. } => {
                    if let Some(v) = self.viewers.iter_mut().find(|v| v.id == viewer) {
                        let lines = rows.as_ref().map_or(0, Vec::len);
                        v.text = match (rows, v.view) {
                            (Ok(rows), View::Jobs) => Text::Ready(services::job_log_lines(&rows)),
                            (Ok(rows), View::SpooledFiles | View::MessageQueue) => {
                                Text::Ready(services::spooled_lines(&rows))
                            }
                            (Err(why), _) => Text::Failed(why),
                        };
                        if lines >= services::MAX_LINES {
                            v.note = Some(format!("The first {} lines only.", services::MAX_LINES));
                        }
                    }
                }
                Ask::Objects(_)
                | Ask::Columns { .. }
                | Ask::Parameters { .. }
                | Ask::Ddl { .. } => rest.push((ask, rows)),
            }
        }
        rest
    }

    /// The view's name in the tree: with its count once read; Message queue with how many are
    /// new (R-SQL-79).
    pub fn label(&self, view: View) -> String {
        let title = view.title();
        match self.lists.get(&view) {
            Some(Listed::Ready(list)) if view == View::MessageQueue => {
                let new = services::newer_than(&list.rows, self.seen.as_deref());
                if new > 0 {
                    format!("{title} ({new} new)")
                } else {
                    title.to_owned()
                }
            }
            Some(Listed::Ready(list)) => format!("{title} ({})", list.rows.len()),
            Some(Listed::Reading) => format!("{title} …"),
            Some(Listed::Failed(_)) | None => title.to_owned(),
        }
    }

    /// Opens `view`'s tab, if it is not open.
    pub fn open_tab(&mut self, view: View) {
        if !self.tabs.contains(&view) {
            self.tabs.push(view);
        }
    }

    pub fn close_tab(&mut self, view: View) {
        self.tabs.retain(|v| *v != view);
    }

    /// `view`'s tab: Refresh, Open, the filter and the grid; the message queue shows the help
    /// of the message selected (R-SQL-79).
    pub fn tab(&mut self, ui: &mut Ui, view: View, db: &mut Database) {
        let mut refresh = false;
        let mut open = None;
        ui.horizontal(|ui| {
            let again = icons::button(icons::ARROWS_CLOCKWISE, "Refresh");
            refresh = icons::add_named(ui, true, again, "Refresh")
                .on_hover_text("Read the lists again")
                .clicked();
            if let Some(Listed::Ready(list)) = self.lists.get_mut(&view) {
                if view.opens() {
                    let row = list.selected_row().cloned();
                    let label = match view {
                        View::SpooledFiles => "Open",
                        View::Jobs => "Job log",
                        View::MessageQueue => "",
                    };
                    if icons::add_named(ui, row.is_some(), egui::Button::new(label), label)
                        .on_hover_text("Or double-click a row")
                        .clicked()
                    {
                        open = row;
                    }
                }
                ui.add(
                    TextEdit::singleline(&mut list.filter)
                        .hint_text("Filter")
                        .desired_width(220.0),
                );
                let (shown, all) = list.counts();
                theme::hint(ui, format!("{shown} of {all}"));
            }
        });
        ui.add_space(4.0);
        match self.lists.get_mut(&view) {
            None | Some(Listed::Reading) => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    theme::hint(ui, "Reading…");
                });
            }
            Some(Listed::Failed(why)) => theme::error(ui, why.as_str()),
            Some(Listed::Ready(list)) => {
                if view == View::MessageQueue {
                    if let Some(newest) = services::newest(&list.rows) {
                        self.seen = Some(newest);
                    }
                    if let Some(row) = list.selected_row() {
                        let help = cell(row, services::messages::HELP);
                        egui::Panel::bottom("sql-message-help")
                            .resizable(true)
                            .default_size(90.0)
                            .show(ui, |ui| {
                                ScrollArea::vertical().show(ui, |ui| {
                                    ui.label(parley_sql_core::message::second_level(&help));
                                });
                            });
                    }
                }
                list.show(ui);
                // A double-click or Enter opens the row, as Open does.
                if view.opens()
                    && let Some(row) = list.detail.take().and_then(|i| list.rows.get(i))
                {
                    open = Some(row.clone());
                }
            }
        }
        if refresh {
            self.read(db);
        }
        if let Some(row) = open {
            self.open_entry(view, &row, db);
        }
    }

    /// Opens a spooled file's text or a job's job log in a viewer window.
    fn open_entry(&mut self, view: View, row: &Row, db: &mut Database) {
        let (title, sql) = match view {
            View::SpooledFiles => {
                let (name, job) = (
                    cell(row, services::spooled::NAME),
                    cell(row, services::spooled::JOB),
                );
                let number = cell(row, services::spooled::NUMBER);
                (
                    format!("{name} ({number}) of {job}"),
                    services::spooled_file_text(&job, &name, &number).map_err(|e| e.to_string()),
                )
            }
            // Jobs has the window's column first.
            View::Jobs => {
                let job = cell(row, services::jobs::NAME + 1);
                (format!("Job log of {job}"), Ok(services::job_log(&job)))
            }
            View::MessageQueue => return,
        };
        self.next_viewer += 1;
        let id = self.next_viewer;
        let text = match sql {
            Ok(sql) => {
                db.ask(Ask::Text { sql, viewer: id });
                Text::Reading
            }
            Err(why) => Text::Failed(why),
        };
        self.viewers.push(Viewer {
            id,
            view,
            title,
            text,
            find: String::new(),
            from: 0,
            select: None,
            note: None,
            closed: false,
        });
    }

    /// The viewer windows open (R-SQL-78, R-SQL-80).
    pub fn viewers(&mut self, ctx: &egui::Context, files: Option<&dyn SqlFiles>) {
        for viewer in &mut self.viewers {
            let builder = ViewportBuilder::default()
                .with_title(&viewer.title)
                .with_inner_size([820.0, 600.0])
                .with_min_inner_size([360.0, 240.0]);
            ctx.show_viewport_immediate(
                ViewportId::from_hash_of(("parley-sql-viewer", viewer.id)),
                builder,
                |ui, _class| {
                    if ui.ctx().input(|i| i.viewport().close_requested()) {
                        viewer.closed = true;
                    }
                    viewer.show(ui, files);
                },
            );
        }
        self.viewers.retain(|v| !v.closed);
    }
}

impl Viewer {
    fn show(&mut self, ui: &mut Ui, files: Option<&dyn SqlFiles>) {
        let top = theme::bar(ui, egui::Margin::symmetric(12, 8));
        egui::Panel::top("sql-viewer-top")
            .frame(top)
            .show(ui, |ui| self.bar(ui, files));
        egui::CentralPanel::default().show(ui, |ui| match &self.text {
            Text::Reading => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    theme::hint(ui, "Reading…");
                });
            }
            Text::Failed(why) => theme::error(ui, why.as_str()),
            Text::Ready(text) => {
                let id = ui.make_persistent_id(("sql-viewer-text", self.id));
                if let Some((start, end)) = self.select.take() {
                    let mut state = TextEdit::load_state(ui.ctx(), id).unwrap_or_default();
                    state
                        .cursor
                        .set_char_range(Some(egui::text::CCursorRange::two(
                            egui::text::CCursor::new(start),
                            egui::text::CCursor::new(end),
                        )));
                    state.store(ui.ctx(), id);
                    ui.memory_mut(|m| m.request_focus(id));
                }
                ScrollArea::both()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let mut shown = text.as_str();
                        ui.add(
                            TextEdit::multiline(&mut shown)
                                .id(id)
                                .font(egui::TextStyle::Monospace)
                                .code_editor()
                                .desired_width(f32::INFINITY),
                        );
                    });
            }
        });
    }

    /// Find, Copy and Save as… (R-SQL-78).
    fn bar(&mut self, ui: &mut Ui, files: Option<&dyn SqlFiles>) {
        ui.horizontal(|ui| {
            ui.label(theme::semibold(&self.title));
            let Text::Ready(text) = &self.text else {
                return;
            };
            let text = text.clone();
            theme::button_row(ui, |ui| {
                let save = icons::button(icons::DOWNLOAD_SIMPLE, "Save as…");
                if icons::add_named(ui, files.is_some(), save, "Save as…").clicked()
                    && let Some(files) = files
                {
                    let suggested = format!("{}.txt", file_name(&self.title));
                    self.note = match files.save_text(&suggested, &text) {
                        Ok(Some(path)) => Some(format!("Saved to {path}.")),
                        Ok(None) => self.note.take(),
                        Err(why) => Some(format!("Not saved: {why}")),
                    };
                }
                let copy = icons::button(icons::COPY, "Copy");
                if icons::add_named(ui, true, copy, "Copy")
                    .on_hover_text("The selection, or all of it")
                    .clicked()
                {
                    ui.ctx().copy_text(text.clone());
                }
                let next = icons::add_named(ui, true, egui::Button::new("Next"), "Next").clicked();
                let field = ui.add(
                    TextEdit::singleline(&mut self.find)
                        .hint_text("Find")
                        .desired_width(200.0),
                );
                let entered = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if next || entered {
                    self.find_next(&text);
                    if entered {
                        field.request_focus();
                    }
                }
            });
        });
        if let Some(note) = &self.note {
            theme::hint(ui, note.as_str());
        }
    }

    /// Selects the next match of the find field, from the last one, round to the start.
    fn find_next(&mut self, text: &str) {
        let found = find_next(text, &self.find, self.from, false)
            .or_else(|| find_next(text, &self.find, 0, false));
        match found {
            Some(range) => {
                let chars = |byte: usize| text.get(..byte).map_or(0, |s| s.chars().count());
                self.select = Some((chars(range.start), chars(range.end)));
                self.from = range.end;
                self.note = None;
            }
            None if self.find.is_empty() => {}
            None => self.note = Some(format!("\"{}\" is not there.", self.find)),
        }
    }
}

/// `title` as a file name: letters, digits, `-` and `_`, the rest as `_`.
fn file_name(title: &str) -> String {
    title
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(cells: &[&str]) -> Row {
        cells.iter().map(|c| Some((*c).to_owned())).collect()
    }

    fn answered(view: View, rows: Vec<Row>) -> Answered {
        (Ask::Service(view), Ok(rows))
    }

    /// R-SQL-80: the window's own jobs are marked in a column of their own.
    #[test]
    fn the_windows_own_jobs_are_marked_r_sql_80() {
        let mut s = System::default();
        let rows = vec![
            row(&[
                "1/QUSER/QZDASOINIT",
                "QUSRWRK",
                "BCI",
                "ACTIVE",
                "12",
                "PGM-x",
            ]),
            row(&["2/QUSER/QZDASOINIT", "QUSRWRK", "BCI", "ACTIVE", "3", ""]),
            row(&["3/ÆØÅ/QPADEV0001", "QINTER", "INT", "DSPW", "1", ""]),
        ];
        let rest = s.receive(
            vec![answered(View::Jobs, rows)],
            [Some("2/QUSER/QZDASOINIT"), Some("1/QUSER/QZDASOINIT")],
        );
        assert_eq!(rest, []);
        let Some(Listed::Ready(list)) = s.lists.get(&View::Jobs) else {
            panic!("read");
        };
        assert_eq!(list.columns[0].name, WINDOW_COLUMN);
        assert_eq!(list.rows[0][0].as_deref(), Some("Catalog job"));
        assert_eq!(list.rows[1][0].as_deref(), Some("SQL job"));
        assert_eq!(list.rows[2][0], None);
        assert_eq!(s.label(View::Jobs), "Jobs (3)");
    }

    /// R-SQL-79: the label counts the messages newer than the newest seen.
    #[test]
    fn the_message_queue_says_how_many_are_new_r_sql_79() {
        let mut s = System::default();
        assert_eq!(s.label(View::MessageQueue), "Message queue");
        let message = |time: &str| row(&[time, "CPF9898", "INFO", "Hej æøå", "QSYS", "&N Help"]);
        s.receive(
            vec![answered(
                View::MessageQueue,
                vec![
                    message("2026-11-15 10:00:02"),
                    message("2026-11-15 10:00:01"),
                ],
            )],
            [None, None],
        );
        assert_eq!(s.label(View::MessageQueue), "Message queue (2 new)");
        s.seen = Some("2026-11-15 10:00:01".to_owned());
        assert_eq!(s.label(View::MessageQueue), "Message queue (1 new)");
        s.seen = Some("2026-11-15 10:00:02".to_owned());
        assert_eq!(s.label(View::MessageQueue), "Message queue");
    }

    /// R-SQL-78: a list that cannot be read says why; the browser's answers pass on.
    #[test]
    fn a_failed_list_says_why_and_other_answers_pass_on_r_sql_78() {
        let mut s = System::default();
        let rest = s.receive(
            vec![
                (
                    Ask::Service(View::SpooledFiles),
                    Err("SQL0551 æøå".to_owned()),
                ),
                (Ask::Objects("QGPL".to_owned()), Ok(Vec::new())),
            ],
            [None, None],
        );
        assert_eq!(rest.len(), 1);
        assert!(
            matches!(s.lists.get(&View::SpooledFiles), Some(Listed::Failed(w)) if w.contains("æøå"))
        );
        assert_eq!(s.label(View::SpooledFiles), "Spooled files");
    }

    /// R-SQL-78: opening a spooled file asks for its text by job, name and number, and the
    /// answer becomes the viewer's text; a number that is not one is said, not asked.
    #[test]
    fn a_spooled_file_opens_in_a_viewer_with_its_text_r_sql_78() {
        let mut s = System::default();
        let mut db = Database::default();
        let entry = row(&["QSYSPRT", "", "9/USR/QPADEV0001", "2", "1", "READY", "2026"]);
        s.open_entry(View::SpooledFiles, &entry, &mut db);
        assert_eq!(s.viewers.len(), 1);
        assert_eq!(s.viewers[0].title, "QSYSPRT (2) of 9/USR/QPADEV0001");
        assert_eq!(s.viewers[0].text, Text::Reading);
        let id = s.viewers[0].id;
        s.receive(
            vec![(
                Ask::Text {
                    sql: String::new(),
                    viewer: id,
                },
                Ok(vec![row(&["Side 1"]), row(&["Løn  "])]),
            )],
            [None, None],
        );
        assert_eq!(s.viewers[0].text, Text::Ready("Side 1\nLøn".to_owned()));
        let bad = row(&["QSYSPRT", "", "9/USR/QPADEV0001", "x", "1", "READY", "2026"]);
        s.open_entry(View::SpooledFiles, &bad, &mut db);
        assert!(matches!(&s.viewers[1].text, Text::Failed(w) if w.contains("\"x\"")));
    }

    /// R-SQL-78: the System node lists the views with counts; a click opens the list as a tab
    /// under the editor, Open shows the spooled file in a viewer window, and the tab closes.
    #[test]
    fn a_view_opens_from_the_tree_as_a_tab_and_a_spooled_file_in_a_viewer_r_sql_78() {
        use egui_kittest::kittest::Queryable;

        use super::super::database::State;
        use super::super::tests::{drawn, window};

        let mut w = window();
        w.database.state = State::Ready {
            job: "1/QUSER/QZDASOINIT".to_owned(),
        };
        w.side.libraries_open = true;
        let spooled = vec![row(&[
            "QSYSPRT",
            "Løn æøå",
            "9/USR/QPADEV0001",
            "2",
            "3",
            "READY",
            "2026",
        ])];
        w.system.receive(
            vec![
                answered(View::SpooledFiles, spooled),
                answered(View::MessageQueue, Vec::new()),
                answered(View::Jobs, Vec::new()),
            ],
            [None, None],
        );
        let mut harness = drawn(w, Vec::new());
        harness.get_by_label("System").click();
        harness.run();
        harness.get_by_label("Spooled files (1)").click();
        harness.run();
        harness.get_by_label("Close Spooled files");
        harness.get_by_label("Løn æøå");
        // A row is selected by its number (R-SQL-16).
        harness.get_by_label("1").click();
        harness.run();
        harness.get_by_label("Open").click();
        // The viewer's spinner repaints while its text is read.
        harness.run_steps(3);
        assert_eq!(harness.state().system.viewers.len(), 1, "a viewer window");
        let title = "QSYSPRT (2) of 9/USR/QPADEV0001";
        assert!(
            harness.query_all_by_label(title).count() > 0,
            "the viewer shows"
        );
        assert!(
            harness.state().side_layout().system_open,
            "kept in the settings"
        );
        // In a test the viewer is drawn in the window, over the tab: it is closed first.
        harness.state_mut().system.viewers.clear();
        harness.run_steps(2);
        harness.get_by_label("Close Spooled files").click();
        harness.run_steps(3);
        assert!(harness.query_by_label("Close Spooled files").is_none());
        assert_eq!(harness.state().system.tabs, []);
    }

    /// R-SQL-78: Find selects the next match, round to the start, and says when there is none.
    #[test]
    fn find_in_a_viewer_goes_round_and_says_when_nothing_is_there_r_sql_78() {
        let mut v = Viewer {
            id: 1,
            view: View::SpooledFiles,
            title: "T".to_owned(),
            text: Text::Reading,
            find: "løn".to_owned(),
            from: 0,
            select: None,
            note: None,
            closed: false,
        };
        let text = "æøå LØN og løn";
        v.find_next(text);
        assert_eq!(v.select, Some((4, 7)), "ignoring case, in characters");
        v.find_next(text);
        assert_eq!(v.select, Some((11, 14)));
        v.find_next(text);
        assert_eq!(v.select, Some((4, 7)), "round to the start");
        v.find = "skat".to_owned();
        v.find_next(text);
        assert!(v.note.as_deref().is_some_and(|n| n.contains("skat")));
        assert_eq!(file_name("Job log of 1/A B/C"), "Job_log_of_1_A_B_C");
    }
}
