//! Quick charts of a result (R-SQL-42, D23): the first text or date column along the bottom,
//! numeric columns as bars or lines, from the rows shown. Drawn with `egui_plot`.

use egui::{Color32, Ui};
use egui_plot::{Bar, BarChart, Legend, Line, Plot, PlotPoints};
use parley_core::sql::protocol::{Column, Row};

use crate::theme;

/// How the values are drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Kind {
    #[default]
    Bars,
    Lines,
}

/// A result's chart: how, and which numeric columns (by index) are drawn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chart {
    pub kind: Kind,
    pub series: Vec<usize>,
}

impl Chart {
    /// Bars of every numeric column, at most [`MAX_SERIES`].
    pub fn new(columns: &[Column]) -> Self {
        Self {
            kind: Kind::Bars,
            series: numeric(columns).take(MAX_SERIES).collect(),
        }
    }
}

/// At most this many columns are drawn at first.
const MAX_SERIES: usize = 4;

fn numeric(columns: &[Column]) -> impl Iterator<Item = usize> + '_ {
    columns
        .iter()
        .enumerate()
        .filter(|(_, c)| c.numeric)
        .map(|(i, _)| i)
}

/// What is drawn: a label per row, and per chosen column its name and values (`None` where a
/// value is null or not a number).
#[derive(Debug, Clone, PartialEq)]
pub struct Data {
    pub labels: Vec<String>,
    pub series: Vec<(String, Vec<Option<f64>>)>,
}

/// The chart's data from `rows`: labels from the first column that is not numeric (row numbers
/// when every column is), values from the `chosen` numeric columns.
pub fn data(columns: &[Column], rows: &[&Row], chosen: &[usize]) -> Data {
    let label_column = columns.iter().position(|c| !c.numeric);
    let labels = rows
        .iter()
        .enumerate()
        .map(|(n, row)| match label_column {
            Some(c) => row.get(c).cloned().flatten().unwrap_or_default(),
            None => (n + 1).to_string(),
        })
        .collect();
    let series = chosen
        .iter()
        .filter_map(|&c| {
            let column = columns.get(c).filter(|col| col.numeric)?;
            let values = rows
                .iter()
                .map(|row| row.get(c).cloned().flatten()?.trim().parse::<f64>().ok())
                .collect();
            Some((column.name.clone(), values))
        })
        .collect();
    Data { labels, series }
}

/// A series' colour, from the theme's palette.
fn colour(ui: &Ui, n: usize) -> Color32 {
    let p = theme::palette(ui);
    let colours = [p.accent, p.blue, p.purple, p.ok, p.warning_text, p.fail];
    colours[n % colours.len()]
}

/// The controls over the chart: bars or lines, and which columns.
fn controls(ui: &mut Ui, chart: &mut Chart, columns: &[Column]) {
    ui.horizontal(|ui| {
        theme::segmented(
            ui,
            &mut chart.kind,
            &[(Kind::Bars, "Bars"), (Kind::Lines, "Lines")],
        );
        ui.menu_button("Columns", |ui| {
            for c in numeric(columns) {
                let mut on = chart.series.contains(&c);
                let name = columns.get(c).map_or("", |col| col.name.as_str());
                if ui.checkbox(&mut on, name).changed() {
                    if on {
                        chart.series.push(c);
                        chart.series.sort_unstable();
                    } else {
                        chart.series.retain(|&s| s != c);
                    }
                }
            }
        });
    });
}

/// The chart of `rows`, under its controls.
pub fn show(ui: &mut Ui, chart: &mut Chart, columns: &[Column], rows: &[&Row]) {
    controls(ui, chart, columns);
    if numeric(columns).next().is_none() {
        theme::hint(ui, "This result has no numeric column to chart.");
        return;
    }
    let data = data(columns, rows, &chart.series);
    let labels = data.labels.clone();
    let colours: Vec<Color32> = (0..data.series.len()).map(|n| colour(ui, n)).collect();
    let kind = chart.kind;
    Plot::new("sql-chart")
        .legend(Legend::default())
        .x_axis_formatter(move |mark, _| {
            let at = mark.value.round();
            if (mark.value - at).abs() > 1e-6 || at < 0.0 {
                return String::new();
            }
            // Whole, non-negative: a row's place.
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "checked whole and >= 0"
            )]
            let index = at as usize;
            labels.get(index).cloned().unwrap_or_default()
        })
        .show(ui, |plot| {
            let count = data.series.len().max(1);
            #[expect(clippy::cast_precision_loss, reason = "a handful of series")]
            let width = 0.8 / count as f64;
            for (n, (name, values)) in data.series.into_iter().enumerate() {
                let points = values
                    .iter()
                    .enumerate()
                    .filter_map(|(i, v)| Some((i, (*v)?)));
                let colour = colours.get(n).copied().unwrap_or(Color32::GRAY);
                match kind {
                    Kind::Bars => {
                        #[expect(clippy::cast_precision_loss, reason = "row and series counts")]
                        let offset = (n as f64 - (count as f64 - 1.0) / 2.0) * width;
                        #[expect(clippy::cast_precision_loss, reason = "row counts")]
                        let bars = points
                            .map(|(i, v)| Bar::new(i as f64 + offset, v).width(width))
                            .collect();
                        plot.bar_chart(BarChart::new(name, bars).color(colour));
                    }
                    Kind::Lines => {
                        #[expect(clippy::cast_precision_loss, reason = "row counts")]
                        let line: PlotPoints<'_> = points.map(|(i, v)| [i as f64, v]).collect();
                        plot.line(Line::new(name, line).color(colour));
                    }
                }
            }
        });
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

    #[test]
    fn labels_come_from_the_first_text_column_and_values_from_numbers_r_sql_42() {
        let columns = [column("N", true), column("BY", false), column("SUM", true)];
        let rows = [
            vec![
                Some("1".to_owned()),
                Some("Århus".to_owned()),
                Some("10.5".to_owned()),
            ],
            vec![Some("2".to_owned()), None, Some("x".to_owned())],
        ];
        let refs: Vec<&Row> = rows.iter().collect();
        let d = data(&columns, &refs, &[0, 2, 1]);
        assert_eq!(d.labels, ["Århus", ""]);
        assert_eq!(
            d.series,
            [
                ("N".to_owned(), vec![Some(1.0), Some(2.0)]),
                ("SUM".to_owned(), vec![Some(10.5), None]),
            ],
            "a text column chosen is left out; a value that is not a number is a gap"
        );
        assert_eq!(Chart::new(&columns).series, [0, 2]);
    }

    #[test]
    fn rows_are_numbered_when_every_column_is_numeric_r_sql_42() {
        let columns = [column("A", true)];
        let rows = [vec![Some("3".to_owned())], vec![None]];
        let refs: Vec<&Row> = rows.iter().collect();
        let d = data(&columns, &refs, &[0]);
        assert_eq!(d.labels, ["1", "2"]);
        assert_eq!(d.series[0].1, [Some(3.0), None]);
        assert_eq!(data(&columns, &[], &[0]).labels.len(), 0);
    }
}
