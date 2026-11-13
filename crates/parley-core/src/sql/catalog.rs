//! The SQL the SQL window reads the IBM i catalog with (R-SQL-32..37), so the window and
//! `sqlprobe --tools` run the same text.

use super::protocol::Row;

/// The SQL job's current schema and library list, in order: each library's system name and
/// its kind (SYSTEM, PRODUCT, CURRENT, USER) (R-SQL-32).
pub const JOB: &str = "SELECT CURRENT SCHEMA, SYSTEM_SCHEMA_NAME, TYPE FROM QSYS2.LIBRARY_LIST_INFO \
    ORDER BY ORDINAL_POSITION";

/// At most this many objects of a library are listed in the browser (R-SQL-34).
pub const MAX_OBJECTS: u32 = 5000;

/// `value` as an SQL string literal, its quotes doubled: names from the catalog are host data.
fn literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

/// The tables, views, indexes, procedures and functions in `library` (its system name), each as
/// kind code, SQL name, system name and text, by kind and name (R-SQL-34). Read with
/// [`objects_from`].
pub fn objects(library: &str) -> String {
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
         ) X ORDER BY KIND, NAME FETCH FIRST {MAX_OBJECTS} ROWS ONLY"
    )
}

/// The columns of `table` in `library` (system names), in order: name, type, length, scale,
/// nullable, heading and text (R-SQL-34). Read with [`columns_from`].
pub fn columns(library: &str, table: &str) -> String {
    format!(
        "SELECT COLUMN_NAME, DATA_TYPE, LENGTH, NUMERIC_SCALE, IS_NULLABLE, COLUMN_HEADING, \
         COLUMN_TEXT FROM QSYS2.SYSCOLUMNS WHERE SYSTEM_TABLE_SCHEMA = {} \
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
    /// The heading, its three 20-character lines joined with blanks.
    pub heading: String,
    pub text: String,
}

fn cell(row: &Row, n: usize) -> String {
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
            let (kind, length, scale) = (cell(row, 1), cell(row, 2), cell(row, 3));
            let data_type = match kind.as_str() {
                "DECIMAL" | "NUMERIC" => format!("{kind}({length}, {})", scale_or_zero(&scale)),
                "CHAR" | "VARCHAR" | "GRAPHIC" | "VARGRAPHIC" | "BINARY" | "VARBINARY" => {
                    format!("{kind}({length})")
                }
                _ => kind,
            };
            ColumnInfo {
                name: cell(row, 0),
                data_type,
                nullable: cell(row, 4) == "Y",
                heading: cell(row, 5)
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" "),
                text: cell(row, 6),
            }
        })
        .collect()
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
        let sql = objects("O'NEIL");
        assert!(sql.contains("SYSTEM_TABLE_SCHEMA = 'O''NEIL'"), "{sql}");
        assert!(sql.contains(&format!("FETCH FIRST {MAX_OBJECTS} ROWS")));
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
}
