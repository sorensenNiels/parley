//! Upload a CSV file into a table, all or nothing (R-SQL-73, D31): reading the file, matching
//! its columns to the table's, and the `INSERT` and values the helper runs in one transaction.

use super::catalog::{self, ColumnInfo};
use crate::error::{Error, Result};

/// Rows sent to the helper in one request.
pub const BATCH: usize = 500;

/// The separators a file may use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Separator {
    #[default]
    Comma,
    Semicolon,
    Tab,
}

impl Separator {
    pub const ALL: [Self; 3] = [Self::Comma, Self::Semicolon, Self::Tab];

    pub const fn char(self) -> char {
        match self {
            Self::Comma => ',',
            Self::Semicolon => ';',
            Self::Tab => '\t',
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Comma => "Comma",
            Self::Semicolon => "Semicolon",
            Self::Tab => "Tab",
        }
    }

    /// The separator `text`'s first line uses most, outside quotes; a comma when none.
    pub fn found_in(text: &str) -> Self {
        let first = first_line(text.trim_start_matches('\u{FEFF}'));
        let mut quoted = false;
        let mut counts = [0usize; 3];
        for c in first.chars() {
            match c {
                '"' => quoted = !quoted,
                _ if quoted => {}
                ',' => counts[0] += 1,
                ';' => counts[1] += 1,
                '\t' => counts[2] += 1,
                _ => {}
            }
        }
        let best = counts
            .iter()
            .enumerate()
            .max_by_key(|&(i, n)| (*n, std::cmp::Reverse(i)))
            .map_or(0, |(i, _)| i);
        if counts.get(best).copied().unwrap_or(0) == 0 {
            Self::Comma
        } else {
            Self::ALL.get(best).copied().unwrap_or_default()
        }
    }
}

/// The first line of `text`, a line break inside quotes included.
fn first_line(text: &str) -> &str {
    let mut quoted = false;
    for (i, c) in text.char_indices() {
        match c {
            '"' => quoted = !quoted,
            '\n' | '\r' if !quoted => return text.get(..i).unwrap_or(text),
            _ => {}
        }
    }
    text
}

/// The records of a CSV file (RFC 4180): quoted fields may hold the separator, quotes doubled,
/// and line breaks; CRLF or LF ends a record; a byte-order mark and blank lines are skipped.
///
/// # Errors
/// A quote that is never closed, naming the line it opened on.
pub fn parse(text: &str, separator: Separator) -> Result<Vec<Vec<String>>> {
    let sep = separator.char();
    let mut records = Vec::new();
    let mut record = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut line = 1usize;
    let mut opened_on = 0usize;
    let mut chars = text.trim_start_matches('\u{FEFF}').chars().peekable();
    let mut end_record = |record: &mut Vec<String>, field: &mut String| {
        record.push(std::mem::take(field));
        let fields = std::mem::take(record);
        if !(fields.len() == 1 && fields.first().is_some_and(String::is_empty)) {
            records.push(fields);
        }
    };
    while let Some(c) = chars.next() {
        if quoted {
            match c {
                '"' if chars.peek() == Some(&'"') => {
                    chars.next();
                    field.push('"');
                }
                '"' => quoted = false,
                _ => {
                    if c == '\n' {
                        line += 1;
                    }
                    field.push(c);
                }
            }
            continue;
        }
        match c {
            '"' if field.is_empty() => {
                quoted = true;
                opened_on = line;
            }
            _ if c == sep => record.push(std::mem::take(&mut field)),
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                line += 1;
                end_record(&mut record, &mut field);
            }
            '\n' => {
                line += 1;
                end_record(&mut record, &mut field);
            }
            _ => field.push(c),
        }
    }
    if quoted {
        return Err(Error::Transfer(format!(
            "A quote opened on line {opened_on} is never closed."
        )));
    }
    if !field.is_empty() || !record.is_empty() {
        end_record(&mut record, &mut field);
    }
    Ok(records)
}

/// How the file's columns go into the table's.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Plan {
    /// (column of the file, column of the table), in the table's order.
    pub pairs: Vec<(usize, usize)>,
    /// The file's columns that match none of the table's, by name (or "column n").
    pub unmatched: Vec<String>,
    /// The table's columns the file leaves out.
    pub left_out: Vec<String>,
    /// Of those, the ones the table needs: NOT NULL without a default.
    pub needed: Vec<String>,
}

impl Plan {
    /// Matches the file's columns to `table`'s: by name, ignoring case, when the file's first
    /// row is `names`; else by position, for the `width` columns of the file's rows.
    pub fn new(table: &[ColumnInfo], names: Option<&[String]>, width: usize) -> Self {
        let mut plan = Self::default();
        match names {
            Some(names) => {
                for (f, name) in names.iter().enumerate() {
                    let wanted = name.trim();
                    match table
                        .iter()
                        .position(|c| c.name.eq_ignore_ascii_case(wanted))
                    {
                        Some(t) if !plan.pairs.iter().any(|&(_, u)| u == t) => {
                            plan.pairs.push((f, t));
                        }
                        Some(_) | None => plan.unmatched.push(wanted.to_owned()),
                    }
                }
            }
            None => {
                for f in 0..width {
                    if f < table.len() {
                        plan.pairs.push((f, f));
                    } else {
                        plan.unmatched.push(format!("column {}", f + 1));
                    }
                }
            }
        }
        plan.pairs.sort_by_key(|&(_, t)| t);
        for (t, column) in table.iter().enumerate() {
            if !plan.pairs.iter().any(|&(_, u)| u == t) {
                plan.left_out.push(column.name.clone());
                if !column.nullable && !column.has_default {
                    plan.needed.push(column.name.clone());
                }
            }
        }
        plan
    }

    /// What stops the upload before anything is sent, if anything.
    pub fn problem(&self) -> Option<String> {
        if self.pairs.is_empty() {
            return Some("No column of the file matches a column of the table.".to_owned());
        }
        (!self.needed.is_empty()).then(|| {
            format!(
                "The table needs a value for {}, which the file leaves out.",
                self.needed.join(", ")
            )
        })
    }

    /// The `INSERT` into `library.table` with a marker for each matched column.
    pub fn insert(&self, library: &str, table: &str, columns: &[ColumnInfo]) -> String {
        let names: Vec<String> = self
            .pairs
            .iter()
            .filter_map(|&(_, t)| columns.get(t).map(|c| catalog::identifier(&c.name)))
            .collect();
        let markers = vec!["?"; names.len()].join(", ");
        format!(
            "INSERT INTO {} ({}) VALUES ({markers})",
            catalog::qualified(library, table),
            names.join(", ")
        )
    }

    /// The values of `record` for the `INSERT`: an empty field is NULL where the column allows
    /// it; a field the record lacks is NULL or empty the same way.
    pub fn values(&self, record: &[String], columns: &[ColumnInfo]) -> Vec<Option<String>> {
        self.pairs
            .iter()
            .map(|&(f, t)| {
                let value = record.get(f).map_or("", String::as_str);
                let nullable = columns.get(t).is_some_and(|c| c.nullable);
                if value.is_empty() && nullable {
                    None
                } else {
                    Some(value.to_owned())
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn column(name: &str, nullable: bool, has_default: bool) -> ColumnInfo {
        ColumnInfo {
            name: name.to_owned(),
            data_type: "VARCHAR(10)".to_owned(),
            nullable,
            has_default,
            heading: String::new(),
            text: String::new(),
        }
    }

    fn strings(fields: &[&str]) -> Vec<String> {
        fields.iter().map(|f| (*f).to_owned()).collect()
    }

    #[test]
    fn quoted_fields_hold_separators_quotes_and_line_breaks_r_sql_73() {
        let text = "\u{FEFF}NAVN,NOTE\r\n\"Søren, Å\",\"say \"\"hi\"\"\nthere\"\r\n\r\nb,\n";
        let records = parse(text, Separator::Comma).expect("parses");
        assert_eq!(
            records,
            vec![
                strings(&["NAVN", "NOTE"]),
                strings(&["Søren, Å", "say \"hi\"\nthere"]),
                strings(&["b", ""]),
            ]
        );
    }

    #[test]
    fn a_last_line_without_a_break_and_an_empty_file_are_read_r_sql_73() {
        assert_eq!(
            parse("a;b", Separator::Semicolon).expect("parses"),
            vec![strings(&["a", "b"])]
        );
        assert_eq!(
            parse("", Separator::Comma).expect("parses"),
            Vec::<Vec<String>>::new()
        );
    }

    #[test]
    fn stray_quotes_old_mac_line_ends_and_ragged_rows_are_read_leniently_r_sql_73() {
        let text = "ab\"c,\"q\"rest\rx\ty;z\r\r1,2,3\næ";
        assert_eq!(
            parse(text, Separator::Comma).expect("parses"),
            vec![
                strings(&["ab\"c", "qrest"]),
                strings(&["x\ty;z"]),
                strings(&["1", "2", "3"]),
                strings(&["æ"]),
            ],
            "a quote inside a field is text; text after a closing quote joins the field; \
             a lone CR ends a record; rows may differ in width"
        );
        assert_eq!(
            parse("\u{FEFF}\n\n", Separator::Tab).expect("parses"),
            Vec::<Vec<String>>::new(),
            "only a byte-order mark and blank lines"
        );
        assert_eq!(
            parse("\"\",\"\"\"\"", Separator::Comma).expect("parses"),
            vec![strings(&["", "\""])],
            "an empty quoted field and one holding only a quote"
        );
    }

    #[test]
    fn a_quote_never_closed_names_its_line_r_sql_73() {
        let err = parse("a\n\"b,\nc", Separator::Comma).expect_err("open quote");
        assert!(err.to_string().contains("line 2"), "{err}");
    }

    #[test]
    fn the_separator_is_found_from_the_first_line_outside_quotes_r_sql_73() {
        assert_eq!(Separator::found_in("a;b;c\n1,2;3"), Separator::Semicolon);
        assert_eq!(Separator::found_in("\"a;b\",c\n"), Separator::Comma);
        assert_eq!(Separator::found_in("a\tb\n"), Separator::Tab);
        assert_eq!(Separator::found_in("one column"), Separator::Comma);
        assert_eq!(Separator::found_in(""), Separator::Comma);
    }

    #[test]
    fn columns_match_by_name_ignoring_case_and_what_is_needed_is_said_r_sql_73() {
        let table = vec![
            column("ID", false, true),
            column("NAVN", false, false),
            column("NOTE", true, false),
            column("BY", false, false),
        ];
        let names = strings(&["navn", "Ekstra", "id"]);
        let plan = Plan::new(&table, Some(&names), 3);
        assert_eq!(plan.pairs, vec![(2, 0), (0, 1)], "in the table's order");
        assert_eq!(plan.unmatched, ["Ekstra"]);
        assert_eq!(plan.left_out, ["NOTE", "BY"]);
        assert_eq!(plan.needed, ["BY"], "NOT NULL without a default");
        assert!(plan.problem().is_some_and(|p| p.contains("BY")));
        assert_eq!(
            plan.insert("LIB", "kunder", &table),
            "INSERT INTO LIB.\"kunder\" (ID, NAVN) VALUES (?, ?)"
        );
    }

    #[test]
    fn without_names_columns_match_by_position_r_sql_73() {
        let table = vec![column("A", true, false), column("B", true, false)];
        let plan = Plan::new(&table, None, 3);
        assert_eq!(plan.pairs, vec![(0, 0), (1, 1)]);
        assert_eq!(plan.unmatched, ["column 3"]);
        assert_eq!(plan.problem(), None);
        let none = Plan::new(&table, Some(&strings(&["X"])), 1);
        assert!(none.problem().is_some_and(|p| p.contains("No column")));
    }

    #[test]
    fn an_empty_field_is_null_where_the_column_allows_it_r_sql_73() {
        let table = vec![column("A", true, false), column("B", false, false)];
        let plan = Plan::new(&table, None, 2);
        assert_eq!(
            plan.values(&strings(&["", ""]), &table),
            vec![None, Some(String::new())]
        );
        assert_eq!(
            plan.values(&strings(&["æøå"]), &table),
            vec![Some("æøå".to_owned()), Some(String::new())],
            "a short record"
        );
    }
}
