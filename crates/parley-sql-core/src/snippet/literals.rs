//! The values written in a statement that a snippet could ask for instead (R-SQL-111):
//! `WHERE USER_NAME = 'QSYSOPR'` offers `'QSYSOPR'`, which becomes
//! `${user_name as text:QSYSOPR}`.

use std::ops::Range;

use super::placeholder::{ValueType, masked, placeholders, spans};
use crate::text::{self, Kind};

/// A string or number literal that could be asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Literal {
    /// Where it is in the SQL, quotes included.
    pub range: Range<usize>,
    /// As written in the SQL.
    pub written: String,
    /// The placeholder's name: the column it is compared with, or `value`, unique.
    pub name: String,
    pub kind: ValueType,
    /// The placeholder's default: the value without quotes.
    pub value: String,
}

impl Literal {
    /// The placeholder put in its place.
    pub fn placeholder(&self) -> String {
        format!("${{{} as {}:{}}}", self.name, self.kind.word(), self.value)
    }
}

/// The string and number literals of `sql` that can become placeholders, in order; not those in
/// comments, delimited names or placeholders, not `X'…'` and the like, and not one holding `}`.
pub fn literals(sql: &str) -> Vec<Literal> {
    let mut taken: Vec<String> = placeholders(sql).into_iter().map(|p| p.name).collect();
    let inside_placeholders: Vec<Range<usize>> = spans(sql).into_iter().map(|(r, _)| r).collect();
    let mut found = Vec::new();
    for (range, kind) in text::highlight(&masked(sql)) {
        let in_placeholder = inside_placeholders
            .iter()
            .any(|p| range.start < p.end && p.start < range.end);
        let Some(written) = sql.get(range.clone()).filter(|_| !in_placeholder) else {
            continue;
        };
        let (kind, value) = match kind {
            Kind::String => {
                // `X'…'`, `G'…'`, `UX'…'`: a letter before the quote is part of the literal.
                let prefixed = sql
                    .get(..range.start)
                    .and_then(|b| b.chars().next_back())
                    .is_some_and(char::is_alphanumeric);
                let Some(inside) = written
                    .strip_prefix('\'')
                    .and_then(|w| w.strip_suffix('\''))
                else {
                    continue;
                };
                if prefixed || inside.contains("${") {
                    continue;
                }
                let value = inside.replace("''", "'");
                let kind = if ValueType::Date.render(&value).is_ok() {
                    ValueType::Date
                } else {
                    ValueType::Text
                };
                (kind, value)
            }
            Kind::Number => (ValueType::Number, written.to_owned()),
            Kind::Keyword | Kind::Name | Kind::Comment => continue,
        };
        if value.contains('}') {
            continue;
        }
        let name = unique(
            &column_before(sql.get(..range.start).unwrap_or_default()),
            &taken,
        );
        taken.push(name.clone());
        found.push(Literal {
            range,
            written: written.to_owned(),
            name,
            kind,
            value,
        });
    }
    found
}

/// `sql` with `literal` replaced by its placeholder.
pub fn ask_for(sql: &str, literal: &Literal) -> String {
    let mut out = sql
        .get(..literal.range.start)
        .unwrap_or_default()
        .to_owned();
    out.push_str(&literal.placeholder());
    out.push_str(sql.get(literal.range.end..).unwrap_or_default());
    out
}

/// The column a literal after `before` is compared with, in lower case: `T.USER_NAME =` →
/// `user_name`; `value` when there is none.
fn column_before(before: &str) -> String {
    let before = before.trim_end();
    let operator = ["<>", "!=", "<=", ">=", "=", "<", ">"]
        .iter()
        .find_map(|op| before.strip_suffix(op))
        .or_else(|| strip_word(before, "LIKE"))
        .or_else(|| {
            let open = before.strip_suffix('(')?.trim_end();
            strip_word(open, "IN")
        });
    let Some(rest) = operator else {
        return "value".to_owned();
    };
    let rest = rest.trim_end();
    let start = rest
        .rfind(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '_' | '$' | '#' | '@')))
        .map_or(0, |i| i + 1);
    let column = rest.get(start..).unwrap_or_default().to_ascii_lowercase();
    let starts_ok = column
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || matches!(c, '$' | '#' | '@'));
    if starts_ok {
        column
    } else {
        "value".to_owned()
    }
}

/// `text` without `word` at its end, as a whole word in any case.
fn strip_word<'a>(text: &'a str, word: &str) -> Option<&'a str> {
    let cut = text.len().checked_sub(word.len())?;
    let (rest, end) = (text.get(..cut)?, text.get(cut..)?);
    let whole = rest
        .chars()
        .next_back()
        .is_none_or(|c| !(c.is_alphanumeric() || c == '_'));
    (end.eq_ignore_ascii_case(word) && whole).then_some(rest)
}

/// `name`, or `name_2`, `name_3`… when it is taken.
fn unique(name: &str, taken: &[String]) -> String {
    if !taken.iter().any(|t| t == name) {
        return name.to_owned();
    }
    // One more number than there are names taken always finds a free one.
    (2..=taken.len() + 2)
        .map(|n| format!("{name}_{n}"))
        .find(|candidate| !taken.contains(candidate))
        .unwrap_or_else(|| name.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn offered(sql: &str) -> Vec<(String, String, String)> {
        literals(sql)
            .into_iter()
            .map(|l| (l.placeholder(), l))
            .map(|(p, l)| (l.written, l.name, p))
            .collect()
    }

    fn row(written: &str, name: &str, placeholder: &str) -> (String, String, String) {
        (written.to_owned(), name.to_owned(), placeholder.to_owned())
    }

    #[test]
    fn literals_are_named_after_the_column_they_are_compared_with_r_sql_111() {
        let sql = "SELECT * FROM QSYS2.USER_INFO u WHERE u.USER_NAME = 'QSYSOPR' AND STORAGE >= 100 \
                   AND TEXT like 'æ%' AND STATUS IN ('A', 'B') FETCH FIRST 10 ROWS ONLY";
        assert_eq!(
            offered(sql),
            [
                row("'QSYSOPR'", "user_name", "${user_name as text:QSYSOPR}"),
                row("100", "storage", "${storage as number:100}"),
                row("'æ%'", "text", "${text as text:æ%}"),
                row("'A'", "status", "${status as text:A}"),
                row("'B'", "value", "${value as text:B}"),
                row("10", "value_2", "${value_2 as number:10}"),
            ]
        );
    }

    #[test]
    fn a_date_string_becomes_a_date_and_quotes_are_undoubled_r_sql_111() {
        assert_eq!(
            offered("WHERE D = '2026-11-25' AND N = 'O''Brien' AND X = '2026-02-30'"),
            [
                row("'2026-11-25'", "d", "${d as date:2026-11-25}"),
                row("'O''Brien'", "n", "${n as text:O'Brien}"),
                row("'2026-02-30'", "x", "${x as text:2026-02-30}"),
            ]
        );
    }

    #[test]
    fn comments_names_placeholders_hex_and_braces_are_not_offered_r_sql_111() {
        let sql = "SELECT \"col 1\" FROM T -- WHERE A = 'x'\n/* 5 */ WHERE A = '${a}' AND B = X'C1' \
                   AND C = '}' AND T1 = ${n as number:1}";
        assert_eq!(offered(sql), []);
        assert_eq!(offered(""), []);
        assert_eq!(offered("SELECT 'open"), [], "a string never closed");
    }

    #[test]
    fn a_quote_in_a_placeholder_s_default_does_not_hide_the_values_after_it_r_sql_111() {
        assert_eq!(
            offered("WHERE N = ${n as text:O'Brien} AND U = 'X'"),
            [row("'X'", "u", "${u as text:X}")]
        );
    }

    #[test]
    fn a_name_taken_by_a_placeholder_is_made_unique_r_sql_111() {
        assert_eq!(
            offered("WHERE U = ${u as text} OR U = 'X'"),
            [row("'X'", "u_2", "${u_2 as text:X}")]
        );
    }

    #[test]
    fn asking_for_a_literal_puts_its_placeholder_in_its_place_r_sql_111() {
        let sql = "SELECT * FROM T WHERE NAVN = 'Søren' AND N > 5";
        let found = literals(sql);
        let first = found.first().expect("two literals");
        let asked = ask_for(sql, first);
        assert_eq!(
            asked,
            "SELECT * FROM T WHERE NAVN = ${navn as text:Søren} AND N > 5"
        );
        let filled = super::super::fill(&asked, |_| None);
        assert_eq!(filled, sql, "the default gives back the statement");
        assert_eq!(literals(&asked).len(), 1, "only 5 is left");
    }
}
