//! A query's rows in a grid (R-SQL-8, R-SQL-16), drawn as Tortuga draws its tables: a header
//! row, then a row per line in fixed-width cells, only the visible rows laid out. Numbers
//! right-aligned, NULL shown apart from an empty string. A click on a heading sorts the rows
//! fetched, dragging its right edge resizes the column (a double-click fits it), a click on a
//! row's number selects it, a double-click on a row shows its values one under another
//! (R-SQL-15). A filter keeps the rows that hold its text.

use std::cmp::Ordering;
use std::collections::BTreeSet;

use egui::{Align, Label, Layout, RichText, ScrollArea, Sense, Ui, vec2};
use parley_core::sql::protocol::{Column, Row};
use parley_core::sql::{export, transfer};

use crate::{icons, theme};

const ROW_HEIGHT: f32 = 20.0;
const NUMBER_WIDTH: f32 = 52.0;
const MIN_WIDTH: f32 = 40.0;
const MAX_FIT: f32 = 420.0;

/// What a column's heading in the grid says (R-SQL-29): the field names, the IBM i column
/// headings (COLHDG, `LABEL ON`), or both. A column without a heading shows its name.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum HeaderMode {
    Names,
    #[default]
    Headings,
    Both,
}

/// What `column`'s heading says in `mode`.
pub fn header_text(column: &Column, mode: HeaderMode) -> String {
    match (mode, column.heading.as_deref()) {
        (HeaderMode::Headings, Some(heading)) => heading.to_owned(),
        (HeaderMode::Both, Some(heading)) => format!("{} · {heading}", column.name),
        (HeaderMode::Names | HeaderMode::Headings | HeaderMode::Both, _) => column.name.clone(),
    }
}

/// The formats Copy as offers (R-SQL-38).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CopyFormat {
    Markdown,
    Inserts,
    Json,
}

impl CopyFormat {
    pub const ALL: [CopyFormat; 3] = [CopyFormat::Markdown, CopyFormat::Inserts, CopyFormat::Json];

    pub fn label(self) -> &'static str {
        match self {
            CopyFormat::Markdown => "Markdown table",
            CopyFormat::Inserts => "INSERT statements",
            CopyFormat::Json => "JSON",
        }
    }
}

/// A query's result as the window shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct ResultView {
    pub columns: Vec<Column>,
    pub rows: Vec<Row>,
    /// Where more rows come from, while there are more.
    pub cursor: Option<u64>,
    /// Every row in sort order: indices into `rows`.
    sorted: Vec<usize>,
    /// The rows shown: `sorted` that pass the filter.
    visible: Vec<usize>,
    /// The column sorted on, and whether ascending.
    sort: Option<(usize, bool)>,
    /// Selected rows, as indices into `rows`.
    selected: BTreeSet<usize>,
    /// Text a row must hold to be shown, in any column, ignoring case (R-SQL-16).
    pub filter: String,
    /// The filter `visible` was made for.
    filtered_for: String,
    widths: Vec<f32>,
    /// The row shown one value under another (R-SQL-15).
    pub detail: Option<usize>,
    /// What the headings say (R-SQL-29), as the window's choice sets it.
    pub header: HeaderMode,
    /// Values marked as different in a comparison (R-SQL-41): (row in `rows`, column).
    pub marked: BTreeSet<(usize, usize)>,
    /// The chart shown instead of the grid, while it is (R-SQL-42).
    pub chart: Option<super::chart::Chart>,
}

impl ResultView {
    pub fn new(columns: Vec<Column>, rows: Vec<Row>, more: bool, cursor: u64) -> Self {
        let sorted: Vec<usize> = (0..rows.len()).collect();
        let mut view = Self {
            visible: sorted.clone(),
            sorted,
            columns,
            rows,
            cursor: more.then_some(cursor),
            sort: None,
            selected: BTreeSet::new(),
            filter: String::new(),
            filtered_for: String::new(),
            widths: Vec::new(),
            detail: None,
            header: HeaderMode::default(),
            marked: BTreeSet::new(),
            chart: None,
        };
        view.widths = (0..view.columns.len()).map(|c| view.fit(c)).collect();
        view
    }

    /// Rows from "Fetch more"; the sort and the filter take them in.
    pub fn append(&mut self, rows: Vec<Row>, more: bool) {
        let start = self.rows.len();
        self.rows.extend(rows);
        self.sorted.extend(start..self.rows.len());
        if !more {
            self.cursor = None;
        }
        if let Some((column, ascending)) = self.sort {
            self.sort_rows(column, ascending);
        }
        self.refilter();
    }

    /// A click on column `column`'s heading: ascending, then descending.
    pub fn sort_by(&mut self, column: usize) {
        let ascending = !matches!(self.sort, Some((c, true)) if c == column);
        self.sort_rows(column, ascending);
        self.refilter();
    }

    fn sort_rows(&mut self, column: usize, ascending: bool) {
        let numeric = self.columns.get(column).is_some_and(|c| c.numeric);
        let rows = &self.rows;
        let value = |i: usize| {
            rows.get(i)
                .and_then(|r| r.get(column))
                .and_then(Option::as_deref)
        };
        self.sorted.sort_by(|&a, &b| {
            let order = compare(value(a), value(b), numeric);
            if ascending { order } else { order.reverse() }
        });
        self.sort = Some((column, ascending));
    }

    /// Makes `visible` the sorted rows that hold the filter's text.
    fn refilter(&mut self) {
        let needle = self.filter.trim().to_lowercase();
        self.visible = if needle.is_empty() {
            self.sorted.clone()
        } else {
            let rows = &self.rows;
            self.sorted
                .iter()
                .copied()
                .filter(|&i| {
                    rows.get(i).is_some_and(|row| {
                        row.iter()
                            .flatten()
                            .any(|v| v.to_lowercase().contains(&needle))
                    })
                })
                .collect()
        };
        self.filtered_for.clone_from(&self.filter);
    }

    /// How many rows are shown, of how many fetched.
    pub fn counts(&self) -> (usize, usize) {
        (self.visible.len(), self.rows.len())
    }

    /// The rows shown, in their order.
    fn shown(&self) -> impl Iterator<Item = (usize, &Row)> {
        self.visible
            .iter()
            .filter_map(|&i| Some((i, self.rows.get(i)?)))
    }

    /// The selected rows shown, or all rows shown when none is, as tab-separated text with the
    /// column names first: it pastes into a spreadsheet as columns.
    pub fn tsv(&self) -> String {
        let clean = |s: &str| s.replace(['\t', '\n', '\r'], " ");
        let mut out = self
            .columns
            .iter()
            .map(|c| clean(&header_text(c, self.header)))
            .collect::<Vec<_>>()
            .join("\t");
        for row in self.chosen() {
            out.push('\n');
            let cells: Vec<String> = row
                .iter()
                .map(|v| clean(v.as_deref().unwrap_or("")))
                .collect();
            out.push_str(&cells.join("\t"));
        }
        out
    }

    /// The rows shown, in their order: what a chart draws.
    pub fn shown_rows(&self) -> Vec<&Row> {
        self.shown().map(|(_, row)| row).collect()
    }

    /// The selected rows shown, or all rows shown when none is: what Copy copies.
    fn chosen(&self) -> Vec<&Row> {
        let all = !self.shown().any(|(i, _)| self.selected.contains(&i));
        self.shown()
            .filter(|(i, _)| all || self.selected.contains(i))
            .map(|(_, row)| row)
            .collect()
    }

    /// What Copy copies, as a Markdown table, `INSERT` statements or JSON (R-SQL-38).
    pub fn copy_as(&self, format: CopyFormat) -> String {
        let rows = self.chosen();
        match format {
            CopyFormat::Markdown => {
                let names: Vec<String> = self
                    .columns
                    .iter()
                    .map(|c| header_text(c, self.header))
                    .collect();
                export::markdown(&names, &self.columns, &rows)
            }
            CopyFormat::Inserts => export::inserts(&self.columns, &rows),
            CopyFormat::Json => export::json(&self.columns, &rows),
        }
    }

    /// The rows shown as CSV (RFC 4180, UTF-8 with a byte-order mark so spreadsheets read
    /// æøå): the column names, then the rows in the order shown; NULL as an empty field.
    pub fn csv(&self) -> String {
        let names: Vec<String> = self
            .columns
            .iter()
            .map(|c| header_text(c, self.header))
            .collect();
        let mut out = String::from('\u{FEFF}');
        out.push_str(&transfer::csv_line(names.iter().map(String::as_str)));
        for (_, row) in self.shown() {
            out.push_str(&transfer::csv_line(
                row.iter().map(|v| v.as_deref().unwrap_or("")),
            ));
        }
        out
    }

    /// How many of the rows shown are selected.
    pub fn selected_count(&self) -> usize {
        self.shown()
            .filter(|(i, _)| self.selected.contains(i))
            .count()
    }

    /// The one row selected among those shown, if exactly one is.
    pub fn selected_row(&self) -> Option<&Row> {
        let mut selected = self.shown().filter(|(i, _)| self.selected.contains(i));
        match (selected.next(), selected.next()) {
            (Some((_, row)), None) => Some(row),
            _ => None,
        }
    }

    /// The row of the detail panel and its place among the rows shown (1-based).
    pub fn detail_row(&self) -> Option<(&Row, usize)> {
        let index = self.detail?;
        let position = self.visible.iter().position(|&i| i == index)?;
        Some((self.rows.get(index)?, position + 1))
    }

    /// The detail panel on the row `step` places down (negative: up) among the rows shown.
    pub fn step_detail(&mut self, step: isize) {
        let Some(position) = self
            .detail
            .and_then(|d| self.visible.iter().position(|&i| i == d))
        else {
            return;
        };
        let next = position.saturating_add_signed(step);
        if let Some(&index) = self.visible.get(next) {
            self.detail = Some(index);
        }
    }

    /// Draws the grid in the space left.
    pub fn show(&mut self, ui: &mut Ui) {
        if self.filter != self.filtered_for {
            self.refilter();
        }
        // Enter on one selected row shows it, as a double-click does (R-SQL-15). Taken before
        // the rows are drawn: a focused row number would take Enter as a click.
        if self.selected.len() == 1
            && !ui.ctx().egui_wants_keyboard_input()
            && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter))
        {
            self.detail = self.selected.first().copied();
        }
        let total = NUMBER_WIDTH + self.widths.iter().sum::<f32>();
        let mut sort = None;
        let mut toggle = None;
        let mut open = None;
        let mut resized = None;
        ScrollArea::horizontal()
            .id_salt("sql-grid-h")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_min_width(total);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    ui.set_height(ROW_HEIGHT);
                    ui.allocate_space(vec2(NUMBER_WIDTH, ROW_HEIGHT));
                    for c in 0..self.columns.len() {
                        match self.heading(ui, c) {
                            Heading::Sort => sort = Some(c),
                            Heading::Resize(width) => resized = Some((c, width)),
                            Heading::Fit => resized = Some((c, self.fit(c))),
                            Heading::None => {}
                        }
                    }
                });
                ui.separator();
                ScrollArea::vertical()
                    .id_salt("sql-grid-v")
                    .auto_shrink([false, false])
                    .show_rows(ui, ROW_HEIGHT, self.visible.len(), |ui, range| {
                        for position in range {
                            let Some(&index) = self.visible.get(position) else {
                                continue;
                            };
                            let (clicked, double) = self.row(ui, index, total);
                            if clicked {
                                toggle = Some(index);
                            }
                            if double {
                                open = Some(index);
                            }
                        }
                    });
            });
        if let Some(c) = sort {
            self.sort_by(c);
        }
        if let Some((c, width)) = resized
            && let Some(w) = self.widths.get_mut(c)
        {
            *w = width.max(MIN_WIDTH);
        }
        if let Some(i) = toggle
            && !self.selected.remove(&i)
        {
            self.selected.insert(i);
        }
        if open.is_some() {
            self.detail = open;
        }
    }

    /// Draws row `index`, `total` wide: (its number was clicked, its values double-clicked).
    fn row(&self, ui: &mut Ui, index: usize, total: f32) -> (bool, bool) {
        let p = theme::palette(ui);
        let selected = self.selected.contains(&index);
        let rect = egui::Rect::from_min_size(ui.cursor().min, vec2(total, ROW_HEIGHT));
        if selected || self.detail == Some(index) {
            ui.painter().rect_filled(rect, 3.0, p.surface_hover);
        }
        let clicked = ui
            .horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.set_height(ROW_HEIGHT);
                let number = egui::Button::selectable(selected, (index + 1).to_string());
                let size = [NUMBER_WIDTH - 4.0, ROW_HEIGHT - 2.0];
                let clicked = ui.add_sized(size, number).clicked();
                ui.add_space(4.0);
                if let Some(row) = self.rows.get(index) {
                    for (c, value) in row.iter().enumerate() {
                        let numeric = self.columns.get(c).is_some_and(|col| col.numeric);
                        let width = self.widths.get(c).copied().unwrap_or(120.0);
                        if self.marked.contains(&(index, c)) {
                            let at = egui::Rect::from_min_size(
                                ui.cursor().min,
                                vec2(width - 2.0, ROW_HEIGHT - 2.0),
                            );
                            ui.painter()
                                .rect_filled(at, 2.0, p.warning_text.gamma_multiply(0.2));
                        }
                        cell(ui, width, value.as_deref(), numeric);
                    }
                }
                clicked
            })
            .inner;
        // The values only: over the row's number it would take its clicks.
        let mut values = rect;
        values.min.x += NUMBER_WIDTH;
        let id = ui.make_persistent_id(("sql-row", index));
        let double = ui.interact(values, id, Sense::click()).double_clicked();
        (clicked, double)
    }

    /// A column's heading: a click sorts; its right edge resizes the column, and a
    /// double-click on the edge fits it.
    fn heading(&self, ui: &mut Ui, c: usize) -> Heading {
        let p = theme::palette(ui);
        let Some(column) = self.columns.get(c) else {
            return Heading::None;
        };
        let width = self.widths.get(c).copied().unwrap_or(120.0);
        let arrow = match self.sort {
            Some((s, true)) if s == c => Some(icons::ARROW_UP),
            Some((s, false)) if s == c => Some(icons::ARROW_DOWN),
            _ => None,
        };
        let name = theme::semibold(header_text(column, self.header))
            .size(12.0)
            .color(p.muted);
        let response = ui
            .allocate_ui(vec2(width, ROW_HEIGHT), |ui| {
                ui.set_width(width - 8.0);
                let layout = if column.numeric {
                    Layout::right_to_left(Align::Center)
                } else {
                    Layout::left_to_right(Align::Center)
                };
                ui.with_layout(layout, |ui| match arrow {
                    Some(arrow) => {
                        icons::label(ui, icons::icon(arrow).size(11.0).color(p.muted), name)
                    }
                    None => ui.add(Label::new(name).truncate().selectable(false)),
                })
                .inner
            })
            .response
            .interact(Sense::click());
        let kind = format!(
            "{} ({}{}){}",
            column.type_name,
            column.length,
            if column.scale > 0 {
                format!(", {}", column.scale)
            } else {
                String::new()
            },
            if column.nullable { ", may be NULL" } else { "" }
        );
        let sorted = response.on_hover_text(about(column, &kind)).clicked();
        // The edge: a few points either side of the column's right border.
        let edge = egui::Rect::from_center_size(
            egui::pos2(ui.cursor().min.x, ui.cursor().center().y),
            vec2(8.0, ROW_HEIGHT),
        );
        let drag = ui.interact(
            edge,
            ui.make_persistent_id(("sql-col-edge", c)),
            Sense::click_and_drag(),
        );
        if drag.hovered() || drag.dragged() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
        }
        if drag.double_clicked() {
            Heading::Fit
        } else if drag.dragged() {
            Heading::Resize(width + drag.drag_delta().x)
        } else if sorted {
            Heading::Sort
        } else {
            Heading::None
        }
    }

    /// The width that fits column `c`'s name and the values shown (the first 500).
    #[expect(
        clippy::cast_precision_loss,
        reason = "character counts are far below f32's exact range"
    )]
    fn fit(&self, c: usize) -> f32 {
        let name = self
            .columns
            .get(c)
            .map_or(0, |col| header_text(col, self.header).chars().count());
        let longest = self
            .shown()
            .take(500)
            .filter_map(|(_, row)| row.get(c))
            .map(|v| v.as_deref().map_or(4, |s| s.chars().count()))
            .max()
            .unwrap_or(0)
            .max(name);
        (longest as f32 * 7.5 + 20.0).clamp(64.0, MAX_FIT)
    }
}

/// A column's hover: its name, heading, text and type, and where it comes from (R-SQL-29).
fn about(column: &Column, kind: &str) -> String {
    let mut lines = vec![column.name.clone()];
    if let Some(heading) = &column.heading {
        lines.push(format!("Heading: {heading}"));
    }
    if let Some(text) = column
        .text
        .as_ref()
        .filter(|t| Some(*t) != column.heading.as_ref())
    {
        lines.push(format!("Text: {text}"));
    }
    if let (Some(schema), Some(table)) = (&column.schema, &column.table) {
        lines.push(format!("From {schema}/{table}"));
    }
    lines.push(format!("{kind}. Click to sort."));
    lines.join("\n")
}

/// What happened on a column's heading.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Heading {
    None,
    Sort,
    /// Dragged to this width.
    Resize(f32),
    /// Its edge was double-clicked.
    Fit,
}

/// One value: right-aligned when numeric, NULL muted in italics.
fn cell(ui: &mut Ui, width: f32, value: Option<&str>, numeric: bool) {
    let p = theme::palette(ui);
    ui.allocate_ui(vec2(width, ROW_HEIGHT), |ui| {
        ui.set_width(width - 8.0);
        let layout = if numeric {
            Layout::right_to_left(Align::Center)
        } else {
            Layout::left_to_right(Align::Center)
        };
        ui.with_layout(layout, |ui| {
            let text = match value {
                Some(text) => RichText::new(text),
                None => RichText::new("NULL").italics().color(p.muted),
            };
            ui.add(Label::new(text).truncate().selectable(false));
        });
    });
}

/// NULL first; numbers as numbers in numeric columns, text otherwise.
fn compare(a: Option<&str>, b: Option<&str>, numeric: bool) -> Ordering {
    match (a, b) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Less,
        (Some(_), None) => Ordering::Greater,
        (Some(a), Some(b)) if numeric => match (a.parse::<f64>(), b.parse::<f64>()) {
            (Ok(x), Ok(y)) => x.total_cmp(&y),
            _ => a.cmp(b),
        },
        (Some(a), Some(b)) => a.cmp(b),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn column(name: &str, numeric: bool) -> Column {
        Column {
            name: name.to_owned(),
            type_name: if numeric { "DECIMAL" } else { "VARCHAR" }.to_owned(),
            length: 10,
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

    fn view() -> ResultView {
        let row =
            |a: Option<&str>, b: Option<&str>| vec![a.map(str::to_owned), b.map(str::to_owned)];
        ResultView::new(
            vec![column("AMOUNT", true), column("NAME", false)],
            vec![
                row(Some("10.5"), Some("æøå")),
                row(Some("-2"), Some("")),
                row(None, Some("tab\there")),
                row(Some("9"), None),
            ],
            true,
            7,
        )
    }

    fn amounts(v: &ResultView) -> Vec<Option<&str>> {
        v.shown().map(|(_, r)| r[0].as_deref()).collect()
    }

    #[test]
    fn numbers_sort_as_numbers_with_null_first_and_a_second_click_reverses_r_sql_8() {
        let mut v = view();
        v.sort_by(0);
        assert_eq!(amounts(&v), [None, Some("-2"), Some("9"), Some("10.5")]);
        v.sort_by(0);
        assert_eq!(amounts(&v), [Some("10.5"), Some("9"), Some("-2"), None]);
    }

    #[test]
    fn fetched_rows_join_the_sort_and_the_cursor_ends_with_the_last_r_sql_8() {
        let mut v = view();
        v.sort_by(0);
        v.append(
            vec![vec![Some("0".to_owned()), Some("x".to_owned())]],
            false,
        );
        assert_eq!(
            amounts(&v),
            [None, Some("-2"), Some("0"), Some("9"), Some("10.5")]
        );
        assert_eq!(v.cursor, None);
    }

    #[test]
    fn tsv_pastes_as_columns_with_null_empty_and_no_stray_tabs_r_sql_8() {
        let mut v = view();
        assert_eq!(v.tsv(), "AMOUNT\tNAME\n10.5\tæøå\n-2\t\n\ttab here\n9\t");
        v.selected.insert(1);
        assert_eq!(v.tsv(), "AMOUNT\tNAME\n-2\t", "only the selected rows");
    }

    #[test]
    fn copy_as_takes_the_selected_rows_or_all_shown_r_sql_38() {
        let mut v = ResultView::new(
            vec![column("N", true), column("NAVN", false)],
            vec![
                vec![Some("1".to_owned()), Some("æøå".to_owned())],
                vec![Some("2".to_owned()), None],
            ],
            false,
            0,
        );
        assert_eq!(
            v.copy_as(CopyFormat::Json),
            "[\n  {\"N\": 1, \"NAVN\": \"æøå\"},\n  {\"N\": 2, \"NAVN\": null}\n]"
        );
        v.selected.insert(1);
        assert_eq!(
            v.copy_as(CopyFormat::Markdown),
            "| N | NAVN |\n| ---: | --- |\n| 2 |  |"
        );
        assert!(
            v.copy_as(CopyFormat::Inserts)
                .ends_with("VALUES (2, NULL);")
        );
    }

    #[test]
    fn csv_quotes_what_needs_it_and_starts_with_a_byte_order_mark_r_sql_8() {
        let v = ResultView::new(
            vec![column("A", false), column("B", false)],
            vec![vec![Some("say \"hi\", æøå".to_owned()), None]],
            false,
            0,
        );
        assert_eq!(v.csv(), "\u{FEFF}A,B\r\n\"say \"\"hi\"\", æøå\",\r\n");
        assert_eq!(v.cursor, None, "no more rows");
    }

    #[test]
    fn the_filter_keeps_rows_holding_its_text_ignoring_case_and_copy_takes_them_r_sql_16() {
        let mut v = view();
        "ÆØ".clone_into(&mut v.filter);
        v.refilter();
        assert_eq!(v.counts(), (1, 4));
        assert_eq!(v.tsv(), "AMOUNT\tNAME\n10.5\tæøå");
        assert_eq!(v.csv(), "\u{FEFF}AMOUNT,NAME\r\n10.5,æøå\r\n");
        "nothing like it".clone_into(&mut v.filter);
        v.refilter();
        assert_eq!(v.counts(), (0, 4));
        v.filter.clear();
        v.refilter();
        assert_eq!(v.counts(), (4, 4));
    }

    #[test]
    fn a_selection_hidden_by_the_filter_is_not_copied_r_sql_16() {
        let mut v = view();
        v.selected.insert(0); // the æøå row
        "-2".clone_into(&mut v.filter);
        v.refilter();
        assert_eq!(v.selected_count(), 0);
        assert_eq!(
            v.tsv(),
            "AMOUNT\tNAME\n-2\t",
            "the rows shown, not the hidden selection"
        );
    }

    #[test]
    fn the_detail_steps_through_the_rows_shown_and_stops_at_the_ends_r_sql_15() {
        let mut v = view();
        v.sort_by(0); // NULL, -2, 9, 10.5
        v.detail = Some(1); // the -2 row
        assert_eq!(v.detail_row().map(|(_, at)| at), Some(2));
        v.step_detail(1);
        assert_eq!(v.detail_row().and_then(|(r, _)| r[0].as_deref()), Some("9"));
        v.step_detail(10);
        assert_eq!(
            v.detail_row().and_then(|(r, _)| r[0].as_deref()),
            Some("9"),
            "past the end"
        );
        v.step_detail(-5);
        assert_eq!(
            v.detail_row().and_then(|(r, _)| r[0].as_deref()),
            None,
            "the NULL row, first"
        );
        assert_eq!(v.detail_row().map(|(_, at)| at), Some(1));
    }

    #[test]
    fn a_column_fits_its_longest_value_within_limits_r_sql_16() {
        let v = view();
        assert!(v.fit(0) >= 64.0);
        let long = ResultView::new(
            vec![column("X", false)],
            vec![vec![Some("x".repeat(500))]],
            false,
            0,
        );
        assert!((long.fit(0) - MAX_FIT).abs() < f32::EPSILON);
    }

    #[test]
    fn headings_names_or_both_head_the_columns_and_copy_uses_them_r_sql_29() {
        // As the owner's IBM i gave them for a DDS file with COLHDG.
        let mut seq = column("REGSEQ", true);
        seq.heading = Some("&Registration.Sequence".to_owned());
        let plain = column("T", false); // an alias: no heading
        let mut v = ResultView::new(
            vec![seq, plain],
            vec![vec![Some("1".to_owned()), Some("æøå".to_owned())]],
            false,
            0,
        );
        assert_eq!(
            v.tsv().lines().next(),
            Some("&Registration.Sequence\tT"),
            "headings by default"
        );
        v.header = HeaderMode::Names;
        assert_eq!(v.tsv().lines().next(), Some("REGSEQ\tT"));
        v.header = HeaderMode::Both;
        assert_eq!(
            v.csv().lines().next(),
            Some("\u{FEFF}REGSEQ · &Registration.Sequence,T")
        );
    }

    #[test]
    fn the_hover_names_heading_text_and_file_r_sql_29() {
        let mut c = column("REGSEQ", true);
        c.heading = Some("Registration sequence".to_owned());
        c.text = Some("Sequence number of the registration".to_owned());
        c.schema = Some("MSGSUDTA".to_owned());
        c.table = Some("REGP".to_owned());
        let hover = about(&c, "DECIMAL (9)");
        assert_eq!(
            hover,
            "REGSEQ\nHeading: Registration sequence\nText: Sequence number of the registration\nFrom MSGSUDTA/REGP\nDECIMAL (9). Click to sort."
        );
        c.text = c.heading.clone();
        assert!(
            !about(&c, "x").contains("Text:"),
            "the same text is not said twice"
        );
    }
}
