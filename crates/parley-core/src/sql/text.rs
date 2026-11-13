//! SQL text as the SQL window needs it, without parsing SQL: where statements end (R-SQL-7)
//! and whether one changes data in bulk (R-SQL-9). A small scanner skips strings (`'…'`, with
//! `''` inside), delimited names (`"…"`, with `""` inside) and comments (`--` to the end of the
//! line, `/* … */`), as Db2 for i writes them.

use std::ops::Range;

/// A piece of SQL text that matters here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Token {
    /// A word outside strings, names and comments.
    Word,
    /// `;`, which ends a statement.
    End,
}

/// The words and statement ends of `sql`, with their byte ranges.
fn tokens(sql: &str) -> Vec<(Token, Range<usize>)> {
    let bytes = sql.as_bytes();
    let mut out = Vec::new();
    let mut at = 0;
    while let Some(&b) = bytes.get(at) {
        match b {
            b'\'' | b'"' => at = past_quoted(bytes, at, b),
            b'-' if bytes.get(at + 1) == Some(&b'-') => {
                at = bytes
                    .get(at..)
                    .and_then(|rest| rest.iter().position(|&c| c == b'\n'))
                    .map_or(bytes.len(), |n| at + n + 1);
            }
            b'/' if bytes.get(at + 1) == Some(&b'*') => {
                at = sql
                    .get(at + 2..)
                    .and_then(|rest| rest.find("*/"))
                    .map_or(bytes.len(), |n| at + 2 + n + 2);
            }
            b';' => {
                out.push((Token::End, at..at + 1));
                at += 1;
            }
            _ if is_word_byte(b) => {
                let start = at;
                while bytes.get(at).copied().is_some_and(is_word_byte) {
                    at += 1;
                }
                out.push((Token::Word, start..at));
            }
            _ => at += 1,
        }
    }
    out
}

/// Letters (also non-ASCII), digits, `_`, `#`, `$` and `@`, as in IBM i names.
const fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'_' | b'#' | b'$' | b'@') || b >= 0x80
}

/// The index just past the string or name that starts at `at` with `quote`; a doubled quote
/// stays inside, and an unclosed one runs to the end.
fn past_quoted(bytes: &[u8], at: usize, quote: u8) -> usize {
    let mut i = at + 1;
    while let Some(&b) = bytes.get(i) {
        if b == quote {
            if bytes.get(i + 1) == Some(&quote) {
                i += 2;
                continue;
            }
            return i + 1;
        }
        i += 1;
    }
    bytes.len()
}

/// Each statement of `sql`, trimmed and without its `;`, as byte ranges; empty ones left out.
pub fn statements(sql: &str) -> Vec<Range<usize>> {
    segments(sql)
        .into_iter()
        .filter_map(|segment| trimmed(sql, segment))
        .collect()
}

/// The statement at byte `cursor` (R-SQL-7): the one whose text, or the `;` and blanks after
/// it, hold the cursor; in blanks after the last statement, that statement.
pub fn statement_at(sql: &str, cursor: usize) -> Option<Range<usize>> {
    let mut last = None;
    for segment in segments(sql) {
        let found = trimmed(sql, segment.clone());
        if found.is_some() {
            last.clone_from(&found);
        }
        if cursor <= segment.end {
            return found.or(last);
        }
    }
    last
}

/// The text between statement ends: up to and including each `;`, then the rest.
fn segments(sql: &str) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    let mut start = 0;
    for (token, range) in tokens(sql) {
        if token == Token::End {
            out.push(start..range.end);
            start = range.end;
        }
    }
    if start < sql.len() {
        out.push(start..sql.len());
    }
    out
}

/// `range` of `sql` without blanks, comments and its `;` at either end; `None` when nothing is
/// left.
fn trimmed(sql: &str, range: Range<usize>) -> Option<Range<usize>> {
    let text = sql.get(range.clone())?;
    let words: Vec<Range<usize>> = tokens(text)
        .into_iter()
        .filter(|(token, _)| *token == Token::Word)
        .map(|(_, r)| r)
        .collect();
    let first = words.first()?;
    // From the first word to the last character that is not a blank or the `;`.
    let end = text.trim_end().trim_end_matches(';').trim_end().len();
    let start = text
        .get(..first.start)?
        .rfind(['('])
        .map_or(first.start, |p| {
            // A statement may open with a parenthesis: `(SELECT …) UNION …`.
            if text
                .get(p + 1..first.start)
                .is_some_and(|s| s.trim().is_empty())
            {
                p
            } else {
                first.start
            }
        });
    (start < end).then(|| range.start + start..range.start + end)
}

/// A statement that changes much at once and is confirmed before it runs (R-SQL-9).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Risk {
    DeleteWithoutWhere,
    UpdateWithoutWhere,
    Drop,
    Truncate,
    Alter,
}

impl Risk {
    /// What the confirmation says it does.
    pub const fn describe(self) -> &'static str {
        match self {
            Self::DeleteWithoutWhere => "deletes every row of the table: it has no WHERE",
            Self::UpdateWithoutWhere => "changes every row of the table: it has no WHERE",
            Self::Drop => "drops an object, with its data",
            Self::Truncate => "deletes every row of the table",
            Self::Alter => "changes the definition of an object",
        }
    }
}

/// Whether `statement` needs confirming. It reads only the first word and whether `WHERE`
/// appears outside strings, names and comments: a guard against slips, not a parser.
pub fn risk(statement: &str) -> Option<Risk> {
    let words: Vec<String> = tokens(statement)
        .into_iter()
        .filter(|(token, _)| *token == Token::Word)
        .filter_map(|(_, r)| statement.get(r).map(str::to_ascii_uppercase))
        .collect();
    let has_where = words.iter().any(|w| w == "WHERE");
    match words.first().map(String::as_str)? {
        "DELETE" if !has_where => Some(Risk::DeleteWithoutWhere),
        "UPDATE" if !has_where => Some(Risk::UpdateWithoutWhere),
        "DROP" => Some(Risk::Drop),
        "TRUNCATE" => Some(Risk::Truncate),
        "ALTER" => Some(Risk::Alter),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(sql: &str) -> Vec<&str> {
        statements(sql)
            .into_iter()
            .filter_map(|r| sql.get(r))
            .collect()
    }

    #[test]
    fn statements_end_at_semicolons_outside_strings_names_and_comments_r_sql_7() {
        let sql = "SELECT ';' FROM \"A;B\";\n-- a comment; with a semicolon\nVALUES 'it''s; fine' /* ; */ ;  \n\n  ";
        // The comment before VALUES is not part of the statement; the one inside it is.
        assert_eq!(
            texts(sql),
            ["SELECT ';' FROM \"A;B\"", "VALUES 'it''s; fine' /* ; */"]
        );
    }

    #[test]
    fn empty_text_and_lone_semicolons_have_no_statements_r_sql_7() {
        for sql in ["", "   ", ";;", " ; -- only a comment\n ;", "/* nothing */"] {
            assert!(statements(sql).is_empty(), "{sql:?}");
        }
    }

    #[test]
    fn the_statement_under_the_cursor_is_the_one_run_r_sql_7() {
        let sql = "SELECT 1 FROM T1;\nSELECT 'æøå' FROM T2;\n\n";
        let at = |cursor| statement_at(sql, cursor).and_then(|r| sql.get(r));
        assert_eq!(at(0), Some("SELECT 1 FROM T1"));
        assert_eq!(at(16), Some("SELECT 1 FROM T1"), "on its semicolon");
        assert_eq!(at(20), Some("SELECT 'æøå' FROM T2"));
        assert_eq!(
            at(sql.len()),
            Some("SELECT 'æøå' FROM T2"),
            "in blanks after the last"
        );
        assert_eq!(statement_at("", 0), None);
    }

    #[test]
    fn an_unclosed_string_does_not_end_the_text_early_r_sql_7() {
        assert_eq!(
            texts("SELECT 'open; still open"),
            ["SELECT 'open; still open"]
        );
    }

    #[test]
    fn a_statement_may_open_with_a_parenthesis_r_sql_7() {
        assert_eq!(
            texts(" (SELECT 1 FROM A) UNION (SELECT 2 FROM B);"),
            ["(SELECT 1 FROM A) UNION (SELECT 2 FROM B)"]
        );
    }

    #[test]
    fn bulk_changes_are_caught_and_ordinary_statements_are_not_r_sql_9() {
        for (sql, risk_found) in [
            ("delete from orders", Some(Risk::DeleteWithoutWhere)),
            ("DELETE FROM ORDERS WHERE ID = 1", None),
            ("UPDATE T SET A = 'WHERE'", Some(Risk::UpdateWithoutWhere)),
            (
                "UPDATE T SET A = 1 -- WHERE in a comment",
                Some(Risk::UpdateWithoutWhere),
            ),
            ("update t set a = 1 where b = 2", None),
            ("/* first */ DROP TABLE T", Some(Risk::Drop)),
            ("TRUNCATE TABLE T", Some(Risk::Truncate)),
            ("ALTER TABLE T ADD COLUMN C INT", Some(Risk::Alter)),
            ("SELECT * FROM \"DELETE\"", None),
            ("INSERT INTO T VALUES ('æøå')", None),
            ("", None),
        ] {
            assert_eq!(risk(sql), risk_found, "{sql}");
        }
    }
}
