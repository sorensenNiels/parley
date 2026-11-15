//! The editor's text laid out in colour (R-SQL-19), with the statement Run would execute on a
//! faint background (R-SQL-28) and the bracket at the cursor with its partner (R-SQL-56).

use std::ops::Range;

use egui::text::{LayoutJob, TextFormat};
use egui::{Color32, FontId, Ui};
use parley_core::sql::text::{self, Kind};

use crate::theme;

/// Text longer than this is not coloured while it is edited: it would be scanned every frame.
const MAX_COLOURED: usize = 200_000;

/// What is marked besides the colours.
#[derive(Debug, Clone, Default)]
pub struct Marks {
    /// The statement Run would execute (R-SQL-28).
    pub current: Option<Range<usize>>,
    /// The bracket at the cursor and its partner, if it has one (R-SQL-56).
    pub bracket: Option<(usize, Option<usize>)>,
    /// Every place of the name under the cursor (R-SQL-66).
    pub words: Vec<Range<usize>>,
    /// Where the last error stopped, underlined (R-SQL-75).
    pub error: Option<Range<usize>>,
}

impl Marks {
    /// The marks for `text` with `selection` (byte offsets): without a selection, the statement
    /// Run would execute (R-SQL-28) and the bracket at the cursor (R-SQL-56).
    pub fn at(text: &str, (start, end): (usize, usize)) -> Self {
        if start < end {
            return Self::default();
        }
        let words = if text.len() <= MAX_COLOURED {
            parley_core::sql::edit::occurrences(text, start)
        } else {
            Vec::new()
        };
        Self {
            current: text::statement_at(text, start),
            bracket: parley_core::sql::edit::matching_bracket(text, start),
            words,
            error: None,
        }
    }
}

/// `text` laid out for the editor in `font`, in colour unless it is very long (R-SQL-19), as
/// Tortuga's code colours.
pub fn coloured(ui: &Ui, text: &str, font: &FontId, wrap_width: f32, marks: &Marks) -> LayoutJob {
    let p = theme::palette(ui);
    let plain = ui.visuals().text_color();
    let spans = if text.len() <= MAX_COLOURED {
        text::highlight(text)
    } else {
        Vec::new()
    };
    // Only while the text is the one the marks were found in.
    let current = marks.current.clone().filter(|r| r.end <= text.len());
    let is_bracket = |i: usize| matches!(text.as_bytes().get(i), Some(b'(' | b')'));
    let bracket = marks
        .bracket
        .filter(|(at, partner)| is_bracket(*at) && partner.is_none_or(is_bracket));
    let mut cuts_at: Vec<usize> = Vec::new();
    if let Some(c) = &current {
        cuts_at.extend([c.start, c.end]);
    }
    let words: Vec<Range<usize>> = marks
        .words
        .iter()
        .filter(|r| r.end <= text.len())
        .cloned()
        .collect();
    for word in &words {
        cuts_at.extend([word.start, word.end]);
    }
    let error = marks.error.clone().filter(|r| r.end <= text.len());
    if let Some(e) = &error {
        cuts_at.extend([e.start, e.end]);
    }
    if let Some((at, partner)) = bracket {
        cuts_at.extend([at, at + 1]);
        if let Some(partner) = partner {
            cuts_at.extend([partner, partner + 1]);
        }
    }
    let mut job = LayoutJob::default();
    // Appends `range` of `text` in `color`, cut where a mark starts or ends.
    let mut append = |range: Range<usize>, color: Color32| {
        let mut points: Vec<usize> = cuts_at
            .iter()
            .copied()
            .filter(|&c| range.start < c && c < range.end)
            .collect();
        points.extend([range.start, range.end]);
        points.sort_unstable();
        points.dedup();
        for cut in points.windows(2) {
            let (a, b) = (cut[0], cut[1]);
            let Some(piece) = text.get(a..b) else {
                continue;
            };
            let mut format = TextFormat::simple(font.clone(), color);
            if current.as_ref().is_some_and(|c| c.start <= a && b <= c.end) {
                format.background = p.surface_hover;
            }
            if words.iter().any(|w| w.start <= a && b <= w.end) {
                format.background = p.accent.gamma_multiply(0.15);
            }
            if error.as_ref().is_some_and(|e| e.start <= a && b <= e.end) {
                format.underline = egui::Stroke::new(1.5, p.fail);
            }
            match bracket {
                Some((at, Some(partner))) if a == at || a == partner => {
                    format.background = p.accent.gamma_multiply(0.3);
                }
                Some((at, None)) if a == at => format.color = p.fail,
                _ => {}
            }
            job.append(piece, 0.0, format);
        }
    };
    let mut at = 0;
    for (range, kind) in spans {
        if range.start < at {
            continue;
        }
        append(at..range.start, plain);
        let color = match kind {
            Kind::Keyword => p.blue,
            Kind::String => p.ok,
            Kind::Name => p.purple,
            Kind::Number => p.redirect,
            Kind::Comment => p.muted,
        };
        append(range.clone(), color);
        at = range.end;
    }
    append(at..text.len(), plain);
    job.wrap.max_width = wrap_width;
    job
}

#[cfg(test)]
mod tests {
    use super::*;

    fn monospace(ui: &Ui) -> FontId {
        egui::TextStyle::Monospace.resolve(ui.style())
    }

    /// The text of the sections `pick` chooses.
    fn picked(job: &LayoutJob, text: &str, pick: impl Fn(&TextFormat) -> bool) -> String {
        job.sections
            .iter()
            .filter(|s| pick(&s.format))
            .filter_map(|s| text.get(s.byte_range.start.0..s.byte_range.end.0))
            .collect()
    }

    #[test]
    fn the_statement_run_would_execute_is_on_a_faint_background_r_sql_28() {
        let text = "VALUES 1;\nSELECT 'æøå' FROM T;\n";
        let current = text::statement_at(text, 12).expect("a statement");
        let mut harness = egui_kittest::Harness::new_ui(move |ui| {
            crate::test_support::install_fonts(ui);
            let marks = Marks {
                current: Some(current.clone()),
                ..Marks::default()
            };
            let job = coloured(ui, text, &monospace(ui), 400.0, &marks);
            let shaded = picked(&job, text, |f| f.background != Color32::TRANSPARENT);
            assert_eq!(shaded, "SELECT 'æøå' FROM T");
            let plain = coloured(ui, text, &monospace(ui), 400.0, &Marks::default());
            assert_eq!(
                picked(&plain, text, |f| f.background != Color32::TRANSPARENT),
                ""
            );
        });
        harness.run();
    }

    #[test]
    fn the_place_of_an_error_is_underlined_in_the_error_colour_r_sql_75() {
        let mut harness = egui_kittest::Harness::new_ui(|ui| {
            crate::test_support::install_fonts(ui);
            let text = "SELECT BELØB FORM T";
            let at = text.find("FORM").expect("in the text");
            let marks = Marks {
                error: Some(at..at + 4),
                ..Marks::default()
            };
            let job = coloured(ui, text, &monospace(ui), 400.0, &marks);
            let fail = theme::palette(ui).fail;
            assert_eq!(picked(&job, text, |f| f.underline.color == fail), "FORM");
            let past_the_end = Marks {
                error: Some(at..99),
                ..Marks::default()
            };
            let job = coloured(ui, text, &monospace(ui), 400.0, &past_the_end);
            assert_eq!(
                picked(&job, text, |f| f.underline.color == fail),
                "",
                "stale"
            );
        });
        harness.run();
    }

    #[test]
    fn the_bracket_and_its_partner_are_marked_and_a_lone_one_in_the_error_colour_r_sql_56() {
        let mut harness = egui_kittest::Harness::new_ui(|ui| {
            crate::test_support::install_fonts(ui);
            let text = "f('æ', (x))";
            let marks = Marks {
                bracket: parley_core::sql::edit::matching_bracket(text, 2),
                ..Marks::default()
            };
            let job = coloured(ui, text, &monospace(ui), 400.0, &marks);
            assert_eq!(
                picked(&job, text, |f| f.background != Color32::TRANSPARENT),
                "()"
            );
            let text = "f((x)";
            let marks = Marks {
                bracket: parley_core::sql::edit::matching_bracket(text, 2),
                ..Marks::default()
            };
            let job = coloured(ui, text, &monospace(ui), 400.0, &marks);
            let fail = theme::palette(ui).fail;
            assert_eq!(picked(&job, text, |f| f.color == fail), "(");
        });
        harness.run();
    }

    #[test]
    fn every_place_of_the_name_under_the_cursor_is_on_a_background_r_sql_66() {
        let mut harness = egui_kittest::Harness::new_ui(|ui| {
            crate::test_support::install_fonts(ui);
            let text = "SELECT navn FROM kunder WHERE NAVN = 'navn'";
            let marks = Marks::at(text, (9, 9));
            let job = coloured(ui, text, &monospace(ui), 400.0, &marks);
            let accent = theme::palette(ui).accent.gamma_multiply(0.15);
            assert_eq!(picked(&job, text, |f| f.background == accent), "navnNAVN");
        });
        harness.run();
    }
}
