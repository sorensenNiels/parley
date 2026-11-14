//! The SQL window's results (R-SQL-14..16): a tab per query, the newest shown, up to
//! [`MAX_TABS`]; under the tabs a filter, the grid, the bar with the counts, Fetch more, Copy and
//! Export, and the row detail beside the grid.

use egui::{Panel, RichText, ScrollArea, Ui};
use parley_core::sql::protocol::Row;

use super::SqlFiles;
use parley_core::ebcdic::CodePage;
use parley_core::sql::{compare, export};

use super::chart;
use super::grid::{CopyFormat, HeaderMode, ResultView};
use crate::{icons, theme};

/// Result tabs kept; the oldest closes first.
pub const MAX_TABS: usize = 10;

/// One query's result in a tab.
#[derive(Debug, Clone, PartialEq)]
struct ResultTab {
    /// The query's first words.
    title: String,
    view: ResultView,
}

/// The result tabs.
#[derive(Debug, Default)]
pub struct Results {
    tabs: Vec<ResultTab>,
    /// The tab shown, an index into `tabs`.
    shown: usize,
    /// Said in the bar after Copy or Export.
    note: Option<String>,
    /// The code page binary values are read in, in the row detail (R-SQL-39).
    pub code_page: CodePage,
    /// The key columns a comparison matches rows by, by name; empty for the first (R-SQL-41).
    compare_keys: Vec<String>,
}

/// What the results area asks of the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ask {
    /// Fetch more rows from this cursor.
    Fetch(u64),
    /// Download every row of the tab shown (R-SQL-71).
    Download,
}

impl Results {
    /// No results yet; binary values are read in `code_page`, the session's (R-SQL-39).
    pub fn reading(code_page: CodePage) -> Self {
        Self {
            code_page,
            ..Self::default()
        }
    }

    /// A new tab for `statement`'s result, shown at once (R-SQL-14).
    pub fn add(&mut self, statement: &str, view: ResultView) {
        if self.tabs.len() == MAX_TABS {
            self.tabs.remove(0);
        }
        self.tabs.push(ResultTab {
            title: title(statement),
            view,
        });
        self.shown = self.tabs.len() - 1;
        self.note = None;
    }

    /// Rows fetched from `cursor`, into the tab that has it.
    pub fn append(&mut self, cursor: u64, rows: Vec<Row>, more: bool) {
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.view.cursor == Some(cursor)) {
            tab.view.append(rows, more);
        }
    }

    /// The columns of the tab shown.
    pub fn shown_columns(&self) -> Option<&[parley_core::sql::protocol::Column]> {
        self.tabs.get(self.shown).map(|t| t.view.columns.as_slice())
    }

    /// For a download of the tab shown (R-SQL-71): its columns, its rows in the host's order,
    /// and the cursor the rest comes from, which the download takes: the grid fetches no more.
    pub fn take_for_download(
        &mut self,
    ) -> Option<(
        Vec<parley_core::sql::protocol::Column>,
        Vec<Row>,
        Option<u64>,
    )> {
        let view = &mut self.tabs.get_mut(self.shown)?.view;
        Some((view.columns.clone(), view.rows.clone(), view.cursor.take()))
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.tabs.is_empty()
    }

    /// The tab shown.
    #[cfg(test)]
    pub fn shown_view(&self) -> Option<&ResultView> {
        self.tabs.get(self.shown).map(|t| &t.view)
    }

    fn close(&mut self, index: usize) {
        if index < self.tabs.len() {
            self.tabs.remove(index);
            self.shown = self.shown.min(self.tabs.len().saturating_sub(1));
        }
    }

    /// Draws the tabs and the shown result; `idle` when no request is in flight.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        idle: bool,
        header: &mut HeaderMode,
        saver: Option<&dyn SqlFiles>,
    ) -> Option<Ask> {
        if self.tabs.is_empty() {
            theme::hint(ui, "Run a query to see its rows here.");
            return None;
        }
        self.tab_strip(ui);
        let shown = self.shown;
        let tab = self.tabs.get_mut(shown)?;
        let view = &mut tab.view;
        view.header = *header;
        ui.horizontal(|ui| {
            ui.label(icons::icon(icons::MAGNIFYING_GLASS).color(theme::palette(ui).muted));
            let field =
                egui::TextEdit::singleline(&mut view.filter).hint_text("Filter the rows fetched");
            theme::sized_edit(ui, 260.0, field)
                .on_hover_text("Rows that hold this text in any column, ignoring case");
            // R-SQL-29: what the column headings say.
            theme::button_row(ui, |ui| {
                // R-SQL-42: the rows as a chart instead of the grid.
                let mut charted = view.chart.is_some();
                if theme::segmented(ui, &mut charted, &[(false, "Table"), (true, "Chart")]) {
                    view.chart = charted.then(|| chart::Chart::new(&view.columns));
                }
                theme::segmented(
                    ui,
                    header,
                    &[
                        (HeaderMode::Names, "Names"),
                        (HeaderMode::Headings, "Headings"),
                        (HeaderMode::Both, "Both"),
                    ],
                );
            });
        });
        let mut ask = None;
        let note = &mut self.note;
        Panel::bottom("sql-grid-actions").show(ui, |ui| {
            ask = bar(ui, view, idle, saver, note);
        });
        if let Some(mut shown) = view.chart.take() {
            {
                let rows = view.shown_rows();
                chart::show(ui, &mut shown, &view.columns, &rows);
            }
            view.chart = Some(shown);
            return ask;
        }
        if view.detail.is_some() {
            let page = &mut self.code_page;
            Panel::right("sql-detail")
                .resizable(true)
                .default_size(320.0)
                .show(ui, |ui| detail(ui, view, page));
        }
        view.show(ui);
        ask
    }

    /// The tabs, newest last, and a button that closes the one shown.
    fn tab_strip(&mut self, ui: &mut Ui) {
        let items: Vec<(usize, String, Option<String>)> = self
            .tabs
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let (_, fetched) = t.view.counts();
                let more = if t.view.cursor.is_some() { "+" } else { "" };
                (i, t.title.clone(), Some(format!("{fetched}{more}")))
            })
            .collect();
        let mut close = false;
        ui.horizontal(|ui| {
            // Room for "Compare with…" and the close button.
            let width = (ui.available_width() - 150.0).max(120.0);
            ui.allocate_ui(egui::vec2(width, 0.0), |ui| {
                ScrollArea::horizontal()
                    .id_salt("sql-result-tabs")
                    .show(ui, |ui| {
                        theme::tabs(ui, &mut self.shown, &items);
                    });
            });
            self.compare_menu(ui);
            let button = theme::icon_button(ui, "🗙");
            close = ui.add(button).on_hover_text("Close this result").clicked();
        });
        if close {
            self.close(self.shown);
        }
    }

    /// "Compare with…": the key columns, then the other results with the same columns; the
    /// comparison opens as a result of its own (R-SQL-41).
    fn compare_menu(&mut self, ui: &mut Ui) {
        let Some(shown) = self.tabs.get(self.shown) else {
            return;
        };
        let names: Vec<String> = shown.view.columns.iter().map(|c| c.name.clone()).collect();
        let mut sorted = names.clone();
        sorted.sort();
        let others: Vec<(usize, String)> = self
            .tabs
            .iter()
            .enumerate()
            .filter(|&(i, t)| {
                let mut theirs: Vec<&String> = t.view.columns.iter().map(|c| &c.name).collect();
                theirs.sort();
                i != self.shown && theirs.into_iter().eq(sorted.iter())
            })
            .map(|(i, t)| (i, t.title.clone()))
            .collect();
        self.compare_keys.retain(|k| names.contains(k));
        let mut chosen = None;
        ui.menu_button("Compare with…", |ui| {
            theme::section_label(ui, "Key columns");
            for name in &names {
                let first = self.compare_keys.is_empty() && names.first() == Some(name);
                let mut on = first || self.compare_keys.contains(name);
                // Ticking another column replaces the first as the default key.
                if ui.checkbox(&mut on, name.as_str()).changed() {
                    if on {
                        self.compare_keys.push(name.clone());
                    } else {
                        self.compare_keys.retain(|k| k != name);
                    }
                    self.compare_keys.dedup();
                }
            }
            ui.separator();
            theme::section_label(ui, "With");
            if others.is_empty() {
                theme::hint(ui, "No other result has the same columns.");
            }
            for (i, title) in &others {
                if ui.button(title.as_str()).clicked() {
                    chosen = Some(*i);
                    ui.close();
                }
            }
        });
        if let Some(other) = chosen {
            self.compare_with(other);
        }
    }

    /// The shown result compared with result `other`, as a result of its own.
    fn compare_with(&mut self, other: usize) {
        let (Some(first), Some(second)) = (self.tabs.get(self.shown), self.tabs.get(other)) else {
            return;
        };
        let compared = compare::compare(
            (&first.view.columns, &first.view.rows),
            (&second.view.columns, &second.view.rows),
            &self.compare_keys,
        );
        let title = format!("{} vs {}", first.title, second.title);
        match compared {
            Ok(c) => {
                let (same, differ) = (c.same, c.rows.len());
                let mut view = ResultView::new(c.columns, c.rows, false, 0);
                view.marked = c.marked.into_iter().collect();
                self.add("", view);
                if let Some(tab) = self.tabs.last_mut() {
                    tab.title = title;
                }
                self.note = Some(format!("{same} rows the same, {differ} rows listed."));
            }
            Err(compare::Mismatch::Columns) => {
                self.note = Some("The results have different columns.".to_owned());
            }
            Err(compare::Mismatch::Key) => {
                self.note = Some("Choose a key column the results have.".to_owned());
            }
        }
    }
}

/// The bar under the grid: how many rows, Fetch more, Copy and Export.
fn bar(
    ui: &mut Ui,
    view: &ResultView,
    idle: bool,
    saver: Option<&dyn SqlFiles>,
    note: &mut Option<String>,
) -> Option<Ask> {
    let mut ask = None;
    ui.horizontal(|ui| {
        let (shown, fetched) = view.counts();
        let more = if view.cursor.is_some() {
            ", more to fetch"
        } else {
            ""
        };
        let counts = if shown == fetched {
            format!("{fetched} rows{more}")
        } else {
            format!("{shown} of {fetched} rows shown{more}")
        };
        theme::hint(ui, counts);
        if let Some(cursor) = view.cursor
            && ui
                .add_enabled(idle, egui::Button::new("Fetch more"))
                .clicked()
        {
            ask = Some(Ask::Fetch(cursor));
        }
        theme::button_row(ui, |ui| {
            if let Some(saver) = saver {
                // R-SQL-71: every row, not only those fetched, to CSV or Excel.
                let download = icons::button(icons::DOWNLOAD_SIMPLE, "Download…");
                if icons::add_named(ui, idle, download, "Download…")
                    .on_hover_text("Every row of the query, to a CSV or Excel file")
                    .clicked()
                {
                    ask = Some(Ask::Download);
                }
                let export = icons::button(icons::EXPORT, "Export…");
                if icons::add_named(ui, true, export, "Export…")
                    .on_hover_text("The rows shown, as CSV")
                    .clicked()
                {
                    *note = Some(match saver.export_csv("query.csv", view.csv().as_bytes()) {
                        Ok(Some(path)) => format!("Saved {path}"),
                        Ok(None) => String::new(),
                        Err(e) => format!("Not saved: {e}"),
                    });
                }
            }
            let what = if view.selected_count() > 0 {
                "Copy rows"
            } else {
                "Copy all"
            };
            let copy = icons::button(icons::COPY, what);
            if icons::add_named(ui, true, copy, what)
                .on_hover_text("As tab-separated text: it pastes into a spreadsheet as columns")
                .clicked()
            {
                ui.ctx().copy_text(view.tsv());
                *note = Some("Copied.".to_owned());
            }
            // R-SQL-38: the same rows in other formats.
            ui.menu_button("Copy as", |ui| {
                for format in CopyFormat::ALL {
                    if ui.button(format.label()).clicked() {
                        ui.ctx().copy_text(view.copy_as(format));
                        *note = Some(format!("Copied as {}.", format.label()));
                        ui.close();
                    }
                }
            });
            if let Some(text) = note.as_deref().filter(|n| !n.is_empty()) {
                theme::hint(ui, text);
            }
        });
    });
    ask
}

/// One row's values one under another (R-SQL-15): name, type and the whole value, each with
/// Copy; Up and Down step through the rows shown.
fn detail(ui: &mut Ui, view: &mut ResultView, page: &mut CodePage) {
    let p = theme::palette(ui);
    let (shown, _) = view.counts();
    let mut step = 0;
    let mut close = false;
    ui.horizontal(|ui| {
        let at = view.detail_row().map_or(0, |(_, at)| at);
        ui.label(theme::semibold(format!("Row {at} of {shown}")));
        theme::button_row(ui, |ui| {
            close = ui
                .add(theme::icon_button(ui, "🗙"))
                .on_hover_text("Close")
                .clicked();
            let down = icons::button(icons::ARROW_DOWN, "Next");
            if icons::add_named(ui, at < shown, down, "Next row").clicked() {
                step = 1;
            }
            let up = icons::button(icons::ARROW_UP, "Previous");
            if icons::add_named(ui, at > 1, up, "Previous row").clicked() {
                step = -1;
            }
        });
    });
    let binary = view.detail_row().is_some_and(|(row, _)| {
        view.columns
            .iter()
            .zip(row)
            .any(|(c, v)| v.is_some() && export::is_binary(c))
    });
    if binary {
        // R-SQL-39: the code page the bytes are read in.
        egui::ComboBox::from_id_salt("sql-detail-ccsid")
            .selected_text(format!("EBCDIC {}", page.label()))
            .show_ui(ui, |ui| {
                for choice in CodePage::ALL {
                    ui.selectable_value(page, choice, choice.label());
                }
            });
    }
    ui.separator();
    if let Some((row, _)) = view.detail_row() {
        let row = row.clone();
        ScrollArea::vertical()
            .id_salt("sql-detail")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for (column, value) in view.columns.iter().zip(&row) {
                    ui.horizontal(|ui| {
                        ui.label(theme::semibold(&column.name));
                        ui.label(RichText::new(&column.type_name).small().color(p.muted));
                        theme::button_row(ui, |ui| {
                            let copy = icons::button(icons::COPY, "");
                            let name = format!("Copy {}", column.name);
                            if icons::add_named(ui, value.is_some(), copy.small(), &name)
                                .on_hover_text("Copy the value")
                                .clicked()
                            {
                                ui.ctx().copy_text(value.clone().unwrap_or_default());
                            }
                        });
                    });
                    let dump = value
                        .as_deref()
                        .filter(|_| export::is_binary(column))
                        .and_then(|hex| export::binary_dump(hex, *page));
                    match value {
                        Some(_) if let Some(dump) = dump => {
                            ui.add(egui::Label::new(RichText::new(dump).monospace()).extend());
                        }
                        Some(text) => {
                            ui.add(egui::Label::new(RichText::new(text).monospace()).wrap());
                        }
                        None => {
                            ui.label(RichText::new("NULL").italics().color(p.muted));
                        }
                    }
                    ui.add_space(6.0);
                }
            });
    }
    if close {
        view.detail = None;
    } else if step != 0 {
        view.step_detail(step);
    }
}

/// A tab's name: the statement's first words.
fn title(statement: &str) -> String {
    let words: Vec<&str> = statement.split_whitespace().take(5).collect();
    let mut title = words.join(" ");
    if title.chars().count() > 32 {
        title = title.chars().take(31).collect::<String>() + "…";
    }
    title
}

#[cfg(test)]
mod tests {
    use parley_core::sql::protocol::Column;

    use super::*;

    fn view(cursor: u64, more: bool) -> ResultView {
        let column = Column {
            name: "N".to_owned(),
            type_name: "INTEGER".to_owned(),
            length: 10,
            scale: 0,
            nullable: false,
            numeric: true,
            field: None,
            schema: None,
            table: None,
            heading: None,
            text: None,
        };
        ResultView::new(vec![column], vec![vec![Some("1".to_owned())]], more, cursor)
    }

    #[test]
    fn each_query_gets_a_tab_and_the_newest_is_shown_r_sql_14() {
        let mut results = Results::default();
        results.add("SELECT * FROM ORDERS WHERE STATUS = 'æøå'", view(1, true));
        results.add("VALUES 1", view(2, false));
        assert_eq!(results.tabs.len(), 2);
        assert_eq!(results.shown, 1);
        assert_eq!(results.tabs[0].title, "SELECT * FROM ORDERS WHERE");
    }

    #[test]
    fn only_the_last_ten_tabs_are_kept_r_sql_14() {
        let mut results = Results::default();
        for n in 0..12 {
            results.add(&format!("VALUES {n}"), view(n, false));
        }
        assert_eq!(results.tabs.len(), MAX_TABS);
        assert_eq!(results.tabs[0].title, "VALUES 2", "the oldest went first");
        assert_eq!(results.shown, MAX_TABS - 1);
    }

    #[test]
    fn fetched_rows_go_to_the_tab_of_their_cursor_r_sql_14() {
        let mut results = Results::default();
        results.add("SELECT A", view(5, true));
        results.add("SELECT B", view(6, false));
        results.append(5, vec![vec![Some("2".to_owned())]], false);
        assert_eq!(results.tabs[0].view.counts(), (2, 2));
        assert_eq!(results.tabs[0].view.cursor, None);
        assert_eq!(results.tabs[1].view.counts(), (1, 1));
    }

    #[test]
    fn closing_the_last_tab_shows_the_one_before_r_sql_14() {
        let mut results = Results::default();
        results.add("SELECT A", view(1, false));
        results.add("SELECT B", view(2, false));
        results.close(1);
        assert_eq!(results.shown, 0);
        results.close(0);
        assert!(results.is_empty());
    }

    #[test]
    fn a_long_statement_makes_a_short_title_r_sql_14() {
        assert_eq!(
            title("SELECT\n  verylongcolumnname1, verylongcolumnname2 FROM T"),
            "SELECT verylongcolumnname1, ver…"
        );
        assert_eq!(title(""), "");
    }

    #[test]
    fn comparing_two_results_opens_a_result_with_the_differences_marked_r_sql_41() {
        let view = |rows: &[(&str, &str)]| {
            let column = |name: &str| parley_core::sql::protocol::Column {
                name: name.to_owned(),
                type_name: "VARCHAR".to_owned(),
                length: 10,
                scale: 0,
                nullable: true,
                numeric: false,
                field: None,
                schema: None,
                table: None,
                heading: None,
                text: None,
            };
            let rows = rows
                .iter()
                .map(|(a, b)| vec![Some((*a).to_owned()), Some((*b).to_owned())])
                .collect();
            ResultView::new(vec![column("ID"), column("NAVN")], rows, false, 0)
        };
        let mut results = Results::default();
        results.add("SELECT * FROM A", view(&[("1", "æøå"), ("2", "to")]));
        results.add("SELECT * FROM B", view(&[("1", "æøå"), ("2", "TO")]));
        results.compare_with(0);
        assert_eq!(results.tabs.len(), 3);
        let tab = &results.tabs[2];
        assert_eq!(tab.title, "SELECT * FROM B vs SELECT * FROM A");
        assert_eq!(tab.view.rows.len(), 2, "the pair that differs");
        assert!(tab.view.marked.contains(&(0, 2)) && tab.view.marked.contains(&(1, 2)));
        assert_eq!(
            results.note.as_deref(),
            Some("1 rows the same, 2 rows listed.")
        );
        results.compare_keys = vec!["NOPE".to_owned()];
        results.shown = 0;
        results.compare_with(1);
        assert_eq!(results.tabs.len(), 3, "nothing opened");
        assert_eq!(
            results.note.as_deref(),
            Some("Choose a key column the results have.")
        );
    }

    #[test]
    fn a_result_shows_as_a_chart_and_back_as_a_table_r_sql_42() {
        use egui_kittest::kittest::Queryable;

        let column = |name: &str, numeric: bool| parley_core::sql::protocol::Column {
            name: name.to_owned(),
            type_name: "X".to_owned(),
            length: 10,
            scale: 0,
            nullable: true,
            numeric,
            field: None,
            schema: None,
            table: None,
            heading: None,
            text: None,
        };
        let rows = vec![
            vec![Some("Århus".to_owned()), Some("3".to_owned())],
            vec![Some("Odense".to_owned()), Some("5".to_owned())],
        ];
        let mut results = Results::default();
        results.add(
            "SELECT BY, N FROM T",
            ResultView::new(vec![column("BY", false), column("N", true)], rows, false, 0),
        );
        let mut harness = egui_kittest::Harness::builder()
            .with_size(egui::vec2(900.0, 500.0))
            .build_ui_state(
                |ui, r: &mut Results| {
                    crate::test_support::install_fonts(ui);
                    let mut header = HeaderMode::default();
                    r.show(ui, true, &mut header, None);
                },
                results,
            );
        harness.run();
        harness.get_by_label("Chart").click();
        harness.run_steps(3);
        assert!(
            harness.query_by_label("Bars").is_some(),
            "the chart's controls"
        );
        assert!(harness.state().tabs[0].view.chart.is_some());
        harness.get_by_label("Lines").click();
        harness.run_steps(3);
        harness.get_by_label("Table").click();
        harness.run_steps(2);
        assert!(harness.state().tabs[0].view.chart.is_none());
    }
}
