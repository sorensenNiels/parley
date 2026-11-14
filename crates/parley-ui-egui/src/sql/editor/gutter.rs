//! The gutter left of the editor's text (R-SQL-61, R-SQL-62, D28): line numbers, a ▶ on each
//! statement's first line that runs it, and the outcome of its last run. Drawn after the text
//! field from the rows it laid out, so wrapped lines and the text size are followed.

use std::collections::HashMap;
use std::ops::Range;
use std::sync::Arc;
use std::time::Duration;

use egui::{Align2, FontId, Galley, Pos2, Rect, Sense, Ui, vec2};
use parley_core::sql::text;

use crate::{icons, theme};

/// Room for the outcome mark, left of the numbers.
const MARK: f32 = 14.0;
/// Room for the ▶, right of the numbers.
const PLAY: f32 = 18.0;

/// How a statement's last run ended (R-SQL-62).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Rows { count: usize, more: bool },
    Changed(i64),
    Done,
    Failed(String),
}

impl Outcome {
    /// What hovering the mark says.
    fn said(&self, took: Duration) -> String {
        let time = format!("{:.2} s", took.as_secs_f64());
        match self {
            Self::Rows { count, more } => {
                let more = if *more { ", more to fetch" } else { "" };
                format!("{count} rows{more} in {time}")
            }
            Self::Changed(count) => format!("{count} rows changed in {time}"),
            Self::Done => format!("Done in {time}"),
            Self::Failed(message) => message.clone(),
        }
    }
}

/// The outcomes of the statements run in the window, by their text with blanks made single.
#[derive(Debug, Default)]
pub struct Outcomes(HashMap<String, (Outcome, Duration)>);

impl Outcomes {
    pub fn note(&mut self, statement: &str, outcome: Outcome, took: Duration) {
        self.0.insert(key(statement), (outcome, took));
    }

    fn get(&self, statement: &str) -> Option<&(Outcome, Duration)> {
        self.0.get(&key(statement))
    }
}

/// A statement's text as the outcomes are kept by: blanks made single, no `;` at the end.
fn key(statement: &str) -> String {
    statement
        .trim()
        .trim_end_matches(';')
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// How wide the gutter is for `text` in `font`.
pub fn width(ui: &Ui, text: &str, font: &FontId) -> f32 {
    let digits = parley_core::sql::edit::line_count(text)
        .to_string()
        .len()
        .max(2);
    let digit = ui.fonts_mut(|f| f.glyph_width(font, '0'));
    #[expect(clippy::cast_precision_loss, reason = "a few digits")]
    let numbers = digits as f32 * digit;
    MARK + numbers + PLAY
}

/// What the gutter needs to know about the text.
pub struct Lines<'a> {
    pub text: &'a str,
    /// The cursor, as a byte offset.
    pub cursor: usize,
    pub outcomes: &'a Outcomes,
    pub font: &'a FontId,
}

/// Paints the gutter `width` wide left of `galley` at `galley_pos`; returns the statement whose
/// ▶ was clicked.
pub fn paint(
    ui: &Ui,
    galley: &Arc<Galley>,
    galley_pos: Pos2,
    width: f32,
    lines: &Lines<'_>,
) -> Option<Range<usize>> {
    let p = theme::palette(ui);
    let left = galley_pos.x - width - ui.spacing().item_spacing.x;
    let painter = ui.painter();
    let starts = line_starts(lines.text);
    let cursor_line = starts
        .partition_point(|&s| s <= lines.cursor)
        .saturating_sub(1);
    let statements = text::statements(lines.text);
    let current = text::statement_at(lines.text, lines.cursor);
    let mut clicked = None;
    let mut line = 0;
    let mut new_line = true;
    for row in &galley.rows {
        let rect = row.rect().translate(galley_pos.to_vec2());
        if new_line {
            let failed = statement_on(&statements, &starts, line)
                .and_then(|r| lines.text.get(r.clone()))
                .and_then(|s| lines.outcomes.get(s))
                .is_some_and(|(o, _)| matches!(o, Outcome::Failed(_)));
            let colour = if failed {
                p.fail
            } else if line == cursor_line {
                p.text
            } else {
                p.muted
            };
            painter.text(
                Pos2::new(left + width - PLAY, rect.center().y),
                Align2::RIGHT_CENTER,
                (line + 1).to_string(),
                lines.font.clone(),
                colour,
            );
            if let Some(statement) = statement_on(&statements, &starts, line) {
                let row_rect = Rect::from_min_max(
                    Pos2::new(left, rect.top()),
                    Pos2::new(left + width, rect.bottom()),
                );
                let is_current = current.as_ref().is_some_and(|c| c.start == statement.start);
                if play(ui, row_rect, line, is_current) {
                    clicked = Some(statement.clone());
                }
                if let Some(text) = lines.text.get(statement.clone())
                    && let Some((outcome, took)) = lines.outcomes.get(text)
                {
                    mark(ui, row_rect, line, outcome, *took);
                }
            }
            line += 1;
        }
        new_line = row.ends_with_newline;
    }
    clicked
}

/// The byte offset each line starts at.
fn line_starts(text: &str) -> Vec<usize> {
    std::iter::once(0)
        .chain(text.match_indices('\n').map(|(i, _)| i + 1))
        .collect()
}

/// The statement whose first line is `line`.
fn statement_on<'a>(
    statements: &'a [Range<usize>],
    starts: &[usize],
    line: usize,
) -> Option<&'a Range<usize>> {
    let from = *starts.get(line)?;
    let to = starts.get(line + 1).copied().unwrap_or(usize::MAX);
    statements.iter().find(|r| (from..to).contains(&r.start))
}

/// The ▶ at the right of the gutter's row; returns whether it was clicked.
fn play(ui: &Ui, row: Rect, line: usize, current: bool) -> bool {
    let rect = Rect::from_min_max(Pos2::new(row.right() - PLAY, row.top()), row.max);
    let response = ui
        .interact(
            rect,
            ui.scope_id().with(("gutter-run", line)),
            Sense::click(),
        )
        .on_hover_text("Run this statement");
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::accesskit::Role::Button,
            true,
            format!("Run the statement on line {}", line + 1),
        )
    });
    let p = theme::palette(ui);
    let colour = if response.hovered() {
        p.accent
    } else if current {
        p.text
    } else {
        p.border
    };
    let size = (row.height().min(14.0) * 0.8).max(6.0);
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        icons::PLAY,
        FontId::new(size, egui::FontFamily::Name(icons::FAMILY.into())),
        colour,
    );
    response.clicked()
}

/// The outcome mark at the left of the gutter's row, which says the outcome on hover.
fn mark(ui: &Ui, row: Rect, line: usize, outcome: &Outcome, took: Duration) {
    let rect = Rect::from_min_size(row.min, vec2(MARK, row.height()));
    let p = theme::palette(ui);
    match outcome {
        Outcome::Failed(_) => {
            ui.painter().circle_filled(rect.center(), 3.5, p.fail);
        }
        Outcome::Rows { .. } | Outcome::Changed(_) | Outcome::Done => {
            let size = (row.height().min(14.0) * 0.8).max(6.0);
            ui.painter().text(
                rect.center(),
                Align2::CENTER_CENTER,
                icons::CHECK_CIRCLE,
                FontId::new(size, egui::FontFamily::Name(icons::FAMILY.into())),
                p.ok,
            );
        }
    }
    ui.interact(
        rect,
        ui.scope_id().with(("gutter-outcome", line)),
        Sense::hover(),
    )
    .on_hover_text(outcome.said(took));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outcomes_are_kept_by_the_statement_with_blanks_made_single_r_sql_62() {
        let mut outcomes = Outcomes::default();
        let took = Duration::from_millis(1500);
        outcomes.note(
            "SELECT *\n  FROM kunder;",
            Outcome::Rows {
                count: 3,
                more: true,
            },
            took,
        );
        let (outcome, _) = outcomes
            .get("SELECT * FROM kunder")
            .expect("the same statement");
        assert_eq!(outcome.said(took), "3 rows, more to fetch in 1.50 s");
        assert!(outcomes.get("SELECT * FROM ordrer").is_none());
        assert_eq!(
            Outcome::Failed("SQL0204: KUNDER i *LIBL findes ikke.".to_owned()).said(took),
            "SQL0204: KUNDER i *LIBL findes ikke."
        );
    }

    #[test]
    fn a_statement_belongs_to_the_line_it_starts_on_r_sql_62() {
        let text = "SELECT 1;\n\nVALUES æ;\n";
        let starts = line_starts(text);
        let statements = text::statements(text);
        assert_eq!(starts, [0, 10, 11, 22]);
        assert!(statement_on(&statements, &starts, 0).is_some());
        assert!(statement_on(&statements, &starts, 1).is_none());
        let second = statement_on(&statements, &starts, 2).expect("a statement");
        assert_eq!(&text[second.clone()], "VALUES æ");
        assert!(statement_on(&statements, &starts, 9).is_none());
    }
}
