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
    /// A CL command that deletes, clears or ends something (R-SQL-30).
    Command,
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
            Self::Command => "is a CL command that deletes, clears or ends something",
        }
    }
}

/// Whether `statement` needs confirming. It reads only the first word and whether `WHERE`
/// appears outside strings, names and comments: a guard against slips, not a parser.
pub fn risk(statement: &str) -> Option<Risk> {
    if let Some(command) = cl_command(statement) {
        let name = command
            .split(|c: char| c.is_whitespace() || c == '(')
            .next()
            .unwrap_or_default()
            .to_ascii_uppercase();
        // A command may be qualified: QSYS/DLTLIB.
        let name = name.rsplit('/').next().unwrap_or_default();
        let ends = matches!(
            name,
            "PWRDWNSYS" | "ENDSYS" | "ENDSBS" | "ENDTCP" | "ENDJOB"
        );
        return (name.starts_with("DLT") || name.starts_with("CLR") || ends)
            .then_some(Risk::Command);
    }
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

/// The CL command of a statement written `CL: command` (R-SQL-30, as ACS Run SQL Scripts),
/// after any comments before it; `None` for SQL.
pub fn cl_command(statement: &str) -> Option<&str> {
    let first = tokens(statement)
        .into_iter()
        .find(|(token, _)| *token == Token::Word)?
        .1;
    let rest = statement.get(first.end..)?;
    let word = statement.get(first)?;
    (word.eq_ignore_ascii_case("CL") && rest.starts_with(':'))
        .then(|| rest.get(1..).unwrap_or_default().trim())
}

/// The SQL that runs CL `command` in the SQL job: `QSYS2.QCMDEXC`, its quotes doubled.
pub fn cl_call(command: &str) -> String {
    format!("CALL QSYS2.QCMDEXC('{}')", command.replace('\'', "''"))
}

/// Whether `statement` may change the SQL job's current schema or library list (R-SQL-32):
/// a `SET` statement or a CL command.
pub fn may_change_job(statement: &str) -> bool {
    cl_command(statement).is_some()
        || tokens(statement)
            .into_iter()
            .find(|(token, _)| *token == Token::Word)
            .and_then(|(_, r)| statement.get(r))
            .is_some_and(|w| w.eq_ignore_ascii_case("SET"))
}

/// `name` in capitals when it is an IBM i system name (a library, R-SQL-32): 1 to 10 of A–Z,
/// 0–9, `_`, `#`, `$`, `@`, not starting with a digit or `_`.
pub fn system_name(name: &str) -> Option<String> {
    let name = name.trim().to_ascii_uppercase();
    let first_ok = name
        .bytes()
        .next()
        .is_some_and(|b| b.is_ascii_uppercase() || matches!(b, b'#' | b'$' | b'@'));
    let ok = first_ok
        && name.len() <= 10
        && name.bytes().all(|b| {
            b.is_ascii_uppercase() || b.is_ascii_digit() || matches!(b, b'_' | b'#' | b'$' | b'@')
        });
    ok.then_some(name)
}

/// What a piece of SQL is, for its colour in the editor (R-SQL-19).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Keyword,
    /// `'…'`, also `X'…'`'s quoted part.
    String,
    /// A delimited name, `"…"`.
    Name,
    Number,
    Comment,
}

/// SQL's reserved and common words, in capitals and in order, for colouring and completion.
pub(super) const KEYWORDS: [&str; 103] = [
    "ADD",
    "ALL",
    "ALTER",
    "AND",
    "ANY",
    "AS",
    "ASC",
    "BEGIN",
    "BETWEEN",
    "BY",
    "CALL",
    "CASE",
    "CAST",
    "CHECK",
    "COLUMN",
    "COMMIT",
    "CONSTRAINT",
    "CREATE",
    "CROSS",
    "CURRENT",
    "DECLARE",
    "DEFAULT",
    "DELETE",
    "DESC",
    "DISTINCT",
    "DROP",
    "ELSE",
    "END",
    "EXCEPT",
    "EXISTS",
    "FETCH",
    "FIRST",
    "FOR",
    "FOREIGN",
    "FROM",
    "FULL",
    "FUNCTION",
    "GRANT",
    "GROUP",
    "HAVING",
    "IF",
    "IN",
    "INDEX",
    "INNER",
    "INSERT",
    "INTERSECT",
    "INTO",
    "IS",
    "JOIN",
    "KEY",
    "LABEL",
    "LEFT",
    "LIKE",
    "LIMIT",
    "MERGE",
    "NEXT",
    "NOT",
    "NULL",
    "OF",
    "OFFSET",
    "ON",
    "ONLY",
    "OR",
    "ORDER",
    "OUTER",
    "OVER",
    "PARTITION",
    "PRIMARY",
    "PROCEDURE",
    "REFERENCES",
    "RENAME",
    "REPLACE",
    "REVOKE",
    "RIGHT",
    "ROLLBACK",
    "ROW",
    "ROWS",
    "SCHEMA",
    "SELECT",
    "SET",
    "TABLE",
    "THEN",
    "TO",
    "TRIGGER",
    "TRUNCATE",
    "UNION",
    "UNIQUE",
    "UPDATE",
    "USING",
    "VALUES",
    "VIEW",
    "WHEN",
    "WHERE",
    "WITH",
    "WITHOUT",
    "AFTER",
    "BEFORE",
    "EACH",
    "MATCHED",
    "NEW",
    "OLD",
    "RETURN",
    "RETURNS",
];

/// The pieces of `sql` that have a colour, in order and on character boundaries; the rest is
/// plain. The same rules as statement splitting: strings, names and comments first.
pub fn highlight(sql: &str) -> Vec<(Range<usize>, Kind)> {
    let bytes = sql.as_bytes();
    let mut out = Vec::new();
    let mut at = 0;
    let mut command_next = false;
    while let Some(&b) = bytes.get(at) {
        let start = at;
        let kind = match b {
            b'\'' => {
                at = past_quoted(bytes, at, b);
                Some(Kind::String)
            }
            b'"' => {
                at = past_quoted(bytes, at, b);
                Some(Kind::Name)
            }
            b'-' if bytes.get(at + 1) == Some(&b'-') => {
                at = bytes
                    .get(at..)
                    .and_then(|rest| rest.iter().position(|&c| c == b'\n'))
                    .map_or(bytes.len(), |n| at + n);
                Some(Kind::Comment)
            }
            b'/' if bytes.get(at + 1) == Some(&b'*') => {
                at = sql
                    .get(at + 2..)
                    .and_then(|rest| rest.find("*/"))
                    .map_or(bytes.len(), |n| at + 2 + n + 2);
                Some(Kind::Comment)
            }
            b'0'..=b'9' => {
                while bytes
                    .get(at)
                    .is_some_and(|c| c.is_ascii_digit() || matches!(c, b'.' | b'E' | b'e'))
                {
                    at += 1;
                }
                Some(Kind::Number)
            }
            _ if is_word_byte(b) => {
                while bytes.get(at).copied().is_some_and(is_word_byte) {
                    at += 1;
                }
                let word = sql.get(start..at).map(str::to_ascii_uppercase);
                if command_next {
                    // R-SQL-30: the command after `CL:`.
                    command_next = false;
                    Some(Kind::Keyword)
                } else if word.as_deref() == Some("CL") && bytes.get(at) == Some(&b':') {
                    at += 1;
                    command_next = true;
                    Some(Kind::Keyword)
                } else {
                    word.filter(|w| KEYWORDS.contains(&w.as_str()))
                        .map(|_| Kind::Keyword)
                }
            }
            _ => {
                at += 1;
                None
            }
        };
        if let Some(kind) = kind {
            out.push((start..at, kind));
        }
    }
    out
}

/// One piece of SQL as the formatter sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Lexeme<'a> {
    /// A word, and whether it is a keyword.
    Word(&'a str, bool),
    /// A string, name, number or block comment: kept exactly as written.
    Kept(&'a str),
    /// `-- …`: kept, and the line ends after it.
    LineComment(&'a str),
    Punct(char),
}

/// `sql` cut into lexemes, blanks dropped.
fn lexemes(sql: &str) -> Vec<Lexeme<'_>> {
    let mut out = Vec::new();
    let mut highlighted = highlight(sql).into_iter().peekable();
    let bytes = sql.as_bytes();
    let mut at = 0;
    while at < sql.len() {
        if let Some((range, kind)) = highlighted.peek().cloned()
            && range.start == at
        {
            highlighted.next();
            let text = sql.get(range.clone()).unwrap_or_default();
            out.push(match kind {
                Kind::Keyword => Lexeme::Word(text, true),
                Kind::Comment if text.starts_with("--") => Lexeme::LineComment(text),
                Kind::String | Kind::Name | Kind::Number | Kind::Comment => Lexeme::Kept(text),
            });
            at = range.end;
            continue;
        }
        let Some(c) = sql.get(at..).and_then(|rest| rest.chars().next()) else {
            break;
        };
        if bytes.get(at).copied().is_some_and(is_word_byte) {
            let start = at;
            while bytes.get(at).copied().is_some_and(is_word_byte) {
                at += 1;
            }
            out.push(Lexeme::Word(sql.get(start..at).unwrap_or_default(), false));
            continue;
        }
        if !c.is_whitespace() {
            out.push(Lexeme::Punct(c));
        }
        at += c.len_utf8();
    }
    out
}

/// Keywords that start a clause on a line of their own (R-SQL-22).
const CLAUSES: [&str; 17] = [
    "SELECT",
    "FROM",
    "WHERE",
    "GROUP",
    "HAVING",
    "ORDER",
    "UNION",
    "EXCEPT",
    "INTERSECT",
    "FETCH",
    "OFFSET",
    "LIMIT",
    "VALUES",
    "SET",
    "INSERT",
    "UPDATE",
    "DELETE",
];

/// Words that start a join, on a line of their own among the FROM clause's contents.
const JOINS: [&str; 6] = ["JOIN", "INNER", "LEFT", "RIGHT", "FULL", "CROSS"];

/// Keywords after which `(` keeps a space: `IN (1, 2)`, not `IN(1, 2)`.
const SPACED_BEFORE_PAREN: [&str; 10] = [
    "IN", "VALUES", "AS", "ON", "AND", "OR", "NOT", "EXISTS", "THEN", "ELSE",
];

const INDENT: &str = "    ";

/// Clause words that a second word completes (`GROUP BY`, `DELETE FROM`): the contents go under
/// the second word, not the first. `UNION`'s query starts a clause of its own.
const TWO_WORD_FIRSTS: [&str; 5] = ["GROUP", "ORDER", "UNION", "INSERT", "DELETE"];

/// Whether `word` is the second word of a two-word clause (`GROUP BY`, `ORDER BY`,
/// `DELETE FROM`, `INSERT INTO`): it stays on the first word's line, the contents go under.
fn second_word(previous: Option<&Lexeme<'_>>, word: &str) -> bool {
    let Some(Lexeme::Word(first, true)) = previous else {
        return false;
    };
    matches!(
        (first.to_ascii_uppercase().as_str(), word),
        ("GROUP" | "ORDER", "BY") | ("DELETE", "FROM") | ("INSERT", "INTO")
    )
}

/// `sql` formatted (R-SQL-22): keywords in capitals, each main clause on its own line with its
/// contents indented under it, AND and OR and joins on lines of their own, a nested SELECT
/// indented inside its parentheses. Strings, names, numbers and comments stay as written.
///
/// Text that holds a CL command (R-SQL-30) is left as it is: CL is not SQL.
pub fn format(sql: &str) -> String {
    let has_cl = statements(sql)
        .into_iter()
        .any(|r| sql.get(r).and_then(cl_command).is_some());
    if has_cl {
        return sql.trim().to_owned();
    }
    let lexemes = lexemes(sql);
    let mut f = Formatter::default();
    for (i, lexeme) in lexemes.iter().enumerate() {
        f.lexeme(lexeme, lexemes.get(i + 1));
    }
    f.out
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_owned()
}

/// The formatter's state as it goes through the lexemes.
#[derive(Default)]
struct Formatter<'a> {
    out: String,
    /// The level of the clause being written.
    level: usize,
    /// For each open parenthesis: the level to go back to if it holds a query.
    parens: Vec<Option<usize>>,
    previous: Option<Lexeme<'a>>,
    line_start: bool,
}

impl<'a> Formatter<'a> {
    fn lexeme(&mut self, lexeme: &Lexeme<'a>, next: Option<&Lexeme<'a>>) {
        let upper = match lexeme {
            Lexeme::Word(w, true) => w.to_ascii_uppercase(),
            _ => String::new(),
        };
        let inline = self.parens.iter().filter(|p| p.is_none()).count() > 0;
        let second = second_word(self.previous.as_ref(), &upper);
        let starts_clause = !inline && CLAUSES.contains(&upper.as_str()) && !second;
        self.break_before(&upper, inline, starts_clause);
        match lexeme {
            Lexeme::Word(_, true) => self.word(&upper),
            Lexeme::Word(w, false) | Lexeme::Kept(w) | Lexeme::LineComment(w) => self.word(w),
            Lexeme::Punct(c) => {
                if self.punct(*c, inline, next) {
                    self.previous = Some(lexeme.clone());
                    return;
                }
            }
        }
        self.line_start = false;
        // A clause's contents go on the lines under it, as does what follows `--`.
        let opens_contents =
            (starts_clause && !TWO_WORD_FIRSTS.contains(&upper.as_str())) || second;
        if opens_contents || matches!(lexeme, Lexeme::LineComment(_)) {
            self.newline(self.level + 1);
        }
        self.previous = Some(lexeme.clone());
    }

    /// A line break before a clause, and before AND, OR and a join among a clause's contents.
    fn break_before(&mut self, upper: &str, inline: bool, starts_clause: bool) {
        if self.line_start || inline {
            return;
        }
        let after_join = matches!(&self.previous, Some(Lexeme::Word(w, true)) if JOINS.contains(&w.to_ascii_uppercase().as_str()));
        if starts_clause {
            self.newline(self.level);
        } else if matches!(upper, "AND" | "OR") || (JOINS.contains(&upper) && !after_join) {
            self.newline(self.level + 1);
        }
    }

    /// A word or kept text, with a space before it unless a line or `(` or `.` comes first.
    fn word(&mut self, text: &str) {
        let glued = matches!(self.previous, Some(Lexeme::Punct('(' | '.')));
        if !self.line_start && !glued {
            self.out.push(' ');
        }
        self.out.push_str(text);
    }

    /// Punctuation; true when it already ended the line.
    fn punct(&mut self, c: char, inline: bool, next: Option<&Lexeme<'_>>) -> bool {
        match c {
            '(' => {
                let spaced = match &self.previous {
                    Some(Lexeme::Word(w, true)) => {
                        SPACED_BEFORE_PAREN.contains(&w.to_ascii_uppercase().as_str())
                    }
                    Some(Lexeme::Word(_, false) | Lexeme::Punct(')')) => false,
                    _ => true,
                };
                if spaced && !self.line_start {
                    self.out.push(' ');
                }
                self.out.push('(');
                let query =
                    matches!(next, Some(Lexeme::Word(w, true)) if w.eq_ignore_ascii_case("SELECT"));
                if query {
                    self.parens.push(Some(self.level));
                    self.level += 2;
                    self.newline(self.level);
                    return true;
                }
                self.parens.push(None);
            }
            ')' => {
                if let Some(Some(back)) = self.parens.pop() {
                    self.level = back;
                    self.newline(self.level + 1);
                }
                self.out.push(')');
            }
            ',' => {
                self.out.push(',');
                if !inline {
                    self.newline(self.level + 1);
                    return true;
                }
            }
            ';' => {
                self.out.push_str(";\n");
                self.level = 0;
                self.parens.clear();
                self.line_start = true;
                return true;
            }
            other => {
                let glued = matches!(self.previous, Some(Lexeme::Punct('(' | '.')));
                if !self.line_start && !glued && other != '.' {
                    self.out.push(' ');
                }
                self.out.push(other);
            }
        }
        false
    }

    /// Ends the line and indents the next to `indent`.
    fn newline(&mut self, indent: usize) {
        let trimmed = self.out.trim_end_matches(' ').len();
        self.out.truncate(trimmed);
        if !self.out.is_empty() && !self.out.ends_with('\n') {
            self.out.push('\n');
        }
        self.out.push_str(&INDENT.repeat(indent));
        self.line_start = true;
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

    fn kinds(sql: &str) -> Vec<(&str, Kind)> {
        highlight(sql)
            .into_iter()
            .filter_map(|(r, k)| Some((sql.get(r)?, k)))
            .collect()
    }

    #[test]
    fn keywords_strings_names_numbers_and_comments_are_found_r_sql_19() {
        assert_eq!(
            kinds("select \"Navn\", 'æøå' from t -- note\nwhere x = 12.5 /* c */"),
            [
                ("select", Kind::Keyword),
                ("\"Navn\"", Kind::Name),
                ("'æøå'", Kind::String),
                ("from", Kind::Keyword),
                ("-- note", Kind::Comment),
                ("where", Kind::Keyword),
                ("12.5", Kind::Number),
                ("/* c */", Kind::Comment),
            ]
        );
    }

    #[test]
    fn words_inside_strings_and_comments_are_not_keywords_and_names_are_not_numbers_r_sql_19() {
        assert_eq!(
            kinds("'SELECT' -- FROM"),
            [("'SELECT'", Kind::String), ("-- FROM", Kind::Comment)]
        );
        assert_eq!(
            kinds("T2 COL1 order_by"),
            [],
            "words with digits are names; ORDER_BY is one word"
        );
        assert_eq!(
            kinds("'open"),
            [("'open", Kind::String)],
            "unclosed runs to the end"
        );
        assert_eq!(kinds(""), []);
    }

    #[test]
    fn the_keyword_list_is_in_capitals_r_sql_19() {
        assert!(
            KEYWORDS
                .iter()
                .all(|k| k.chars().all(|c| c.is_ascii_uppercase() || c == ' '))
        );
    }

    #[test]
    fn a_query_gets_its_clauses_on_lines_of_their_own_r_sql_22() {
        assert_eq!(
            format(
                "select name, count(*) from orders o left join customers c on c.id = o.customer where status = 'open' and total > 100 order by name"
            ),
            "SELECT\n    name,\n    count(*)\nFROM\n    orders o\n    LEFT JOIN customers c ON c.id = o.customer\nWHERE\n    status = 'open'\n    AND total > 100\nORDER BY\n    name"
        );
    }

    #[test]
    fn a_nested_select_is_indented_inside_its_parentheses_r_sql_22() {
        assert_eq!(
            format("SELECT A FROM T WHERE X IN (SELECT B FROM U) AND Y IN (1, 2)"),
            "SELECT\n    A\nFROM\n    T\nWHERE\n    X IN (\n        SELECT\n            B\n        FROM\n            U\n    )\n    AND Y IN (1, 2)"
        );
    }

    #[test]
    fn strings_names_and_comments_are_kept_exactly_r_sql_22() {
        let formatted = format(
            "select 'select x from y', \"Æøå Navn\" -- why, and where\nfrom t /* keep  this */",
        );
        assert!(formatted.contains("'select x from y'"), "{formatted}");
        assert!(formatted.contains("\"Æøå Navn\""), "{formatted}");
        assert!(formatted.contains("-- why, and where"), "{formatted}");
        assert!(formatted.contains("/* keep  this */"), "{formatted}");
        assert!(formatted.starts_with("SELECT\n"), "{formatted}");
    }

    #[test]
    fn statements_are_formatted_one_after_another_and_formatting_twice_changes_nothing_r_sql_22() {
        let once = format("update t set a = 1 where b = 2; delete from t where c = 'æøå'");
        assert_eq!(
            once,
            "UPDATE\n    t\nSET\n    a = 1\nWHERE\n    b = 2;\nDELETE FROM\n    t\nWHERE\n    c = 'æøå'"
        );
        assert_eq!(format(&once), once);
        assert_eq!(format(""), "");
    }

    #[test]
    fn a_cl_statement_is_found_after_comments_and_called_through_qcmdexc_r_sql_30() {
        assert_eq!(cl_command("CL: ADDLIBLE MYLIB"), Some("ADDLIBLE MYLIB"));
        assert_eq!(
            cl_command("-- add it\n  cl:SNDMSG MSG('æøå''s') TOUSR(QSYSOPR)"),
            Some("SNDMSG MSG('æøå''s') TOUSR(QSYSOPR)")
        );
        assert_eq!(cl_command("SELECT CL FROM T"), None);
        assert_eq!(cl_command("CLOSE C"), None);
        assert_eq!(cl_command(""), None);
        assert_eq!(cl_command("CL:"), Some(""));
        assert_eq!(
            cl_call("SNDMSG MSG('æøå''s') TOUSR(QSYSOPR)"),
            "CALL QSYS2.QCMDEXC('SNDMSG MSG(''æøå''''s'') TOUSR(QSYSOPR)')"
        );
    }

    #[test]
    fn a_cl_command_that_deletes_clears_or_ends_is_confirmed_r_sql_30() {
        for command in [
            "CL: DLTLIB X",
            "CL: qsys/clrpfm F",
            "CL: PWRDWNSYS *IMMED",
            "cl: ENDJOB(123/A/B)",
        ] {
            assert_eq!(risk(command), Some(Risk::Command), "{command}");
        }
        for command in ["CL: ADDLIBLE X", "CL: WRKACTJOB", "CL: DSPLIB DLTLIB"] {
            assert_eq!(risk(command), None, "{command}");
        }
    }

    #[test]
    fn cl_and_its_command_are_coloured_and_left_alone_by_format_r_sql_30() {
        let sql = "CL: ADDLIBLE MYLIB;\nselect a from t";
        let pieces: Vec<(&str, Kind)> = highlight(sql)
            .into_iter()
            .filter_map(|(r, k)| sql.get(r).map(|t| (t, k)))
            .collect();
        assert_eq!(
            &pieces[..2],
            [("CL:", Kind::Keyword), ("ADDLIBLE", Kind::Keyword)]
        );
        assert!(!pieces.iter().any(|(t, _)| *t == "MYLIB"));
        assert_eq!(format("CL: addlible   mylib"), "CL: addlible   mylib");
        assert_eq!(format(sql), sql, "text holding a command is left alone");
    }

    #[test]
    fn set_and_cl_may_change_the_job_r_sql_32() {
        for statement in ["SET SCHEMA X", "-- c\nset path = X", "CL: ADDLIBLE X"] {
            assert!(may_change_job(statement), "{statement}");
        }
        for statement in ["SELECT SET FROM T", "VALUES 'SET'", ""] {
            assert!(!may_change_job(statement), "{statement}");
        }
    }

    #[test]
    fn a_library_name_is_a_system_name_r_sql_32() {
        assert_eq!(system_name(" msgsudta "), Some("MSGSUDTA".to_owned()));
        assert_eq!(system_name("$LIB#@_1"), Some("$LIB#@_1".to_owned()));
        for name in [
            "",
            "1LIB",
            "_LIB",
            "ELEVENCHARS",
            "MY LIB",
            "ÆØÅ",
            "A'B",
            "A)",
        ] {
            assert_eq!(system_name(name), None, "{name}");
        }
    }
}
