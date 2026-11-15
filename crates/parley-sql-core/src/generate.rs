//! The SQL that rebuilds a library or a table (R-SQL-93..96): `QSYS2.GENERATE_SQL` asked for one
//! kind of object at a time with a `'%'` name, written in an order that can be run, and the
//! tables whose rows a backup reads.

use std::collections::BTreeSet;

use super::catalog::{self, Kind, cell, literal};
use super::protocol::Row;

/// A section of a library's script, in the order it is written: the tables first; then, after
/// a backup's rows, what would stop the rows going in (constraints, triggers) or needs the
/// tables (R-SQL-93, R-SQL-96).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Part {
    Tables,
    Constraints,
    Indexes,
    Functions,
    Views,
    Procedures,
    Triggers,
}

impl Part {
    pub const ALL: [Part; 7] = [
        Part::Tables,
        Part::Constraints,
        Part::Indexes,
        Part::Functions,
        Part::Views,
        Part::Procedures,
        Part::Triggers,
    ];

    /// Its code in the rows of [`contents`].
    fn code(self) -> &'static str {
        match self {
            Part::Tables => "T",
            Part::Constraints => "C",
            Part::Indexes => "I",
            Part::Functions => "F",
            Part::Views => "V",
            Part::Procedures => "P",
            Part::Triggers => "R",
        }
    }

    /// Its `DATABASE_OBJECT_TYPE` for `QSYS2.GENERATE_SQL`.
    fn object_type(self) -> &'static str {
        match self {
            Part::Tables => "TABLE",
            Part::Constraints => "CONSTRAINT",
            Part::Indexes => "INDEX",
            Part::Functions => "FUNCTION",
            Part::Views => "VIEW",
            Part::Procedures => "PROCEDURE",
            Part::Triggers => "TRIGGER",
        }
    }

    /// Its heading in the script.
    pub fn heading(self) -> &'static str {
        match self {
            Part::Tables => "Tables",
            Part::Constraints => "Constraints",
            Part::Indexes => "Indexes",
            Part::Functions => "Functions",
            Part::Views => "Views",
            Part::Procedures => "Procedures",
            Part::Triggers => "Triggers",
        }
    }

    /// The kind of object chosen in the dialog that brings it: a table's constraints and
    /// triggers come with the tables.
    pub fn kind(self) -> Kind {
        match self {
            Part::Tables | Part::Constraints | Part::Triggers => Kind::Table,
            Part::Indexes => Kind::Index,
            Part::Functions => Kind::Function,
            Part::Views => Kind::View,
            Part::Procedures => Kind::Procedure,
        }
    }
}

/// Every object of `part` in `library` (system name), from `QSYS2.GENERATE_SQL` with the name
/// `'%'`, without its header. Tables come without their constraints and triggers, and views
/// without their triggers: those are parts of their own, written after the rows. Read with
/// [`catalog::ddl_from`].
pub fn part(library: &str, part: Part) -> String {
    let options = match part {
        Part::Tables => ", CONSTRAINT_OPTION => '0', TRIGGER_OPTION => '0'",
        Part::Views => ", TRIGGER_OPTION => '0'",
        Part::Constraints | Part::Indexes | Part::Functions | Part::Procedures | Part::Triggers => {
            ""
        }
    };
    format!(
        "CALL QSYS2.GENERATE_SQL(DATABASE_OBJECT_NAME => '%', DATABASE_OBJECT_LIBRARY_NAME => {}, \
         DATABASE_OBJECT_TYPE => '{}', HEADER_OPTION => '0'{options})",
        literal(library),
        part.object_type()
    )
}

/// A table whose rows a backup writes (R-SQL-95, R-SQL-96).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataTable {
    /// The SQL name.
    pub name: String,
    /// It has a column `GENERATED ALWAYS` (identity, row change timestamp, `ROWID`): its rows
    /// go back with `OVERRIDING SYSTEM VALUE`.
    pub overriding: bool,
}

/// What a library holds, for a script or a backup: the parts it has any of, and its tables.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Contents {
    pub parts: BTreeSet<Part>,
    pub tables: Vec<DataTable>,
}

/// Which parts `library` (system name) has, and its tables (`T`, `P`: not views, aliases or
/// materialized query tables, whose rows come from elsewhere), by name; with `only`, just the
/// table or view of that SQL name. Asked first, so `QSYS2.GENERATE_SQL` is only asked for parts
/// there are. Read with [`contents_from`].
///
/// No `EXISTS`: IBM i 7.3 refuses it in a select list's `CASE` (SQL0104 at `EXISTS`). A join
/// says which tables have a column `GENERATED ALWAYS`, and `SELECT DISTINCT` gives one row per
/// part there is.
pub fn contents(library: &str, only: Option<&str>) -> String {
    let lib = literal(library);
    let which = only.map_or_else(
        || "T.TABLE_TYPE IN ('T', 'P')".to_owned(),
        |name| format!("T.TABLE_NAME = {}", literal(name)),
    );
    let any = |part: Part, from: &str| {
        format!(
            " UNION ALL SELECT DISTINCT '{}', '', '' FROM {from}",
            part.code()
        )
    };
    let mut sql = format!(
        "SELECT 'D', T.TABLE_NAME, CASE WHEN G.NAME IS NULL THEN 'N' ELSE 'Y' END \
         FROM QSYS2.SYSTABLES T LEFT JOIN (SELECT DISTINCT SYSTEM_TABLE_NAME AS NAME \
         FROM QSYS2.SYSCOLUMNS WHERE SYSTEM_TABLE_SCHEMA = {lib} AND HAS_DEFAULT IN ('A', 'E', 'I')) G \
         ON G.NAME = T.SYSTEM_TABLE_NAME \
         WHERE T.SYSTEM_TABLE_SCHEMA = {lib} AND {which}"
    );
    for (part, from) in [
        (
            Part::Tables,
            format!(
                "QSYS2.SYSTABLES WHERE SYSTEM_TABLE_SCHEMA = {lib} AND TABLE_TYPE IN ('T', 'P', 'M')"
            ),
        ),
        (
            Part::Constraints,
            format!("QSYS2.SYSCST WHERE SYSTEM_CONSTRAINT_SCHEMA = {lib}"),
        ),
        (
            Part::Indexes,
            format!("QSYS2.SYSINDEXES WHERE SYSTEM_INDEX_SCHEMA = {lib}"),
        ),
        (
            Part::Functions,
            format!("QSYS2.SYSROUTINES WHERE ROUTINE_SCHEMA = {lib} AND ROUTINE_TYPE = 'FUNCTION'"),
        ),
        (
            Part::Views,
            format!(
                "QSYS2.SYSTABLES WHERE SYSTEM_TABLE_SCHEMA = {lib} AND TABLE_TYPE IN ('V', 'L')"
            ),
        ),
        (
            Part::Procedures,
            format!(
                "QSYS2.SYSROUTINES WHERE ROUTINE_SCHEMA = {lib} AND ROUTINE_TYPE = 'PROCEDURE'"
            ),
        ),
        (
            Part::Triggers,
            format!("QSYS2.SYSTRIGGERS WHERE SYSTEM_TRIGGER_SCHEMA = {lib}"),
        ),
    ] {
        sql.push_str(&any(part, &from));
    }
    sql.push_str(" ORDER BY 1, 2");
    sql
}

/// The rows of [`contents`]; rows of an unknown code are skipped.
pub fn contents_from(rows: &[Row]) -> Contents {
    let mut contents = Contents::default();
    for row in rows {
        let code = cell(row, 0);
        if code == "D" {
            let name = cell(row, 1);
            if !name.is_empty() {
                contents.tables.push(DataTable {
                    name,
                    overriding: cell(row, 2) == "Y",
                });
            }
        } else if let Some(part) = Part::ALL.into_iter().find(|p| p.code() == code) {
            contents.parts.insert(part);
        }
    }
    contents
}

/// The indexes over the table `table` in `library` (system names) and the views built directly
/// on it, each as kind code, library and system name (R-SQL-94). Read with [`dependents_from`].
pub fn dependents(library: &str, table: &str) -> String {
    let (lib, table) = (literal(library), literal(table));
    format!(
        "SELECT 'I', SYSTEM_INDEX_SCHEMA, SYSTEM_INDEX_NAME FROM QSYS2.SYSINDEXES \
         WHERE SYSTEM_TABLE_SCHEMA = {lib} AND SYSTEM_TABLE_NAME = {table} \
         UNION SELECT 'V', SYSTEM_VIEW_SCHEMA, SYSTEM_VIEW_NAME FROM QSYS2.SYSVIEWDEP \
         WHERE SYSTEM_TABLE_SCHEMA = {lib} AND SYSTEM_TABLE_NAME = {table} ORDER BY 1, 2, 3"
    )
}

/// An object that depends on a table: its kind, library and system name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dependent {
    pub kind: Kind,
    pub library: String,
    pub name: String,
}

/// The rows of [`dependents`]: indexes first, then views.
pub fn dependents_from(rows: &[Row]) -> Vec<Dependent> {
    rows.iter()
        .filter_map(|row| {
            let kind = match cell(row, 0).as_str() {
                "I" => Kind::Index,
                "V" => Kind::View,
                _ => return None,
            };
            let (library, name) = (cell(row, 1), cell(row, 2));
            (!library.is_empty() && !name.is_empty()).then_some(Dependent {
                kind,
                library,
                name,
            })
        })
        .collect()
}

/// One error as a one-line SQL comment.
fn comment(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A part of a library's script under its heading; one that could not be generated says why
/// in a comment, so the rest still runs.
pub fn section(part: Part, generated: &Result<String, String>) -> String {
    let heading = part.heading();
    match generated {
        Ok(text) => format!("-- {heading}\n\n{}\n", text.trim_end()),
        Err(e) => format!("-- {heading}: not generated: {}\n", comment(e)),
    }
}

/// One object's SQL in a table's script (R-SQL-94), or why there is none, as a comment.
pub fn piece(label: &str, generated: &Result<String, String>) -> String {
    match generated {
        Ok(text) => format!("{}\n", text.trim_end()),
        Err(e) => format!("-- {label}: not generated: {}\n", comment(e)),
    }
}

/// Now, local time, as the headers say when a script or backup was written.
pub fn now() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M").to_string()
}

/// The first lines of a library's script (R-SQL-93), `when` written.
pub fn script_header(library: &str, when: &str) -> String {
    format!(
        "-- The SQL of library {}, from QSYS2.GENERATE_SQL, {when}.\n\
         -- In an order that can be run: tables, constraints, indexes, functions, views, \
         procedures, triggers.\n\n",
        catalog::identifier(library)
    )
}

/// The first lines of a library's backup (R-SQL-96), `when` written.
pub fn backup_header(library: &str, when: &str) -> String {
    format!(
        "-- A backup of library {lib} as SQL, {when}: its tables, their rows as INSERT statements,\n\
         -- then constraints, indexes, functions, views, procedures and triggers.\n\
         -- To restore, create the library if it is gone (CREATE SCHEMA {lib}) and run this script.\n\n",
        lib = catalog::identifier(library)
    )
}

/// The first line of one table's rows as SQL (R-SQL-95), `when` written.
pub fn rows_header(table: &str, when: &str) -> String {
    format!("-- The rows of {table} as INSERT statements, {when}.\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(cells: &[Option<&str>]) -> Row {
        cells.iter().map(|c| c.map(str::to_owned)).collect()
    }

    #[test]
    fn a_part_is_every_object_of_its_kind_without_the_constraints_of_tables_r_sql_93() {
        let sql = part("O'NEIL", Part::Tables);
        assert!(sql.contains("DATABASE_OBJECT_NAME => '%'"), "{sql}");
        assert!(
            sql.contains("DATABASE_OBJECT_LIBRARY_NAME => 'O''NEIL'"),
            "{sql}"
        );
        assert!(sql.contains("DATABASE_OBJECT_TYPE => 'TABLE'"), "{sql}");
        assert!(sql.contains("CONSTRAINT_OPTION => '0'"), "{sql}");
        assert!(sql.contains("TRIGGER_OPTION => '0'"), "{sql}");
        assert!(sql.contains("HEADER_OPTION => '0'"), "{sql}");
        let sql = part("QGPL", Part::Constraints);
        assert!(sql.contains("'CONSTRAINT', HEADER_OPTION => '0')"), "{sql}");
        assert!(part("QGPL", Part::Views).contains("TRIGGER_OPTION => '0'"));
        assert!(!part("QGPL", Part::Indexes).contains("_OPTION => '0', "));
    }

    #[test]
    fn the_parts_come_tables_first_and_triggers_last_r_sql_93() {
        let mut sorted = Part::ALL;
        sorted.sort();
        assert_eq!(sorted, Part::ALL);
        assert_eq!(Part::ALL.first(), Some(&Part::Tables));
        assert_eq!(Part::ALL.last(), Some(&Part::Triggers));
        assert_eq!(Part::Constraints.kind(), Kind::Table);
        assert_eq!(Part::Triggers.kind(), Kind::Table);
        assert_eq!(Part::Views.kind(), Kind::View);
    }

    #[test]
    fn contents_lists_the_parts_there_are_and_the_tables_with_rows_r_sql_96() {
        let sql = contents("O'NEIL", None);
        assert!(sql.contains("T.SYSTEM_TABLE_SCHEMA = 'O''NEIL'"), "{sql}");
        assert!(sql.contains("T.TABLE_TYPE IN ('T', 'P')"), "{sql}");
        assert!(sql.contains("HAS_DEFAULT IN ('A', 'E', 'I')"), "{sql}");
        assert!(sql.contains("QSYS2.SYSTRIGGERS"), "{sql}");
        // IBM i 7.3 refused `CASE WHEN EXISTS (…)` with SQL0104 (the owner, 2026-11-15).
        assert!(!sql.contains("EXISTS"), "{sql}");
        let one = contents("QGPL", Some("Kunder æøå"));
        assert!(one.contains("T.TABLE_NAME = 'Kunder æøå'"), "{one}");
        assert!(!one.contains("T.TABLE_TYPE IN"), "{one}");
        let read = contents_from(&[
            row(&[Some("D"), Some("KUNDER"), Some("N")]),
            row(&[Some("D"), Some("ORDRER"), Some("Y")]),
            row(&[Some("D"), Some(""), Some("N")]),
            row(&[Some("I"), Some(""), Some("")]),
            row(&[Some("T"), Some(""), Some("")]),
            row(&[Some("X"), None, None]),
            row(&[]),
        ]);
        assert_eq!(
            read.parts.into_iter().collect::<Vec<_>>(),
            [Part::Tables, Part::Indexes]
        );
        assert_eq!(
            read.tables,
            [
                DataTable {
                    name: "KUNDER".to_owned(),
                    overriding: false
                },
                DataTable {
                    name: "ORDRER".to_owned(),
                    overriding: true
                },
            ]
        );
    }

    #[test]
    fn a_tables_dependents_are_its_indexes_and_views_by_system_name_r_sql_94() {
        let sql = dependents("QGPL", "O'K");
        assert!(sql.contains("QSYS2.SYSINDEXES"), "{sql}");
        assert!(sql.contains("QSYS2.SYSVIEWDEP"), "{sql}");
        assert!(sql.contains("SYSTEM_TABLE_NAME = 'O''K'"), "{sql}");
        let found = dependents_from(&[
            row(&[Some("I"), Some("QGPL      "), Some("KUNDER_IX")]),
            row(&[Some("V"), Some("ANDEN"), Some("KUNDEV")]),
            row(&[Some("V"), None, Some("KUNDEV")]),
            row(&[Some("Q"), Some("QGPL"), Some("X")]),
        ]);
        assert_eq!(
            found,
            [
                Dependent {
                    kind: Kind::Index,
                    library: "QGPL".to_owned(),
                    name: "KUNDER_IX".to_owned()
                },
                Dependent {
                    kind: Kind::View,
                    library: "ANDEN".to_owned(),
                    name: "KUNDEV".to_owned()
                },
            ]
        );
    }

    #[test]
    fn a_part_that_failed_is_a_comment_so_the_script_still_runs_r_sql_93() {
        assert_eq!(
            section(
                Part::Views,
                &Ok("CREATE VIEW V AS SELECT 1 FROM T;\n\n".to_owned())
            ),
            "-- Views\n\nCREATE VIEW V AS SELECT 1 FROM T;\n"
        );
        assert_eq!(
            section(
                Part::Triggers,
                &Err("SQL0204\nTRIGGER æøå not found".to_owned())
            ),
            "-- Triggers: not generated: SQL0204 TRIGGER æøå not found\n"
        );
        assert_eq!(
            piece("QGPL.V", &Err("SQL0443".to_owned())),
            "-- QGPL.V: not generated: SQL0443\n"
        );
        assert_eq!(piece("QGPL.V", &Ok("X;\n".to_owned())), "X;\n");
        assert!(backup_header("My Lib", "2026-11-15 10:00").contains("CREATE SCHEMA \"My Lib\""));
        assert!(script_header("QGPL", "now").starts_with("-- The SQL of library QGPL"));
    }
}
