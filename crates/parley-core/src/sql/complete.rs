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

pub(super) fn is_name_char(c: char) -> bool {
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
    Close,
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
            ')' => out.push(Token::Close),
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

/// A common table expression of a `WITH` (R-SQL-76): its name and its columns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cte {
    pub name: String,
    pub columns: Vec<String>,
}

/// The common table expressions `statement` starts with: each name, and its columns as listed
/// (`x (a, b) AS (…)`), else the names of its select list (a column, or what `AS` or a trailing
/// name calls it; an expression without one is left out). Names in capitals unless quoted.
pub fn ctes(statement: &str) -> Vec<Cte> {
    let tokens = tokens(statement);
    let word = |i: usize| match tokens.get(i) {
        Some(Token::Word(w)) => Some(w.as_str()),
        _ => None,
    };
    let mut found = Vec::new();
    if word(0) != Some("WITH") {
        return found;
    }
    let mut at = 1 + usize::from(word(1) == Some("RECURSIVE"));
    while let Some(name) = word(at) {
        at += 1;
        let mut columns = Vec::new();
        if tokens.get(at) == Some(&Token::Open) {
            at += 1;
            while let Some(token) = tokens.get(at) {
                at += 1;
                match token {
                    Token::Word(column) => columns.push(column.clone()),
                    Token::Close => break,
                    Token::Dot | Token::Comma | Token::Open | Token::Other => {}
                }
            }
        }
        if word(at) != Some("AS") || tokens.get(at + 1) != Some(&Token::Open) {
            break;
        }
        let inner = at + 2;
        let Some(close) = closing(&tokens, inner) else {
            break;
        };
        if columns.is_empty() {
            columns = select_names(tokens.get(inner..close).unwrap_or_default());
        }
        found.push(Cte {
            name: name.to_owned(),
            columns,
        });
        at = close + 1;
        if tokens.get(at) != Some(&Token::Comma) {
            break;
        }
        at += 1;
    }
    found
}

/// The index of the `)` that closes the bracket opened just before `from`.
fn closing(tokens: &[Token], from: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (i, token) in tokens.iter().enumerate().skip(from) {
        match token {
            Token::Open => depth += 1,
            Token::Close if depth == 0 => return Some(i),
            Token::Close => depth -= 1,
            Token::Word(_) | Token::Dot | Token::Comma | Token::Other => {}
        }
    }
    None
}

/// The names a `SELECT …`'s list gives its columns, up to its `FROM`.
fn select_names(tokens: &[Token]) -> Vec<String> {
    let mut rest = match tokens.first() {
        Some(Token::Word(w)) if w == "SELECT" => tokens.get(1..).unwrap_or_default(),
        _ => return Vec::new(),
    };
    if let Some(Token::Word(w)) = rest.first()
        && (w == "DISTINCT" || w == "ALL")
    {
        rest = rest.get(1..).unwrap_or_default();
    }
    let mut names = Vec::new();
    let mut item: Vec<&Token> = Vec::new();
    let mut depth = 0usize;
    for token in rest {
        match token {
            Token::Open => depth += 1,
            Token::Close => depth = depth.saturating_sub(1),
            Token::Comma if depth == 0 => {
                names.extend(item_name(&item));
                item.clear();
                continue;
            }
            Token::Word(w) if depth == 0 && w == "FROM" => break,
            Token::Word(_) | Token::Dot | Token::Comma | Token::Other => {}
        }
        item.push(token);
    }
    names.extend(item_name(&item));
    names
}

/// One select-list item's name: after `AS`, a trailing name after a column or a `)`, or the
/// column itself (`COL`, `T.COL`). A number is no name.
fn item_name(item: &[&Token]) -> Option<String> {
    let last = match item.last()? {
        Token::Word(w) if !w.starts_with(|c: char| c.is_ascii_digit()) => w.clone(),
        Token::Word(_) | Token::Dot | Token::Comma | Token::Open | Token::Close | Token::Other => {
            return None;
        }
    };
    let named = match item.len() {
        1 => true,
        n => match item.get(n - 2)? {
            Token::Word(_) | Token::Close => true,
            Token::Dot => n == 3,
            Token::Comma | Token::Open | Token::Other => false,
        },
    };
    named.then_some(last)
}

/// A routine being called at the cursor (R-SQL-65): its name, the name before a `.` in front
/// of it, both in capitals unless quoted, and which argument the cursor is at, from 0.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallAt {
    pub qualifier: Option<String>,
    pub name: String,
    pub argument: usize,
}

/// An open bracket while scanning: the routine before it, if a name is, and the commas since.
struct Open {
    routine: Option<(Option<String>, String)>,
    commas: usize,
}

/// The routine whose brackets the cursor at byte `cursor` of `statement` is in, if a name is
/// before them that is not a keyword; strings and comments are skipped.
pub fn call_at(statement: &str, cursor: usize) -> Option<CallAt> {
    let before = statement.get(..cursor)?;
    let mut stack: Vec<Open> = Vec::new();
    let mut chars = before.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        match c {
            '\'' | '"' => {
                if !chars.by_ref().any(|(_, d)| d == c) {
                    return None;
                }
            }
            '-' if chars.peek().is_some_and(|&(_, d)| d == '-') => {
                if !chars.by_ref().any(|(_, d)| d == '\n') {
                    return None;
                }
            }
            '/' if chars.peek().is_some_and(|&(_, d)| d == '*') => {
                chars.next();
                let mut star = false;
                let closed = chars.by_ref().any(|(_, d)| {
                    let end = star && d == '/';
                    star = d == '*';
                    end
                });
                if !closed {
                    return None;
                }
            }
            '(' => stack.push(Open {
                routine: routine_before(before.get(..i).unwrap_or_default()),
                commas: 0,
            }),
            ')' => {
                stack.pop();
            }
            ',' => {
                if let Some(open) = stack.last_mut() {
                    open.commas += 1;
                }
            }
            _ => {}
        }
    }
    let open = stack.pop()?;
    let (qualifier, name) = open.routine?;
    Some(CallAt {
        qualifier,
        name,
        argument: open.commas,
    })
}

/// The name just before a `(` at the end of `text` (blanks between allowed) and its qualifier;
/// `None` for a keyword or no name.
fn routine_before(text: &str) -> Option<(Option<String>, String)> {
    let text = text.trim_end();
    let (name, rest) = name_back(text)?;
    if KEYWORDS.contains(&name.as_str()) {
        return None;
    }
    let qualifier = rest
        .strip_suffix('.')
        .or_else(|| rest.strip_suffix('/'))
        .and_then(|r| name_back(r).map(|(q, _)| q));
    Some((qualifier, name))
}

/// The name at the end of `text` (quoted: as written; else in capitals) and the text before it.
fn name_back(text: &str) -> Option<(String, &str)> {
    if let Some(inner) = text.strip_suffix('"') {
        let open = inner.rfind('"')?;
        return Some((inner.get(open + 1..)?.to_owned(), inner.get(..open)?));
    }
    let start = text
        .char_indices()
        .rev()
        .take_while(|&(_, c)| is_name_char(c))
        .last()?
        .0;
    let name = text.get(start..)?;
    if name.starts_with(|c: char| c.is_ascii_digit()) {
        return None;
    }
    Some((name.to_uppercase(), text.get(..start)?))
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

    #[test]
    fn the_routine_and_the_argument_at_the_cursor_are_found_r_sql_65() {
        let call = |text: &str| call_at(text, text.len());
        assert_eq!(
            call("CALL qgpl.opdater_kunde(1, 'a,b', "),
            Some(CallAt {
                qualifier: Some("QGPL".to_owned()),
                name: "OPDATER_KUNDE".to_owned(),
                argument: 2,
            })
        );
        let nested = call("SELECT beløb(x, rund(y, 2), ").expect("a call");
        assert_eq!((nested.name.as_str(), nested.argument), ("BELØB", 2));
        let inner = call("SELECT beløb(x, rund(y, ").expect("a call");
        assert_eq!((inner.name.as_str(), inner.argument), ("RUND", 1));
        assert_eq!(call("SELECT f(x) "), None, "closed");
        assert_eq!(call("SELECT * FROM t WHERE a IN ("), None, "a keyword");
        assert_eq!(call("SELECT 'f(x"), None, "in a string");
        assert_eq!(call("SELECT (1 + "), None, "no name");
        let quoted = call("CALL \"Min lib\".\"Gør det\"(").expect("a call");
        assert_eq!(quoted.qualifier.as_deref(), Some("Min lib"));
        assert_eq!(quoted.name, "Gør det");
        assert_eq!(call(""), None);
    }

    #[test]
    fn a_withs_names_and_columns_are_read_from_the_statement_r_sql_76() {
        let statement = "WITH totaler (kunde, sum) AS (SELECT K, SUM(B) FROM T GROUP BY K), \
             store AS (SELECT DISTINCT t.navn, COUNT(*) AS antal, beløb b, 1 + 2, \"Æble\" FROM X) \
             SELECT * FROM totaler JOIN store s ON s.navn = totaler.kunde";
        assert_eq!(
            ctes(statement),
            vec![
                Cte {
                    name: "TOTALER".to_owned(),
                    columns: vec!["KUNDE".to_owned(), "SUM".to_owned()],
                },
                Cte {
                    name: "STORE".to_owned(),
                    columns: vec![
                        "NAVN".to_owned(),
                        "ANTAL".to_owned(),
                        "B".to_owned(),
                        "Æble".to_owned()
                    ],
                },
            ]
        );
    }

    #[test]
    fn without_a_with_or_when_it_is_cut_short_there_are_none_or_some_r_sql_76() {
        assert_eq!(ctes("SELECT * FROM T"), Vec::new());
        assert_eq!(ctes(""), Vec::new());
        assert_eq!(
            ctes("WITH x AS (SELECT a FROM"),
            Vec::new(),
            "not closed yet"
        );
        let recursive = ctes("WITH RECURSIVE r (n) AS (VALUES 1) SELECT * FROM r");
        assert_eq!(
            recursive.first().map(|c| c.columns.clone()),
            Some(vec!["N".to_owned()])
        );
    }
}
