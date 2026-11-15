//! Parameter markers in a statement (R-SQL-40, D23): `?` and `:name` outside strings, delimited
//! names and comments. `JTOpen` takes only `?`, so a `:name` becomes `?` and its value is sent
//! once for each place it stands.

use crate::text::is_name_char;

/// The markers of a statement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Markers {
    /// The statement with every marker as `?`.
    pub sql: String,
    /// The form's fields, in order of first use: `1`, `2`, … for `?`, `:NAME` once per name.
    pub fields: Vec<String>,
    /// For each `?` in `sql`, in order, the field whose value it takes.
    pub slots: Vec<usize>,
}

impl Markers {
    /// The values to send, one per `?`, from the fields' values (`None` for null).
    pub fn values(&self, fields: &[Option<String>]) -> Vec<Option<String>> {
        self.slots
            .iter()
            .map(|&f| fields.get(f).cloned().flatten())
            .collect()
    }
}

fn is_name_start(c: char) -> bool {
    c.is_alphabetic() || matches!(c, '_' | '#' | '$' | '@')
}

/// The markers in `statement`, or `None` when it has none.
pub fn markers(statement: &str) -> Option<Markers> {
    let mut sql = String::with_capacity(statement.len());
    let mut fields: Vec<String> = Vec::new();
    let mut slots = Vec::new();
    let mut positional = 0;
    let mut chars = statement.chars().peekable();
    let mut previous = ' ';
    while let Some(c) = chars.next() {
        match c {
            '\'' | '"' => {
                sql.push(c);
                // A doubled quote inside stays inside: it closes and opens again.
                for d in chars.by_ref() {
                    sql.push(d);
                    if d == c {
                        break;
                    }
                }
            }
            '-' if chars.peek() == Some(&'-') => {
                sql.push(c);
                for d in chars.by_ref() {
                    sql.push(d);
                    if d == '\n' {
                        break;
                    }
                }
            }
            '/' if chars.peek() == Some(&'*') => {
                sql.push(c);
                let mut last = ' ';
                for d in chars.by_ref() {
                    sql.push(d);
                    if last == '*' && d == '/' {
                        break;
                    }
                    last = d;
                }
            }
            '?' => {
                positional += 1;
                fields.push(positional.to_string());
                slots.push(fields.len() - 1);
                sql.push('?');
            }
            ':' if !is_name_char(previous) && chars.peek().is_some_and(|&d| is_name_start(d)) => {
                let mut name = String::from(":");
                while let Some(&d) = chars.peek().filter(|&&d| is_name_char(d)) {
                    name.extend(d.to_uppercase());
                    chars.next();
                }
                let field = fields.iter().position(|f| *f == name).unwrap_or_else(|| {
                    fields.push(name);
                    fields.len() - 1
                });
                slots.push(field);
                sql.push('?');
            }
            _ => sql.push(c),
        }
        previous = c;
    }
    (!slots.is_empty()).then_some(Markers { sql, fields, slots })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn question_marks_and_names_become_question_marks_r_sql_40() {
        let m = markers("SELECT * FROM T WHERE A = ? AND B = :navn OR C = :Navn AND D = ?")
            .expect("markers");
        assert_eq!(
            m.sql,
            "SELECT * FROM T WHERE A = ? AND B = ? OR C = ? AND D = ?"
        );
        assert_eq!(m.fields, ["1", ":NAVN", "2"]);
        assert_eq!(m.slots, [0, 1, 1, 2]);
        let values = m.values(&[Some("x".to_owned()), Some("æøå".to_owned()), None]);
        assert_eq!(
            values,
            [
                Some("x".to_owned()),
                Some("æøå".to_owned()),
                Some("æøå".to_owned()),
                None
            ]
        );
    }

    #[test]
    fn markers_in_strings_names_and_comments_are_left_alone_r_sql_40() {
        for sql in [
            "SELECT '?', 'it''s :x' FROM T",
            "SELECT \"A?\" FROM T -- where a = ?",
            "SELECT 1 /* :x ? */ FROM T",
            "VALUES TIME('12:30:00')",
            "CL: DSPLIB QGPL",
            "SELECT X:Y FROM T",
            "",
        ] {
            assert_eq!(markers(sql), None, "{sql}");
        }
    }

    #[test]
    fn a_truncated_statement_is_not_a_panic_r_sql_40() {
        assert_eq!(markers("SELECT 'open"), None);
        assert_eq!(markers("SELECT /* open"), None);
        assert_eq!(markers(":").map(|m| m.slots), None);
        assert_eq!(markers("? ").map(|m| m.fields), Some(vec!["1".to_owned()]));
    }
}
