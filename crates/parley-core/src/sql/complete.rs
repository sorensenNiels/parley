//! Completion in the SQL editor (R-SQL-36): the name being typed at the cursor, the tables a
//! statement names with their aliases, and the SQL keywords that fit.

use super::text::KEYWORDS;

/// The name being typed: the text from `start` to `end` (the cursor), and the name before a `.`
/// in front of it, if any. Unquoted names are in capitals.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Typed {
    pub start: usize,
    pub end: usize,
    pub prefix: String,
    pub qualifier: Option<String>,
}

fn is_name_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | '#' | '$' | '@')
}

/// What is typed at byte `cursor` of `text`: a name or a part of one, or nothing after a `.`;
/// `None` in a string or a comment, after a number, or where no name is being typed.
pub fn typed_at(text: &str, cursor: usize) -> Option<Typed> {
    let before = text.get(..cursor)?;
    let start = before
        .char_indices()
        .rev()
        .take_while(|&(_, c)| is_name_char(c))
        .last()
        .map_or(cursor, |(i, _)| i);
    let prefix = before.get(start..)?;
    let qualifier = before
        .get(..start)
        .and_then(|b| b.strip_suffix('.'))
        .and_then(|b| {
            let from = b
                .char_indices()
                .rev()
                .take_while(|&(_, c)| is_name_char(c))
                .last()?
                .0;
            b.get(from..)
        })
        .map(str::to_uppercase);
    let line = before.rsplit('\n').next().unwrap_or_default();
    let in_text = line.matches('\'').count() % 2 == 1 || line.contains("--");
    let number = prefix.starts_with(|c: char| c.is_ascii_digit());
    if in_text || (number && qualifier.is_none()) || (prefix.is_empty() && qualifier.is_none()) {
        return None;
    }
    Some(Typed {
        start,
        end: cursor,
        prefix: prefix.to_uppercase(),
        qualifier,
    })
}

/// The name at byte `at` of `text` and the name before a `.` in front of it, in capitals, for
/// hovering (R-SQL-37); `None` off a name, on a number, or in a string or a comment.
pub fn name_at(text: &str, at: usize) -> Option<(Option<String>, String)> {
    if !text.is_char_boundary(at) {
        return None;
    }
    let end = text
        .get(at..)?
        .char_indices()
        .find(|&(_, c)| !is_name_char(c))
        .map_or(text.len(), |(i, _)| at + i);
    let typed = typed_at(text, end)?;
    (!typed.prefix.is_empty() && !typed.prefix.starts_with(|c: char| c.is_ascii_digit()))
        .then_some((typed.qualifier, typed.prefix))
}

/// A table a statement names after FROM, JOIN, UPDATE or INTO, with its library and alias.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableRef {
    pub library: Option<String>,
    pub table: String,
    pub alias: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    Word(String),
    /// `.`, or `/` in system naming.
    Dot,
    Comma,
    Open,
    Other,
}

/// The statement's words, dots and commas; strings and comments are left out, and unquoted
/// words are in capitals.
fn tokens(statement: &str) -> Vec<Token> {
    let mut out = Vec::new();
    let mut chars = statement.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\'' => {
                for d in chars.by_ref() {
                    if d == '\'' {
                        break;
                    }
                }
            }
            '"' => {
                let name: String = chars.by_ref().take_while(|&d| d != '"').collect();
                out.push(Token::Word(name));
            }
            '-' if chars.peek() == Some(&'-') => {
                for d in chars.by_ref() {
                    if d == '\n' {
                        break;
                    }
                }
            }
            '.' | '/' => out.push(Token::Dot),
            ',' => out.push(Token::Comma),
            '(' => out.push(Token::Open),
            c if is_name_char(c) => {
                let mut word = c.to_uppercase().to_string();
                while let Some(&d) = chars.peek().filter(|&&d| is_name_char(d)) {
                    word.extend(d.to_uppercase());
                    chars.next();
                }
                out.push(Token::Word(word));
            }
            c if c.is_whitespace() => {}
            _ => out.push(Token::Other),
        }
    }
    out
}

/// Words that end a table reference rather than name its alias.
const CLAUSES: [&str; 26] = [
    "WHERE",
    "JOIN",
    "INNER",
    "LEFT",
    "RIGHT",
    "FULL",
    "CROSS",
    "ON",
    "GROUP",
    "ORDER",
    "FETCH",
    "SET",
    "VALUES",
    "UNION",
    "EXCEPT",
    "INTERSECT",
    "LIMIT",
    "OFFSET",
    "WITH",
    "FOR",
    "HAVING",
    "USING",
    "SELECT",
    "EXCEPTION",
    "OUTER",
    "NATURAL",
];

/// The tables `statement` names, in order.
pub fn tables_in(statement: &str) -> Vec<TableRef> {
    let tokens = tokens(statement);
    let word = |i: usize| match tokens.get(i) {
        Some(Token::Word(w)) => Some(w.as_str()),
        _ => None,
    };
    let mut found = Vec::new();
    for (i, token) in tokens.iter().enumerate() {
        let Token::Word(w) = token else { continue };
        if !matches!(w.as_str(), "FROM" | "JOIN" | "UPDATE" | "INTO") {
            continue;
        }
        let mut at = i + 1;
        // `TABLE(…)` and `(SELECT …)` are not tables of the catalog.
        while let Some(first) = word(at) {
            if tokens.get(at + 1) == Some(&Token::Open) || CLAUSES.contains(&first) {
                break;
            }
            let (library, table) = if tokens.get(at + 1) == Some(&Token::Dot) {
                let Some(table) = word(at + 2) else { break };
                at += 3;
                (Some(first.to_owned()), table.to_owned())
            } else {
                at += 1;
                (None, first.to_owned())
            };
            if word(at) == Some("AS") {
                at += 1;
            }
            let alias = word(at).filter(|a| !CLAUSES.contains(a)).map(str::to_owned);
            if alias.is_some() {
                at += 1;
            }
            found.push(TableRef {
                library,
                table,
                alias,
            });
            if w != "FROM" || tokens.get(at) != Some(&Token::Comma) {
                break;
            }
            at += 1;
        }
    }
    found
}

/// The SQL keywords starting with `prefix` (in capitals), in order.
pub fn keywords(prefix: &str) -> impl Iterator<Item = &'static str> {
    KEYWORDS
        .into_iter()
        .filter(move |k| k.starts_with(prefix) && *k != prefix)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typed(text: &str) -> Option<Typed> {
        typed_at(text, text.len())
    }

    #[test]
    fn the_name_being_typed_and_its_qualifier_are_found_r_sql_36() {
        let t = typed("SELECT * FROM qgpl.kun").expect("typed");
        assert_eq!(t.prefix, "KUN");
        assert_eq!(t.qualifier.as_deref(), Some("QGPL"));
        assert_eq!(t.start, "SELECT * FROM qgpl.".len());
        let t = typed("SELECT k.").expect("after a dot");
        assert_eq!((t.prefix.as_str(), t.qualifier.as_deref()), ("", Some("K")));
        assert_eq!(typed("SELECT ætt").expect("æøå").prefix, "ÆTT");
        assert_eq!(typed("SEL").expect("a keyword").qualifier, None);
    }

    #[test]
    fn nothing_is_offered_in_text_comments_numbers_or_blanks_r_sql_36() {
        for text in [
            "SELECT 'abc",
            "-- a comment",
            "VALUES 12",
            "SELECT ",
            "",
            "SELECT * FROM T ",
        ] {
            assert_eq!(typed(text), None, "{text}");
        }
        assert_eq!(typed_at("abc", 99), None, "past the end");
        assert_eq!(typed_at("æøå", 1), None, "inside a character");
    }

    #[test]
    fn tables_and_aliases_are_read_from_the_statement_r_sql_36() {
        let refs = tables_in(
            "SELECT * FROM qgpl.kunder k JOIN ordre AS o ON k.id = o.kunde, \"Mixed\" WHERE 'FROM x' = 1",
        );
        assert_eq!(
            refs,
            [
                TableRef {
                    library: Some("QGPL".to_owned()),
                    table: "KUNDER".to_owned(),
                    alias: Some("K".to_owned()),
                },
                TableRef {
                    library: None,
                    table: "ORDRE".to_owned(),
                    alias: Some("O".to_owned()),
                },
            ]
        );
        let refs = tables_in("SELECT * FROM a, lib/b WHERE 1 = 1");
        assert_eq!(refs.len(), 2);
        assert_eq!(refs[1].library.as_deref(), Some("LIB"));
        assert_eq!(refs[1].alias, None);
        assert_eq!(
            tables_in("SELECT * FROM TABLE(QSYS2.JOBLOG_INFO('*')) x"),
            []
        );
        assert_eq!(tables_in("UPDATE æøå SET a = 1")[0].table, "ÆØÅ");
        assert_eq!(tables_in("SELECT * FROM"), []);
    }

    #[test]
    fn the_name_under_the_pointer_is_found_with_its_library_r_sql_37() {
        let text = "SELECT * FROM qgpl.kunder k -- æøå";
        assert_eq!(
            name_at(text, 20),
            Some((Some("QGPL".to_owned()), "KUNDER".to_owned()))
        );
        assert_eq!(name_at(text, 14), Some((None, "QGPL".to_owned())));
        assert_eq!(name_at(text, 8), None, "on a blank");
        assert_eq!(name_at(text, 31), None, "in a comment");
        assert_eq!(name_at(text, 999), None);
        assert_eq!(name_at("VALUES 12", 8), None);
    }

    #[test]
    fn keywords_are_offered_by_prefix_but_not_the_word_typed_r_sql_36() {
        assert!(keywords("SEL").any(|k| k == "SELECT"));
        assert_eq!(keywords("SELECT").count(), 0);
    }
}
