//! The form for a statement's parameter markers (R-SQL-40, D23): a field per `?` and per
//! `:name`, each with a switch for null; the values are remembered per statement.

use egui::{RichText, TextEdit};
use parley_sql_core::params::Markers;

use parley_ui_egui::{dialog, theme};

/// A field's text and whether it is null.
pub type Value = (String, bool);

/// The form, open while a statement with markers waits for its values.
#[derive(Debug, Clone)]
pub struct ParamForm {
    /// The statement as written, for the history and to remember the values by.
    pub statement: String,
    pub markers: Markers,
    pub values: Vec<Value>,
}

/// What the operator did with the form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Run with these values, one per `?` of the marked statement (`None` for null).
    Run(Vec<Option<String>>),
    Cancel,
}

impl ParamForm {
    /// The form for `statement`, filled with the values used last for it, if any.
    pub fn new(statement: String, markers: Markers, remembered: Option<&Vec<Value>>) -> Self {
        let values = match remembered {
            Some(values) if values.len() == markers.fields.len() => values.clone(),
            _ => vec![(String::new(), false); markers.fields.len()],
        };
        Self {
            statement,
            markers,
            values,
        }
    }

    /// The label of field `index`: "Parameter 1" for a `?`, the name for a `:name`.
    fn label(&self, index: usize) -> String {
        let field = self.markers.fields.get(index).map_or("", String::as_str);
        if field.starts_with(':') {
            field.to_owned()
        } else {
            format!("Parameter {field}")
        }
    }

    /// Draws the form; what was done, once something was.
    pub fn show(&mut self, ctx: &egui::Context) -> Option<Outcome> {
        let labels: Vec<String> = (0..self.values.len()).map(|i| self.label(i)).collect();
        let statement = self.statement.clone();
        let values = &mut self.values;
        let (close, (cancel, run)) = dialog::dialog(
            ctx,
            "sql-parameters",
            "Values for the parameters",
            480.0,
            |ui| {
                ui.label(RichText::new(statement.trim()).monospace());
                ui.add_space(8.0);
                egui::Grid::new("sql-parameters")
                    .num_columns(3)
                    .spacing([12.0, 8.0])
                    .show(ui, |ui| {
                        for (label, (text, null)) in labels.iter().zip(values.iter_mut()) {
                            let name = ui.label(label.as_str());
                            let field = TextEdit::singleline(text).hint_text("value");
                            ui.add_enabled_ui(!*null, |ui| {
                                theme::sized_edit(ui, 240.0, field).labelled_by(name.id);
                            });
                            ui.horizontal(|ui| {
                                theme::switch(ui, null)
                                    .labelled_by(name.id)
                                    .on_hover_text("Send NULL instead of the text");
                                ui.label("NULL");
                            });
                            ui.end_row();
                        }
                    });
                theme::hint(
                    ui,
                    "Each value goes to IBM i as text, which converts it to the parameter's type.",
                );
                dialog::actions(ui, theme::primary_button(ui, "Run"), true)
            },
        );
        if close || cancel {
            Some(Outcome::Cancel)
        } else if run {
            let fields: Vec<Option<String>> = self
                .values
                .iter()
                .map(|(text, null)| (!null).then(|| text.clone()))
                .collect();
            Some(Outcome::Run(self.markers.values(&fields)))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use egui_kittest::kittest::Queryable;
    use parley_sql_core::params::markers;

    use super::*;

    fn form(statement: &str) -> ParamForm {
        let m = markers(statement).expect("markers");
        ParamForm::new(statement.to_owned(), m, None)
    }

    fn drawn(form: ParamForm) -> egui_kittest::Harness<'static, (ParamForm, Option<Outcome>)> {
        egui_kittest::Harness::builder()
            .with_size(egui::vec2(800.0, 600.0))
            .build_ui_state(
                |ui, (form, outcome): &mut (ParamForm, Option<Outcome>)| {
                    parley_ui_egui::test_support::install_fonts(ui);
                    if let Some(done) = form.show(ui.ctx()) {
                        *outcome = Some(done);
                    }
                },
                (form, None),
            )
    }

    #[test]
    fn the_form_keeps_its_buttons_in_place_and_sends_a_value_per_marker_r_sql_40() {
        let mut f = form("SELECT * FROM T WHERE A = :navn OR B = :navn AND C = ?");
        assert_eq!(f.label(0), ":NAVN");
        assert_eq!(f.label(1), "Parameter 1");
        f.values = vec![("æøå".to_owned(), false), (String::new(), true)];
        let mut harness = drawn(f);
        harness.run();
        let run = harness.get_by_label("Run").rect();
        harness.run_steps(5);
        assert_eq!(harness.get_by_label("Run").rect(), run);
        harness.get_by_label("Run").click();
        harness.run();
        assert_eq!(
            harness.state().1,
            Some(Outcome::Run(vec![
                Some("æøå".to_owned()),
                Some("æøå".to_owned()),
                None
            ]))
        );
    }

    #[test]
    fn remembered_values_fill_the_form_when_they_still_fit_r_sql_40() {
        let statement = "SELECT * FROM T WHERE A = ?";
        let m = markers(statement).expect("markers");
        let kept = vec![("42".to_owned(), false)];
        let f = ParamForm::new(statement.to_owned(), m.clone(), Some(&kept));
        assert_eq!(f.values, kept);
        let stale = vec![("1".to_owned(), false), ("2".to_owned(), false)];
        let f = ParamForm::new(statement.to_owned(), m, Some(&stale));
        assert_eq!(f.values, [(String::new(), false)]);
    }

    #[test]
    fn cancel_closes_the_form_r_sql_40() {
        let mut harness = drawn(form("VALUES ?"));
        harness.run();
        harness.get_by_label("Cancel").click();
        harness.run();
        assert_eq!(harness.state().1, Some(Outcome::Cancel));
    }
}
