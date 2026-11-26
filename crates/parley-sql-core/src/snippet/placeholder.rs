//! Placeholders in a snippet's SQL (R-SQL-26, R-SQL-110): `${name}`, `${name:default}`, and
//! with a type, `${name as text}` or `${name as number:7}`. An untyped value goes in as typed;
//! a typed one is checked and written as SQL wants it.

/// A placeholder in a snippet's SQL: its name, its type and the default written with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Placeholder {
    pub name: String,
    pub kind: ValueType,
    pub default: Option<String>,
}

/// What a placeholder's value is (R-SQL-110).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValueType {
    /// No type: the value goes in as typed (R-SQL-26).
    Any,
    /// Quoted, a `'` in it doubled.
    Text,
    /// A sign, digits and one decimal point or comma, written with a point.
    Number,
    /// `YYYY-MM-DD`, a real date, written quoted.
    Date,
    /// An SQL name, in capitals, or a delimited `"name"` as typed; written without quotes.
    Name,
    /// Values separated by commas, each written as text, for an `IN (…)` (R-SQL-114).
    List,
    /// A library's name, written as [`Self::Name`]; the form offers the known ones (R-SQL-115).
    Library,
    /// A table's name, written as [`Self::Name`]; the form offers a library's (R-SQL-115).
    Table,
    /// A type Parley does not know, as written: the value goes in as typed.
    Unknown(String),
}

/// Why a value does not fit its placeholder's type (R-SQL-110).
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum BadValue {
    #[error("not a number")]
    Number,
    #[error("not a date written YYYY-MM-DD")]
    Date,
    #[error("not an SQL name: A–Z, 0–9, _, $, # and @, or \"quoted\"")]
    Name,
    #[error("no values: write them separated by commas")]
    List,
}

impl ValueType {
    fn parse(word: &str) -> Self {
        match word.to_ascii_lowercase().as_str() {
            "text" => Self::Text,
            "number" => Self::Number,
            "date" => Self::Date,
            "name" => Self::Name,
            "list" => Self::List,
            "library" => Self::Library,
            "table" => Self::Table,
            _ => Self::Unknown(word.to_owned()),
        }
    }

    /// How it is written after `as`; empty for no type.
    pub fn word(&self) -> &str {
        match self {
            Self::Any => "",
            Self::Text => "text",
            Self::Number => "number",
            Self::Date => "date",
            Self::Name => "name",
            Self::List => "list",
            Self::Library => "library",
            Self::Table => "table",
            Self::Unknown(word) => word,
        }
    }

    /// What the value field's hint says.
    pub const fn hint(&self) -> &'static str {
        match self {
            Self::Any | Self::Unknown(_) => "Goes in as typed",
            Self::Text => "Text; Parley writes the quotes",
            Self::Number => "A number",
            Self::Date => "YYYY-MM-DD",
            Self::Name => "A name, such as QGPL",
            Self::List => "Values separated by commas",
            Self::Library => "A library, such as QGPL",
            Self::Table => "A table, such as SYSTABLES",
        }
    }

    /// `value` as it goes into the SQL.
    pub fn render(&self, value: &str) -> Result<String, BadValue> {
        match self {
            Self::Any | Self::Unknown(_) => Ok(value.to_owned()),
            Self::Text => Ok(format!("'{}'", value.replace('\'', "''"))),
            Self::Number => number(value.trim()).ok_or(BadValue::Number),
            Self::Date => date(value.trim())
                .map(|d| format!("'{d}'"))
                .ok_or(BadValue::Date),
            Self::Name | Self::Library | Self::Table => name(value.trim()).ok_or(BadValue::Name),
            Self::List => list(value).ok_or(BadValue::List),
        }
    }

    /// Whether the value is written in quotes, so the SQL must not quote it again.
    const fn quotes(&self) -> bool {
        matches!(self, Self::Text | Self::Date | Self::List)
    }
}

/// A number: an optional sign, digits, and one decimal point or comma; written with a point.
fn number(value: &str) -> Option<String> {
    let digits = value.strip_prefix(['-', '+']).unwrap_or(value);
    let mut points = 0;
    let mut any_digit = false;
    for c in digits.chars() {
        match c {
            '0'..='9' => any_digit = true,
            '.' | ',' => points += 1,
            _ => return None,
        }
    }
    (any_digit && points <= 1).then(|| value.replace(',', "."))
}

/// Values separated by commas, each quoted with its `'` doubled, separated by `, `; `None`
/// without one (R-SQL-114).
fn list(value: &str) -> Option<String> {
    let items: Vec<String> = value
        .split(',')
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(|v| format!("'{}'", v.replace('\'', "''")))
        .collect();
    (!items.is_empty()).then(|| items.join(", "))
}

/// A real date written `YYYY-MM-DD`.
fn date(value: &str) -> Option<&str> {
    let mut parts = value.split('-');
    let (year, month, day) = (parts.next()?, parts.next()?, parts.next()?);
    let lengths_ok = year.len() == 4 && month.len() == 2 && day.len() == 2;
    if parts.next().is_some() || !lengths_ok {
        return None;
    }
    let all_digits = [year, month, day]
        .iter()
        .all(|p| p.bytes().all(|b| b.is_ascii_digit()));
    if !all_digits {
        return None;
    }
    let (year, month, day): (u32, u32, u32) =
        (year.parse().ok()?, month.parse().ok()?, day.parse().ok()?);
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return None,
    };
    (year >= 1 && (1..=days).contains(&day)).then_some(value)
}

/// An SQL name in capitals, or a delimited name as typed.
fn name(value: &str) -> Option<String> {
    if let Some(inside) = value.strip_prefix('"').and_then(|v| v.strip_suffix('"')) {
        // A `"` inside a delimited name is written twice.
        let ok = !inside.is_empty() && !inside.replace("\"\"", "").contains('"');
        return ok.then(|| value.to_owned());
    }
    let upper = value.to_ascii_uppercase();
    let first_ok = upper
        .bytes()
        .next()
        .is_some_and(|b| b.is_ascii_uppercase() || matches!(b, b'$' | b'#' | b'@'));
    let ok = first_ok
        && upper.len() <= 128
        && upper.bytes().all(|b| {
            b.is_ascii_uppercase() || b.is_ascii_digit() || matches!(b, b'_' | b'$' | b'#' | b'@')
        });
    ok.then_some(upper)
}

/// What is inside `${…}`: name, type and default.
fn read(inside: &str) -> Placeholder {
    let (head, default) = match inside.split_once(':') {
        Some((head, default)) => (head, Some(default.to_owned())),
        None => (inside, None),
    };
    let words: Vec<&str> = head.split_whitespace().collect();
    let (name, kind) = match words.as_slice() {
        [name @ .., as_word, kind] if !name.is_empty() && as_word.eq_ignore_ascii_case("as") => {
            (name.join(" "), ValueType::parse(kind))
        }
        _ => (head.trim().to_owned(), ValueType::Any),
    };
    Placeholder {
        name,
        kind,
        default,
    }
}

/// The placeholders of `sql`, each name once, in the order they first appear.
pub fn placeholders(sql: &str) -> Vec<Placeholder> {
    let mut found: Vec<Placeholder> = Vec::new();
    for (_, inside) in spans(sql) {
        let placeholder = read(inside);
        if !placeholder.name.is_empty() && !found.iter().any(|p| p.name == placeholder.name) {
            found.push(placeholder);
        }
    }
    found
}

/// `sql` with each placeholder replaced by `value(name)`, or its default when that is `None`
/// (an empty text when it has none), written as its type wants (R-SQL-110). A value that does
/// not fit its type goes in as typed: the form does not let it through. An untyped value inside
/// a `'…'` string has its `'` doubled, so the string stays whole (R-SQL-112).
pub fn fill(sql: &str, value: impl Fn(&str) -> Option<String>) -> String {
    let strings: Vec<std::ops::Range<usize>> = crate::text::highlight(&masked(sql))
        .into_iter()
        .filter(|(_, kind)| *kind == crate::text::Kind::String)
        .map(|(range, _)| range)
        .collect();
    let mut out = String::with_capacity(sql.len());
    let mut at = 0;
    for (range, inside) in spans(sql) {
        out.push_str(sql.get(at..range.start).unwrap_or_default());
        let p = read(inside);
        let raw = value(&p.name).unwrap_or_else(|| p.default.clone().unwrap_or_default());
        // A string never closed runs to the end of the text, so it may end where we do.
        let in_string = strings
            .iter()
            .any(|s| s.start < range.start && range.end <= s.end);
        let raw = match p.kind {
            ValueType::Any | ValueType::Unknown(_) if in_string => raw.replace('\'', "''"),
            ValueType::Any
            | ValueType::Unknown(_)
            | ValueType::Text
            | ValueType::Number
            | ValueType::Date
            | ValueType::Name
            | ValueType::List
            | ValueType::Library
            | ValueType::Table => raw,
        };
        out.push_str(&p.kind.render(&raw).unwrap_or(raw));
        at = range.end;
    }
    out.push_str(sql.get(at..).unwrap_or_default());
    out
}

/// The text, date and list placeholders written between quotes in `sql`, by name: Parley writes
/// their quotes, so these would be quoted twice (R-SQL-110).
pub fn quoted_twice(sql: &str) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for (range, inside) in spans(sql) {
        let p = read(inside);
        let before = range
            .start
            .checked_sub(1)
            .and_then(|i| sql.get(i..range.start));
        let after = sql.get(range.end..range.end + 1);
        if p.kind.quotes() && before == Some("'") && after == Some("'") && !names.contains(&p.name)
        {
            names.push(p.name);
        }
    }
    names
}

/// `sql` with each placeholder blanked to as many `x`s, so that a `'` in a default does not
/// start or end a string for the lexer; byte offsets stay the same.
pub(super) fn masked(sql: &str) -> String {
    let mut out = String::with_capacity(sql.len());
    let mut at = 0;
    for (range, _) in spans(sql) {
        out.push_str(sql.get(at..range.start).unwrap_or_default());
        out.push_str(&"x".repeat(range.len()));
        at = range.end;
    }
    out.push_str(sql.get(at..).unwrap_or_default());
    out
}

/// Each `${…}` in `sql`: its byte range and what is inside the braces.
pub(super) fn spans(sql: &str) -> Vec<(std::ops::Range<usize>, &str)> {
    let mut spans = Vec::new();
    let mut from = 0;
    while let Some(start) = sql
        .get(from..)
        .and_then(|rest| rest.find("${"))
        .map(|n| from + n)
    {
        let Some(end) = sql
            .get(start + 2..)
            .and_then(|rest| rest.find('}'))
            .map(|n| start + 2 + n)
        else {
            break;
        };
        spans.push((start..end + 1, sql.get(start + 2..end).unwrap_or_default()));
        from = end + 1;
    }
    spans
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_type_is_read_before_the_default_and_an_old_placeholder_is_untyped_r_sql_110() {
        let found = placeholders(
            "${user as text:QSYSOPR} ${n} ${d AS Date} ${x as foo:1} ${my value:a:b} ${as}",
        );
        let kinds: Vec<(&str, ValueType, Option<&str>)> = found
            .iter()
            .map(|p| (p.name.as_str(), p.kind.clone(), p.default.as_deref()))
            .collect();
        assert_eq!(
            kinds,
            [
                ("user", ValueType::Text, Some("QSYSOPR")),
                ("n", ValueType::Any, None),
                ("d", ValueType::Date, None),
                ("x", ValueType::Unknown("foo".to_owned()), Some("1")),
                ("my value", ValueType::Any, Some("a:b")),
                ("as", ValueType::Any, None),
            ]
        );
    }

    #[test]
    fn text_is_quoted_with_its_quotes_doubled_r_sql_110() {
        assert_eq!(
            ValueType::Text.render("O'Brien"),
            Ok("'O''Brien'".to_owned())
        );
        assert_eq!(ValueType::Text.render("æøå"), Ok("'æøå'".to_owned()));
        assert_eq!(ValueType::Text.render(""), Ok("''".to_owned()));
        assert_eq!(ValueType::Any.render("O'Brien"), Ok("O'Brien".to_owned()));
        assert_eq!(
            ValueType::Unknown("foo".to_owned()).render("x"),
            Ok("x".to_owned())
        );
    }

    #[test]
    fn a_number_takes_a_sign_and_one_point_or_comma_r_sql_110() {
        let n = |v: &str| ValueType::Number.render(v);
        assert_eq!(n("7"), Ok("7".to_owned()));
        assert_eq!(n(" -1,5 "), Ok("-1.5".to_owned()));
        assert_eq!(n("+0.25"), Ok("+0.25".to_owned()));
        for bad in ["", "-", "1.2.3", "1,000.5", "7; DROP TABLE T", "1e3", "æ"] {
            assert_eq!(n(bad), Err(BadValue::Number), "{bad}");
        }
    }

    #[test]
    fn a_date_must_exist_r_sql_110() {
        let d = |v: &str| ValueType::Date.render(v);
        assert_eq!(d("2026-11-25"), Ok("'2026-11-25'".to_owned()));
        assert_eq!(d("2024-02-29"), Ok("'2024-02-29'".to_owned()));
        for bad in [
            "",
            "2026-02-29",
            "1900-02-29",
            "2026-13-01",
            "2026-04-31",
            "2026-1-1",
            "26-11-25",
            "2026-11-25-1",
            "0000-01-01",
            "2026-1a-01",
            "æøå",
        ] {
            assert_eq!(d(bad), Err(BadValue::Date), "{bad}");
        }
    }

    #[test]
    fn a_name_is_put_in_capitals_or_kept_when_delimited_r_sql_110() {
        let n = |v: &str| ValueType::Name.render(v);
        assert_eq!(n(" qgpl "), Ok("QGPL".to_owned()));
        assert_eq!(n("$my#lib@_1"), Ok("$MY#LIB@_1".to_owned()));
        assert_eq!(n("\"my \"\"lib\"\"\""), Ok("\"my \"\"lib\"\"\"".to_owned()));
        let long = "A".repeat(128);
        assert_eq!(n(&long), Ok(long.clone()));
        for bad in [
            "",
            "1LIB",
            "_LIB",
            "MY LIB",
            "æøå",
            "LIB;",
            "\"\"",
            "\"a\"b\"",
            &"A".repeat(129),
        ] {
            assert_eq!(n(bad), Err(BadValue::Name), "{bad}");
        }
    }

    #[test]
    fn a_list_writes_each_value_as_text_for_an_in_r_sql_114() {
        let l = |v: &str| ValueType::List.render(v);
        assert_eq!(l("QSYSOPR"), Ok("'QSYSOPR'".to_owned()));
        assert_eq!(
            l(" a, O'Brien ,,æøå, "),
            Ok("'a', 'O''Brien', 'æøå'".to_owned())
        );
        assert_eq!(l(""), Err(BadValue::List));
        assert_eq!(l(" , ,"), Err(BadValue::List));
        assert_eq!(
            fill("WHERE U IN (${users as list:A,B})", |_| None),
            "WHERE U IN ('A', 'B')"
        );
    }

    #[test]
    fn libraries_and_tables_are_written_as_names_r_sql_115() {
        assert_eq!(
            fill("SELECT * FROM ${lib as library:qgpl}.${t AS table}", |n| (n
                == "t")
                .then(|| "sysTables".to_owned())),
            "SELECT * FROM QGPL.SYSTABLES"
        );
        assert_eq!(ValueType::Table.render("1X"), Err(BadValue::Name));
    }

    #[test]
    fn filling_writes_each_value_as_its_type_wants_r_sql_110() {
        let sql = "SELECT * FROM ${lib as name}.T WHERE U = ${user as text:QSYSOPR} AND D > ${days as number} AND X = ${x}";
        let filled = fill(sql, |name| match name {
            "lib" => Some("qgpl".to_owned()),
            "days" => Some("1,5".to_owned()),
            "x" => Some("O'Brien".to_owned()),
            _ => None,
        });
        assert_eq!(
            filled,
            "SELECT * FROM QGPL.T WHERE U = 'QSYSOPR' AND D > 1.5 AND X = O'Brien"
        );
    }

    #[test]
    fn an_untyped_value_inside_a_string_has_its_quotes_doubled_r_sql_112() {
        let sql = "WHERE U = '${u}' AND T LIKE '%${t}%' AND X = ${x} AND F = '${f as foo}'";
        let filled = fill(sql, |name| match name {
            "u" => Some("O'Brien".to_owned()),
            "t" => Some("æ'ø".to_owned()),
            "x" => Some("'raw'".to_owned()),
            "f" => Some("a'b".to_owned()),
            _ => None,
        });
        assert_eq!(
            filled,
            "WHERE U = 'O''Brien' AND T LIKE '%æ''ø%' AND X = 'raw' AND F = 'a''b'"
        );
        assert_eq!(fill("'${u}'", |_| Some(String::new())), "''", "empty");
        assert_eq!(
            fill("'${u}", |_| Some("a'b".to_owned())),
            "'a''b",
            "never closed"
        );
        assert_eq!(
            fill("'${u:O'Brien}'", |_| None),
            "'O''Brien'",
            "a default too"
        );
    }

    #[test]
    fn text_and_dates_between_quotes_are_named_r_sql_110() {
        let sql = "WHERE A = '${a as text}' AND B = '${b as date}' AND C = '${c}' AND D = ${d as text} AND N = '${n as number}' AND A2 = '${a as text}'";
        assert_eq!(quoted_twice(sql), ["a", "b"]);
        assert_eq!(
            quoted_twice("${a as text}"),
            Vec::<String>::new(),
            "at the edges"
        );
    }
}
