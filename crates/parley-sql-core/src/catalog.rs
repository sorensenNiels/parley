//! The SQL the SQL window reads the IBM i catalog with (R-SQL-32..37), so the window and
//! `sqlprobe --tools` run the same text.

use super::protocol::Row;

/// The SQL job's current schema and library list, in order: each library's system name and
/// its kind (SYSTEM, PRODUCT, CURRENT, USER) (R-SQL-32); and the IBM i's relational database
/// name, by default its system name, for the root of the side panel's tree (R-SQL-83).
pub const JOB: &str = "SELECT CURRENT SCHEMA, SYSTEM_SCHEMA_NAME, TYPE, CURRENT SERVER \
    FROM QSYS2.LIBRARY_LIST_INFO ORDER BY ORDINAL_POSITION";

/// How many objects of a library are listed in the browser at first; a setting changes it
/// (R-SQL-34, R-SQL-127).
pub const MAX_OBJECTS: u32 = 5000;

/// `value` as an SQL string literal, its quotes doubled: names from the catalog are host data.
pub(crate) fn literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

/// The tables, views, indexes, procedures and functions in `library` (its system name), each as
/// kind code, SQL name, system name and text, by kind and name (R-SQL-34). Read with
/// [`objects_from`]. At most `max` (R-SQL-127).
pub fn objects(library: &str, max: u32) -> String {
    let lib = literal(library);
    format!(
        "SELECT KIND, NAME, SYSTEM_NAME, TEXT FROM (\
         SELECT CASE WHEN TABLE_TYPE IN ('V', 'L') THEN 'V' ELSE 'T' END AS KIND, \
         TABLE_NAME AS NAME, SYSTEM_TABLE_NAME AS SYSTEM_NAME, TABLE_TEXT AS TEXT \
         FROM QSYS2.SYSTABLES WHERE SYSTEM_TABLE_SCHEMA = {lib} \
         UNION ALL SELECT 'I', INDEX_NAME, SYSTEM_INDEX_NAME, INDEX_TEXT \
         FROM QSYS2.SYSINDEXES WHERE SYSTEM_INDEX_SCHEMA = {lib} \
         UNION ALL SELECT CASE ROUTINE_TYPE WHEN 'PROCEDURE' THEN 'P' ELSE 'F' END, \
         ROUTINE_NAME, SPECIFIC_NAME, CAST(LONG_COMMENT AS VARCHAR(2000)) \
         FROM QSYS2.SYSROUTINES WHERE ROUTINE_SCHEMA = {lib}\
         ) X ORDER BY KIND, NAME FETCH FIRST {max} ROWS ONLY"
    )
}

/// The columns of `table` in `library` (system names), in order: name, type, length, scale,
/// nullable, heading, text and whether it has a default (R-SQL-34, R-SQL-73). Read with
/// [`columns_from`].
pub fn columns(library: &str, table: &str) -> String {
    format!(
        "SELECT COLUMN_NAME, DATA_TYPE, LENGTH, NUMERIC_SCALE, IS_NULLABLE, COLUMN_HEADING, \
         COLUMN_TEXT, HAS_DEFAULT FROM QSYS2.SYSCOLUMNS WHERE SYSTEM_TABLE_SCHEMA = {} \
         AND SYSTEM_TABLE_NAME = {} ORDER BY ORDINAL_POSITION",
        literal(library),
        literal(table)
    )
}

/// The SQL that creates `name` (its system name; a routine's specific name) of `kind` in
/// `library`, as `QSYS2.GENERATE_SQL` writes it, a line a row (R-SQL-35). Read with [`ddl_from`].
pub fn ddl(library: &str, name: &str, kind: Kind) -> String {
    format!(
        "CALL QSYS2.GENERATE_SQL(DATABASE_OBJECT_NAME => {}, DATABASE_OBJECT_LIBRARY_NAME => {}, \
         DATABASE_OBJECT_TYPE => '{}')",
        literal(name),
        literal(library),
        kind.object_type()
    )
}

/// The statement text from the rows of [`ddl`]: the last column of each row, a line each.
pub fn ddl_from(rows: &[Row]) -> String {
    let lines: Vec<&str> = rows
        .iter()
        .map(|row| {
            row.last()
                .and_then(Option::as_deref)
                .unwrap_or_default()
                .trim_end()
        })
        .collect();
    let mut text = lines.join("\n");
    text.truncate(text.trim_end().len());
    text
}

/// The first rows of a table or view (SQL names), as R-SQL-35's "Select first rows" runs it.
pub fn first_rows(library: &str, name: &str) -> String {
    format!(
        "SELECT * FROM {} FETCH FIRST 100 ROWS ONLY",
        qualified(library, name)
    )
}

/// What an object in a library is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind {
    Table,
    View,
    Index,
    Procedure,
    Function,
}

impl Kind {
    pub const ALL: [Kind; 5] = [
        Kind::Table,
        Kind::View,
        Kind::Index,
        Kind::Procedure,
        Kind::Function,
    ];

    fn from_code(code: &str) -> Option<Kind> {
        match code {
            "T" => Some(Kind::Table),
            "V" => Some(Kind::View),
            "I" => Some(Kind::Index),
            "P" => Some(Kind::Procedure),
            "F" => Some(Kind::Function),
            _ => None,
        }
    }

    /// One of it, in lower case: in the completion list (R-SQL-36).
    pub fn singular(self) -> &'static str {
        match self {
            Kind::Table => "table",
            Kind::View => "view",
            Kind::Index => "index",
            Kind::Procedure => "procedure",
            Kind::Function => "function",
        }
    }

    /// The group's name in the browser.
    pub fn plural(self) -> &'static str {
        match self {
            Kind::Table => "Tables",
            Kind::View => "Views",
            Kind::Index => "Indexes",
            Kind::Procedure => "Procedures",
            Kind::Function => "Functions",
        }
    }

    /// Its `DATABASE_OBJECT_TYPE` for `QSYS2.GENERATE_SQL`.
    fn object_type(self) -> &'static str {
        match self {
            Kind::Table => "TABLE",
            Kind::View => "VIEW",
            Kind::Index => "INDEX",
            Kind::Procedure => "PROCEDURE",
            Kind::Function => "FUNCTION",
        }
    }

    /// Whether it has columns to show and rows to select.
    pub fn has_columns(self) -> bool {
        match self {
            Kind::Table | Kind::View => true,
            Kind::Index | Kind::Procedure | Kind::Function => false,
        }
    }
}

/// An object in a library.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Object {
    pub kind: Kind,
    /// The SQL name.
    pub name: String,
    /// The system name (for a routine, its specific name).
    pub system_name: String,
    pub text: String,
}

/// A column of a table or view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnInfo {
    pub name: String,
    /// The type as written in SQL: `DECIMAL(9, 2)`, `VARCHAR(50)`, `DATE`.
    pub data_type: String,
    pub nullable: bool,
    /// It has a default (or is an identity or row-change column): an `INSERT` may leave it out
    /// (R-SQL-73). `HAS_DEFAULT` is N for none.
    pub has_default: bool,
    /// The heading, its three 20-character lines joined with blanks.
    pub heading: String,
    pub text: String,
}

pub(crate) fn cell(row: &Row, n: usize) -> String {
    row.get(n)
        .cloned()
        .flatten()
        .unwrap_or_default()
        .trim()
        .to_owned()
}

/// The rows of [`objects`]; rows of an unknown kind or without a name are skipped.
pub fn objects_from(rows: &[Row]) -> Vec<Object> {
    rows.iter()
        .filter_map(|row| {
            let kind = Kind::from_code(&cell(row, 0))?;
            let name = cell(row, 1);
            (!name.is_empty()).then(|| Object {
                kind,
                system_name: cell(row, 2),
                text: cell(row, 3),
                name,
            })
        })
        .collect()
}

/// The rows of [`columns`].
pub fn columns_from(rows: &[Row]) -> Vec<ColumnInfo> {
    rows.iter()
        .filter(|row| !cell(row, 0).is_empty())
        .map(|row| {
            // The catalog's length is a decimal's precision.
            let length = cell(row, 2);
            ColumnInfo {
                name: cell(row, 0),
                data_type: type_text(&cell(row, 1), &length, &length, &cell(row, 3)),
                nullable: cell(row, 4) == "Y",
                has_default: !matches!(cell(row, 7).as_str(), "N" | ""),
                heading: cell(row, 5)
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" "),
                text: cell(row, 6),
            }
        })
        .collect()
}

/// The parameters of the procedures and functions called `name` in `library` (SQL names), a
/// row each, by specific name and position: specific name, position, mode, name, type, length,
/// precision and scale; a routine without parameters has one row of nulls after its specific
/// name (R-SQL-65). Read with [`signatures_from`].
pub fn parameters(library: &str, name: &str) -> String {
    format!(
        "SELECT R.SPECIFIC_NAME, P.ORDINAL_POSITION, P.PARAMETER_MODE, P.PARAMETER_NAME, \
         P.DATA_TYPE, P.CHARACTER_MAXIMUM_LENGTH, P.NUMERIC_PRECISION, P.NUMERIC_SCALE \
         FROM QSYS2.SYSROUTINES R LEFT JOIN QSYS2.SYSPARMS P \
         ON P.SPECIFIC_SCHEMA = R.SPECIFIC_SCHEMA AND P.SPECIFIC_NAME = R.SPECIFIC_NAME \
         AND P.ROW_TYPE = 'P' \
         WHERE R.ROUTINE_SCHEMA = {} AND R.ROUTINE_NAME = {} \
         ORDER BY R.SPECIFIC_NAME, P.ORDINAL_POSITION",
        literal(library),
        literal(name)
    )
}

/// A parameter of a routine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parameter {
    /// IN, OUT or INOUT; empty for a function's.
    pub mode: String,
    pub name: String,
    pub data_type: String,
}

/// One signature of a routine: its specific name and its parameters, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signature {
    pub specific_name: String,
    pub parameters: Vec<Parameter>,
}

/// The rows of [`parameters`], a signature per specific name.
pub fn signatures_from(rows: &[Row]) -> Vec<Signature> {
    let mut out: Vec<Signature> = Vec::new();
    for row in rows {
        let specific_name = cell(row, 0);
        if specific_name.is_empty() {
            continue;
        }
        if out.last().is_none_or(|s| s.specific_name != specific_name) {
            out.push(Signature {
                specific_name,
                parameters: Vec::new(),
            });
        }
        let kind = cell(row, 4);
        if kind.is_empty() {
            continue;
        }
        let data_type = type_text(&kind, &cell(row, 5), &cell(row, 6), &cell(row, 7));
        if let Some(signature) = out.last_mut() {
            signature.parameters.push(Parameter {
                mode: cell(row, 2),
                name: cell(row, 3),
                data_type,
            });
        }
    }
    out
}

/// A type as written in SQL from its kind, length, precision and scale.
fn type_text(kind: &str, length: &str, precision: &str, scale: &str) -> String {
    match kind {
        "DECIMAL" | "NUMERIC" => format!("{kind}({precision}, {})", scale_or_zero(scale)),
        "CHAR" | "VARCHAR" | "GRAPHIC" | "VARGRAPHIC" | "BINARY" | "VARBINARY" | "CHARACTER"
        | "CHARACTER VARYING" => format!("{kind}({length})"),
        _ => kind.to_owned(),
    }
}

fn scale_or_zero(scale: &str) -> &str {
    if scale.is_empty() { "0" } else { scale }
}

/// `name` as written in a statement: as it is when it is an ordinary identifier, else quoted.
pub fn identifier(name: &str) -> String {
    let ordinary = name
        .bytes()
        .next()
        .is_some_and(|b| b.is_ascii_uppercase() || matches!(b, b'#' | b'$' | b'@'))
        && name.bytes().all(|b| {
            b.is_ascii_uppercase() || b.is_ascii_digit() || matches!(b, b'_' | b'#' | b'$' | b'@')
        });
    if ordinary {
        name.to_owned()
    } else {
        format!("\"{}\"", name.replace('"', "\"\""))
    }
}

/// `library.name`, each part as [`identifier`] writes it.
pub fn qualified(library: &str, name: &str) -> String {
    format!("{}.{}", identifier(library), identifier(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(cells: &[Option<&str>]) -> Row {
        cells.iter().map(|c| c.map(str::to_owned)).collect()
    }

    #[test]
    fn catalog_queries_quote_names_from_the_host_r_sql_34() {
        let sql = objects("O'NEIL", MAX_OBJECTS);
        assert!(sql.contains("SYSTEM_TABLE_SCHEMA = 'O''NEIL'"), "{sql}");
        assert!(sql.contains(&format!("FETCH FIRST {MAX_OBJECTS} ROWS")));
        // R-SQL-127: the number of objects is the setting's.
        assert!(objects("QGPL", 40_000).contains("FETCH FIRST 40000 ROWS"));
        let sql = columns("QSYS2", "SYSTABLES");
        assert!(sql.contains("SYSTEM_TABLE_NAME = 'SYSTABLES'"), "{sql}");
    }

    #[test]
    fn object_rows_become_objects_r_sql_34() {
        let objects = objects_from(&[
            row(&[
                Some("T"),
                Some("CUSTOMER"),
                Some("CUSTOMER"),
                Some("Kunder æøå "),
            ]),
            row(&[Some("P"), Some("NIGHTLY_RUN"), Some("NIGHT00001"), None]),
            row(&[Some("X"), Some("ODD"), Some("ODD"), None]),
            row(&[Some("V"), None, None, None]),
            row(&[]),
        ]);
        assert_eq!(objects.len(), 2);
        assert_eq!(objects[0].kind, Kind::Table);
        assert_eq!(objects[0].text, "Kunder æøå");
        assert_eq!(objects[1].kind, Kind::Procedure);
        assert_eq!(objects[1].system_name, "NIGHT00001");
    }

    #[test]
    fn column_rows_say_the_type_as_sql_writes_it_r_sql_34() {
        let columns = columns_from(&[
            row(&[
                Some("AMOUNT"),
                Some("DECIMAL"),
                Some("9"),
                Some("2"),
                Some("Y"),
                Some("Beløb               i kr"),
                Some(""),
            ]),
            row(&[
                Some("NAME"),
                Some("VARCHAR"),
                Some("50"),
                None,
                Some("N"),
                None,
                Some("Navn"),
            ]),
            row(&[
                Some("BORN"),
                Some("DATE"),
                Some("4"),
                None,
                Some("N"),
                None,
                None,
            ]),
            row(&[None]),
        ]);
        assert_eq!(columns.len(), 3);
        assert_eq!(columns[0].data_type, "DECIMAL(9, 2)");
        assert!(columns[0].nullable);
        assert_eq!(columns[0].heading, "Beløb i kr");
        assert_eq!(columns[1].data_type, "VARCHAR(50)");
        assert_eq!(columns[2].data_type, "DATE");
    }

    #[test]
    fn the_ddl_is_generated_by_system_name_and_read_a_line_a_row_r_sql_35() {
        let sql = ddl("QGPL", "NIGHT00001", Kind::Procedure);
        assert!(
            sql.contains("DATABASE_OBJECT_NAME => 'NIGHT00001'"),
            "{sql}"
        );
        assert!(sql.contains("DATABASE_OBJECT_TYPE => 'PROCEDURE'"), "{sql}");
        let text = ddl_from(&[
            row(&[Some("1"), Some("0"), Some("CREATE TABLE QGPL.KUNDER (   ")]),
            row(&[Some("2"), Some("0"), Some("  NAVN VARCHAR(50) -- æøå")]),
            row(&[Some("3"), Some("0"), None]),
            row(&[Some("4"), Some("0"), Some(")  ")]),
            row(&[Some("5"), Some("0"), Some("")]),
        ]);
        assert_eq!(
            text,
            "CREATE TABLE QGPL.KUNDER (\n  NAVN VARCHAR(50) -- æøå\n\n)"
        );
        assert_eq!(ddl_from(&[]), "");
        assert_eq!(
            first_rows("QGPL", "My Table"),
            "SELECT * FROM QGPL.\"My Table\" FETCH FIRST 100 ROWS ONLY"
        );
    }

    #[test]
    fn names_are_quoted_only_when_they_must_be_r_sql_34() {
        assert_eq!(qualified("QGPL", "CUSTOMER"), "QGPL.CUSTOMER");
        assert_eq!(identifier("My Table"), "\"My Table\"");
        assert_eq!(identifier("a\"b"), "\"a\"\"b\"");
        assert_eq!(identifier("ÆØÅ"), "\"ÆØÅ\"");
        assert_eq!(identifier("#OK_1"), "#OK_1");
    }

    #[test]
    fn parameters_are_asked_by_name_with_quotes_doubled_r_sql_65() {
        let sql = parameters("QGPL", "O'HARA");
        assert!(sql.contains("ROUTINE_NAME = 'O''HARA'"), "{sql}");
        assert!(sql.contains("ROW_TYPE = 'P'"), "{sql}");
    }

    #[test]
    fn signatures_are_read_a_specific_name_each_r_sql_65() {
        let rows = [
            row(&[
                Some("BELØB1"),
                Some("1"),
                Some("IN"),
                Some("KUNDE"),
                Some("DECIMAL"),
                None,
                Some("9"),
                Some("0"),
            ]),
            row(&[
                Some("BELØB1"),
                Some("2"),
                Some("OUT"),
                Some("NAVN"),
                Some("VARCHAR"),
                Some("50"),
                None,
                None,
            ]),
            row(&[Some("BELØB2"), None, None, None, None, None, None, None]),
            row(&[None]),
        ];
        let signatures = signatures_from(&rows);
        assert_eq!(signatures.len(), 2);
        assert_eq!(
            signatures[0].parameters,
            [
                Parameter {
                    mode: "IN".to_owned(),
                    name: "KUNDE".to_owned(),
                    data_type: "DECIMAL(9, 0)".to_owned()
                },
                Parameter {
                    mode: "OUT".to_owned(),
                    name: "NAVN".to_owned(),
                    data_type: "VARCHAR(50)".to_owned()
                },
            ]
        );
        assert!(signatures[1].parameters.is_empty(), "no parameters");
    }

    /// Recorded from the owner's IBM i 7.3 (2026-11-14, `sqlprobe --sql`): `QSYS2.QCMDEXC` has
    /// three signatures, the first with parameters that have no name.
    #[test]
    fn qcmdexc_reads_as_three_signatures_with_unnamed_parameters_r_sql_65() {
        let rows = [
            row(&[
                Some("QCMDEXC"),
                Some("1"),
                Some("IN"),
                None,
                Some("CHARACTER VARYING"),
                Some("32000"),
                None,
                None,
            ]),
            row(&[
                Some("QCMDEXC"),
                Some("2"),
                Some("IN"),
                None,
                Some("DECIMAL"),
                None,
                Some("15"),
                Some("5"),
            ]),
            row(&[
                Some("QCMDEXC1"),
                Some("1"),
                Some("IN"),
                Some("COMMAND"),
                Some("CHARACTER VARYING"),
                Some("32000"),
                None,
                None,
            ]),
            row(&[
                Some("QCMDEXC2"),
                Some("1"),
                Some("IN"),
                Some("COMMAND"),
                Some("CHARACTER VARYING"),
                Some("32000"),
                None,
                None,
            ]),
        ];
        let signatures = signatures_from(&rows);
        assert_eq!(signatures.len(), 3);
        let first: Vec<(&str, &str)> = signatures[0]
            .parameters
            .iter()
            .map(|p| (p.name.as_str(), p.data_type.as_str()))
            .collect();
        assert_eq!(
            first,
            [("", "CHARACTER VARYING(32000)"), ("", "DECIMAL(15, 5)")]
        );
        assert_eq!(signatures[1].parameters[0].name, "COMMAND");
    }
}
