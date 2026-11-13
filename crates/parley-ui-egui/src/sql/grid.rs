//! A query's rows in a grid (R-SQL-8), drawn as Tortuga draws its tables: a header row, then a
//! row per line in fixed-width cells, only the visible rows laid out. Numbers right-aligned,
//! NULL shown apart from an empty string; a click on a heading sorts the rows fetched, a click
//! on a row's number selects it for copying.

use std::cmp::Ordering;
use std::collections::BTreeSet;

use egui::{Align, Label, Layout, RichText, ScrollArea, Sense, Ui, vec2};
use parley_core::sql::protocol::{Column, Row};

use crate::{icons, theme};

const ROW_HEIGHT: f32 = 20.0;
const NUMBER_WIDTH: f32 = 52.0;

/// A query's result as the window shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResultView {
    pub columns: Vec<Column>,
    pub rows: Vec<Row>,
    /// Where more rows come from, while there are more.
    pub cursor: Option<u64>,
    /// The rows in the order shown: indices into `rows`.
    order: Vec<usize>,
    /// The column sorted on, and whether ascending.
    sort: Option<(usize, bool)>,
    /// Selected rows, as indices into `rows`.
    selected: BTreeSet<usize>,
}

impl ResultView {
    pub fn new(columns: Vec<Column>, rows: Vec<Row>, more: bool, cursor: u64) -> Self {
        let order = (0..rows.len()).collect();
        Self {
            columns,
            rows,
            cursor: more.then_some(cursor),
            order,
            sort: None,
            selected: BTreeSet::new(),
        }
    }

    /// Rows from "Fetch more"; the sort, if any, takes them in.
    pub fn append(&mut self, rows: Vec<Row>, more: bool) {
        let start = self.rows.len();
        self.rows.extend(rows);
        self.order.extend(start..self.rows.len());
        if !more {
            self.cursor = None;
        }
        if let Some((column, ascending)) = self.sort {
            self.sort_rows(column, ascending);
        }
    }

    /// A click on column `column`'s heading: ascending, then descending.
    pub fn sort_by(&mut self, column: usize) {
        let ascending = !matches!(self.sort, Some((c, true)) if c == column);
        self.sort_rows(column, ascending);
    }

    fn sort_rows(&mut self, column: usize, ascending: bool) {
        let numeric = self.columns.get(column).is_some_and(|c| c.numeric);
        let rows = &self.rows;
        let value = |i: usize| {
            rows.get(i)
                .and_then(|r| r.get(column))
                .and_then(Option::as_deref)
        };
        self.order.sort_by(|&a, &b| {
            let order = compare(value(a), value(b), numeric);
            if ascending { order } else { order.reverse() }
        });
        self.sort = Some((column, ascending));
    }

    /// The rows shown, in their order.
    fn shown(&self) -> impl Iterator<Item = &Row> {
        self.order.iter().filter_map(|&i| self.rows.get(i))
    }

    /// The selected rows, or all rows when none is selected, as tab-separated text with the
    /// column names first: it pastes into a spreadsheet as columns.
    pub fn tsv(&self) -> String {
        let clean = |s: &str| s.replace(['\t', '\n', '\r'], " ");
        let mut out = self
            .columns
            .iter()
            .map(|c| clean(&c.name))
            .collect::<Vec<_>>()
            .join("\t");
        let all = self.selected.is_empty();
        for (i, row) in self
            .order
            .iter()
            .filter_map(|&i| Some((i, self.rows.get(i)?)))
        {
            if all || self.selected.contains(&i) {
                out.push('\n');
                let cells: Vec<String> = row
                    .iter()
                    .map(|v| clean(v.as_deref().unwrap_or("")))
                    .collect();
                out.push_str(&cells.join("\t"));
            }
        }
        out
    }

    /// All rows as CSV (RFC 4180, UTF-8 with a byte-order mark so spreadsheets read æøå):
    /// the column names, then the rows in the order shown; NULL as an empty field.
    pub fn csv(&self) -> String {
        let field = |s: &str| {
            if s.contains([',', '"', '\n', '\r']) {
                format!("\"{}\"", s.replace('"', "\"\""))
            } else {
                s.to_owned()
            }
        };
        let mut out = String::from('\u{FEFF}');
        let names: Vec<String> = self.columns.iter().map(|c| field(&c.name)).collect();
        out.push_str(&names.join(","));
        out.push_str("\r\n");
        for row in self.shown() {
            let cells: Vec<String> = row
                .iter()
                .map(|v| field(v.as_deref().unwrap_or("")))
                .collect();
            out.push_str(&cells.join(","));
            out.push_str("\r\n");
        }
        out
    }

    pub fn selected_count(&self) -> usize {
        self.selected.len()
    }

    /// Draws the grid in the space left.
    pub fn show(&mut self, ui: &mut Ui) {
        let p = theme::palette(ui);
        let widths: Vec<f32> = (0..self.columns.len()).map(|c| self.width(c)).collect();
        let total = NUMBER_WIDTH + widths.iter().sum::<f32>();
        let mut sort = None;
        let mut toggle = None;
        ScrollArea::horizontal()
            .id_salt("sql-grid-h")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_min_width(total);
                ui.horizontal(|ui| {
                    ui.set_height(ROW_HEIGHT);
                    ui.allocate_space(vec2(NUMBER_WIDTH, ROW_HEIGHT));
                    for (c, column) in self.columns.iter().enumerate() {
                        if self.heading(ui, c, column, widths[c]) {
                            sort = Some(c);
                        }
                    }
                });
                ui.separator();
                ScrollArea::vertical()
                    .id_salt("sql-grid-v")
                    .auto_shrink([false, false])
                    .show_rows(ui, ROW_HEIGHT, self.order.len(), |ui, range| {
                        for position in range {
                            let Some(&index) = self.order.get(position) else {
                                continue;
                            };
                            let selected = self.selected.contains(&index);
                            let top = ui.cursor().min;
                            if selected {
                                let rect = egui::Rect::from_min_size(top, vec2(total, ROW_HEIGHT));
                                ui.painter().rect_filled(rect, 3.0, p.surface_hover);
                            }
                            ui.horizontal(|ui| {
                                ui.set_height(ROW_HEIGHT);
                                let number =
                                    egui::Button::selectable(selected, (index + 1).to_string());
                                if ui
                                    .add_sized([NUMBER_WIDTH - 4.0, ROW_HEIGHT - 2.0], number)
                                    .clicked()
                                {
                                    toggle = Some(index);
                                }
                                if let Some(row) = self.rows.get(index) {
                                    for (c, value) in row.iter().enumerate() {
                                        let numeric =
                                            self.columns.get(c).is_some_and(|col| col.numeric);
                                        cell(
                                            ui,
                                            widths.get(c).copied().unwrap_or(120.0),
                                            value.as_deref(),
                                            numeric,
                                        );
                                    }
                                }
                            });
                        }
                    });
            });
        if let Some(c) = sort {
            self.sort_by(c);
        }
        if let Some(i) = toggle
            && !self.selected.remove(&i)
        {
            self.selected.insert(i);
        }
    }

    /// A column's heading, a button that sorts; true when clicked.
    fn heading(&self, ui: &mut Ui, c: usize, column: &Column, width: f32) -> bool {
        let p = theme::palette(ui);
        let arrow = match self.sort {
            Some((s, true)) if s == c => Some(icons::ARROW_UP),
            Some((s, false)) if s == c => Some(icons::ARROW_DOWN),
            _ => None,
        };
        let name = theme::semibold(&column.name).size(12.0).color(p.muted);
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
        response
            .on_hover_text(format!("{kind}. Click to sort."))
            .clicked()
    }

    /// A column's width, from its name and its first rows' values.
    #[expect(
        clippy::cast_precision_loss,
        reason = "character counts are far below f32's exact range"
    )]
    fn width(&self, c: usize) -> f32 {
        let name = self
            .columns
            .get(c)
            .map_or(0, |col| col.name.chars().count());
        let longest = self
            .rows
            .iter()
            .take(200)
            .filter_map(|row| row.get(c))
            .map(|v| v.as_deref().map_or(4, |s| s.chars().count()))
            .max()
            .unwrap_or(0)
            .max(name);
        (longest as f32 * 7.5 + 20.0).clamp(64.0, 360.0)
    }
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
            ui.add(Label::new(text).truncate());
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

    #[test]
    fn numbers_sort_as_numbers_with_null_first_and_a_second_click_reverses_r_sql_8() {
        let mut v = view();
        v.sort_by(0);
        let amounts: Vec<Option<&str>> = v.shown().map(|r| r[0].as_deref()).collect();
        assert_eq!(amounts, [None, Some("-2"), Some("9"), Some("10.5")]);
        v.sort_by(0);
        let amounts: Vec<Option<&str>> = v.shown().map(|r| r[0].as_deref()).collect();
        assert_eq!(amounts, [Some("10.5"), Some("9"), Some("-2"), None]);
    }

    #[test]
    fn fetched_rows_join_the_sort_and_the_cursor_ends_with_the_last_r_sql_8() {
        let mut v = view();
        v.sort_by(0);
        v.append(
            vec![vec![Some("0".to_owned()), Some("x".to_owned())]],
            false,
        );
        let amounts: Vec<Option<&str>> = v.shown().map(|r| r[0].as_deref()).collect();
        assert_eq!(
            amounts,
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
}
