//! A result's rows as text for the clipboard (R-SQL-38): a Markdown table, `INSERT` statements
//! or JSON. Nulls stay null, numbers are not quoted, and text is escaped for each format.

use std::fmt::Write as _;

use super::catalog;
use super::protocol::{Column, Row};
use parley_core::CodePage;

/// Where `INSERT` statements go when the columns do not all come from one table.
pub const NO_TABLE: &str = "TABLE_NAME";

/// Whether the column holds bytes, sent as hex (R-SQL-39).
pub fn is_binary(column: &Column) -> bool {
    let kind = column.type_name.to_uppercase();
    kind.contains("BINARY") || kind.contains("FOR BIT DATA")
}

/// Whether `value` can stand as a number in SQL and JSON as it is.
fn is_number(value: &str) -> bool {
    let digits = value.strip_prefix('-').unwrap_or(value);
    let (whole, fraction) = digits.split_once('.').unwrap_or((digits, ""));
    !whole.is_empty()
        && whole.bytes().all(|b| b.is_ascii_digit())
        && fraction.bytes().all(|b| b.is_ascii_digit())
        && !(whole.len() > 1 && whole.starts_with('0'))
}

/// A Markdown table: `names` as the heading; `|` escaped, line breaks as blanks, null empty.
pub fn markdown(names: &[String], columns: &[Column], rows: &[&Row]) -> String {
    let cell = |s: &str| s.replace('|', "\\|").replace(['\r', '\n'], " ");
    let line = |cells: Vec<String>| format!("| {} |", cells.join(" | "));
    let mut out = vec![line(names.iter().map(|n| cell(n)).collect())];
    out.push(line(
        columns
            .iter()
            .map(|c| if c.numeric { "---:" } else { "---" }.to_owned())
            .collect(),
    ));
    for row in rows {
        out.push(line(
            row.iter()
                .map(|v| cell(v.as_deref().unwrap_or_default()))
                .collect(),
        ));
    }
    out.join("\n")
}

/// The table the columns come from, when they all come from one (R-SQL-29's library and table).
pub(crate) fn source_table(columns: &[Column]) -> Option<String> {
    let first = columns.first()?;
    let (schema, table) = (first.schema.as_deref()?, first.table.as_deref()?);
    let same = columns
        .iter()
        .all(|c| c.schema.as_deref() == Some(schema) && c.table.as_deref() == Some(table));
    (same && !schema.is_empty() && !table.is_empty()).then(|| catalog::qualified(schema, table))
}

/// One `INSERT` statement a row, into the table the columns come from or [`NO_TABLE`].
pub fn inserts(columns: &[Column], rows: &[&Row]) -> String {
    let table = source_table(columns).unwrap_or_else(|| NO_TABLE.to_owned());
    let head = insert_head(&table, columns, false);
    rows.iter()
        .map(|row| format!("{head} {};", values(columns, row)))
        .collect::<Vec<_>>()
        .join("\n")
}

/// `INSERT INTO table (columns)`, with `OVERRIDING SYSTEM VALUE` when the table has columns
/// `GENERATED ALWAYS` whose values must go back as they were (R-SQL-95).
pub fn insert_head(table: &str, columns: &[Column], overriding: bool) -> String {
    let names: Vec<String> = columns
        .iter()
        .map(|c| catalog::identifier(c.field.as_deref().unwrap_or(&c.name)))
        .collect();
    let overriding = if overriding {
        " OVERRIDING SYSTEM VALUE"
    } else {
        ""
    };
    format!("INSERT INTO {table} ({}){overriding}", names.join(", "))
}

/// `VALUES (…)` of one row: nulls as `NULL`, numbers as they are, bytes as `X'…'`, text quoted.
pub fn values(columns: &[Column], row: &Row) -> String {
    let values: Vec<String> = columns
        .iter()
        .enumerate()
        .map(|(i, column)| match row.get(i).cloned().flatten() {
            None => "NULL".to_owned(),
            Some(v) if column.numeric && is_number(&v) => v,
            Some(v) if is_binary(column) => format!("X'{v}'"),
            Some(v) => format!("'{}'", v.replace('\'', "''")),
        })
        .collect();
    format!("VALUES ({})", values.join(", "))
}

/// A JSON array of objects keyed by column name.
pub fn json(columns: &[Column], rows: &[&Row]) -> String {
    let text = |s: &str| serde_json::to_string(s).unwrap_or_else(|_| "null".to_owned());
    let objects: Vec<String> = rows
        .iter()
        .map(|row| {
            let fields: Vec<String> = columns
                .iter()
                .enumerate()
                .map(|(i, column)| {
                    let value = match row.get(i).cloned().flatten() {
                        None => "null".to_owned(),
                        Some(v) if column.numeric && is_number(&v) => v,
                        Some(v) => text(&v),
                    };
                    format!("{}: {value}", text(&column.name))
                })
                .collect();
            format!("  {{{}}}", fields.join(", "))
        })
        .collect();
    if objects.is_empty() {
        "[]".to_owned()
    } else {
        format!("[\n{}\n]", objects.join(",\n"))
    }
}

/// Bytes sent as hex (two digits a byte) as lines of 16: the offset, the bytes in groups of
/// four, and the same bytes read as EBCDIC in `page`, a byte with no character as `.`
/// (R-SQL-39). `None` when `hex` is not hex.
pub fn binary_dump(hex: &str, page: CodePage) -> Option<String> {
    if !hex.len().is_multiple_of(2) || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let bytes: Vec<u8> = (0..hex.len())
        .step_by(2)
        .map(|i| {
            hex.get(i..i + 2)
                .and_then(|d| u8::from_str_radix(d, 16).ok())
        })
        .collect::<Option<_>>()?;
    let lines: Vec<String> = bytes
        .chunks(16)
        .enumerate()
        .map(|(n, line)| {
            let groups: Vec<String> = line
                .chunks(4)
                .map(|g| {
                    g.iter().fold(String::new(), |mut out, b| {
                        // Writing to a String cannot fail.
                        let _ = write!(out, "{b:02X}");
                        out
                    })
                })
                .collect();
            let text: String = line
                .iter()
                .map(|&b| {
                    let c = page.to_char(b);
                    if b < 0x40 || c.is_control() { '.' } else { c }
                })
                .collect();
            format!("{:04X}  {:<35}  {text}", n * 16, groups.join(" "))
        })
        .collect();
    Some(lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn column(name: &str, type_name: &str, numeric: bool) -> Column {
        Column {
            name: name.to_owned(),
            type_name: type_name.to_owned(),
            length: 10,
            scale: 0,
            nullable: true,
            numeric,
            field: None,
            schema: Some("QGPL".to_owned()),
            table: Some("KUNDER".to_owned()),
            heading: None,
            text: None,
        }
    }

    fn rows() -> Vec<Row> {
        vec![
            vec![
                Some("10.50".to_owned()),
                Some("O'Brien | æøå".to_owned()),
                Some("C1F2".to_owned()),
            ],
            vec![None, Some("two\nlines".to_owned()), None],
        ]
    }

    fn columns() -> Vec<Column> {
        vec![
            column("BELØB", "DECIMAL", true),
            column("NAVN", "VARCHAR", false),
            column("RAW", "CHAR () FOR BIT DATA", false),
        ]
    }

    #[test]
    fn markdown_escapes_bars_and_line_breaks_and_aligns_numbers_r_sql_38() {
        let rows = rows();
        let refs: Vec<&Row> = rows.iter().collect();
        let names = ["BELØB", "NAVN", "RAW"].map(str::to_owned);
        assert_eq!(
            markdown(&names, &columns(), &refs),
            "| BELØB | NAVN | RAW |\n| ---: | --- | --- |\n| 10.50 | O'Brien \\| æøå | C1F2 |\n|  | two lines |  |"
        );
    }

    #[test]
    fn inserts_quote_text_keep_numbers_and_nulls_and_name_the_table_r_sql_38() {
        let rows = rows();
        let refs: Vec<&Row> = rows.iter().collect();
        assert_eq!(
            inserts(&columns(), &refs),
            "INSERT INTO QGPL.KUNDER (\"BELØB\", NAVN, RAW) VALUES (10.50, 'O''Brien | æøå', X'C1F2');\n\
             INSERT INTO QGPL.KUNDER (\"BELØB\", NAVN, RAW) VALUES (NULL, 'two\nlines', NULL);"
        );
        let mut mixed = columns();
        mixed[1].table = Some("ORDRE".to_owned());
        assert!(inserts(&mixed, &refs).starts_with("INSERT INTO TABLE_NAME "));
    }

    #[test]
    fn json_keeps_numbers_and_nulls_and_escapes_text_r_sql_38() {
        let rows = rows();
        let refs: Vec<&Row> = rows.iter().collect();
        assert_eq!(
            json(&columns(), &refs),
            "[\n  {\"BELØB\": 10.50, \"NAVN\": \"O'Brien | æøå\", \"RAW\": \"C1F2\"},\n  \
             {\"BELØB\": null, \"NAVN\": \"two\\nlines\", \"RAW\": null}\n]"
        );
        assert_eq!(json(&columns(), &[]), "[]");
    }

    #[test]
    fn bytes_show_as_hex_and_as_ebcdic_in_the_code_page_r_sql_39() {
        // "Æble" in CCSID 277 is 7B 82 93 85; 00 has no character.
        assert_eq!(
            binary_dump("7B82938500", CodePage::Ccsid277).as_deref(),
            Some(format!("0000  {:<35}  Æble.", "7B829385 00").as_str())
        );
        assert_eq!(
            binary_dump("7B", CodePage::Ccsid37).as_deref(),
            Some(format!("0000  {:<35}  #", "7B").as_str())
        );
        let long = binary_dump(&"C1".repeat(17), CodePage::Ccsid37).expect("hex");
        assert_eq!(long.lines().count(), 2);
        assert!(
            long.lines()
                .nth(1)
                .is_some_and(|l| l.starts_with("0010  C1 "))
        );
        assert_eq!(binary_dump("ABC", CodePage::Ccsid37), None, "odd length");
        assert_eq!(binary_dump("ZZ", CodePage::Ccsid37), None);
        assert_eq!(binary_dump("", CodePage::Ccsid37).as_deref(), Some(""));
    }

    #[test]
    fn only_plain_numbers_go_unquoted_r_sql_38() {
        for n in ["0", "-12", "3.25", "12345678901234567890.123456"] {
            assert!(is_number(n), "{n}");
        }
        for n in ["", "-", "1e5", "007", "1.2.3", "æ", "1,5"] {
            assert!(!is_number(n), "{n}");
        }
    }
}
