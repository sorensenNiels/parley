//! The place of an error marked in the editor (R-SQL-75): what the host's message names, found
//! in the statement that ran, underlined until the text changes.

use std::ops::Range;

use super::Scripts;

/// An error's place in the shown script: its byte range, the message, and the text's generation
/// it was found in (an edit drops it).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorMark {
    range: Range<usize>,
    message: String,
    generation: u64,
}

impl Scripts {
    /// Marks where `message` says `statement`, as it ran from the shown script, stopped.
    pub fn mark_error(&mut self, statement: &str, message: &str) {
        self.error_mark = None;
        let Some(place) = parley_core::sql::message::error_place(statement, message) else {
            return;
        };
        let Some(at) = self.text().find(statement) else {
            return;
        };
        self.error_mark = Some(ErrorMark {
            range: at + place.start..at + place.end,
            message: message.to_owned(),
            generation: self.generation,
        });
    }

    /// The error's place while the text is the one it was found in.
    pub fn error_place(&self) -> Option<Range<usize>> {
        self.error_mark
            .as_ref()
            .filter(|m| m.generation == self.generation)
            .map(|m| m.range.clone())
    }

    /// The message to show while the pointer is on the error's place.
    pub(super) fn error_at(&self, at: usize) -> Option<&str> {
        let mark = self.error_mark.as_ref()?;
        (mark.generation == self.generation && mark.range.contains(&at))
            .then_some(mark.message.as_str())
    }

    /// Puts the cursor on the error's place, selecting it.
    pub fn go_to_error(&mut self) {
        if let Some(range) = self.error_place() {
            self.select_range((range.start, range.end));
        }
    }

    /// A new run: the old mark goes.
    pub fn clear_error_mark(&mut self) {
        self.error_mark = None;
    }
}

#[cfg(test)]
mod tests {
    use crate::sql::tests::window;
    use parley_core::sql::protocol::SqlError;
    use parley_core::sql::runner::Event;

    fn failed(message: &str) -> Event {
        Event {
            tag: 0,
            result: Err(parley_core::Error::Sql(SqlError {
                message: message.to_owned(),
                sqlstate: Some("42601".to_owned()),
                sqlcode: -104,
            })),
        }
    }

    #[test]
    fn a_failed_statement_marks_where_it_stopped_until_the_text_changes_r_sql_75() {
        let mut w = window();
        let text = "VALUES 'æøå';\nSELECT X FORM T";
        w.scripts.set_text(text, text.len());
        w.on_run(
            "SELECT X FORM T",
            failed("[SQL0104] Token FORM was not valid. Valid tokens: FROM INTO."),
        );
        let place = w.scripts.error_place().expect("marked");
        assert_eq!(text.get(place.clone()), Some("FORM"));
        assert_eq!(
            w.scripts.error_at(place.start),
            Some(
                "[SQL0104] Token FORM was not valid. Valid tokens: FROM INTO. (SQLSTATE 42601, SQLCODE -104)"
            )
        );
        w.scripts.go_to_error();
        assert_eq!(w.scripts.selection, (place.start, place.end), "selected");
        w.scripts.insert(" ");
        assert_eq!(w.scripts.error_place(), None, "an edit drops it");
    }

    #[test]
    fn an_error_without_a_place_marks_nothing_r_sql_75() {
        let mut w = window();
        w.scripts.set_text("SELECT * FROM T", 0);
        w.on_run(
            "SELECT * FROM T",
            failed("[SQL0551] Not authorized to object T in L type *FILE."),
        );
        assert_eq!(w.scripts.error_place(), None);
    }
}
