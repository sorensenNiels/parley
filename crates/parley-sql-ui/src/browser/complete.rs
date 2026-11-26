//! The browser's part in completion (R-SQL-36, R-SQL-76), in hovering a name (R-SQL-37) and
//! in an upload's columns (R-SQL-73): from what was read, asking for what is missing.

use parley_sql_core::catalog::{self, ColumnInfo, Kind, Object};
use parley_sql_core::complete::{self, Typed};
use parley_sql_core::text;

use super::{Browser, Load};
use crate::database::Database;
use crate::editor::Item;

impl Browser {
    /// What to offer for `typed` in `statement`: after `x.` the columns of the table or alias
    /// `x`, else the tables of library `x`; without a qualifier, the columns of the statement's
    /// tables, the tables of `schema`, the libraries and the keywords. From what was read;
    /// what is missing is asked for, and offered once answered.
    pub fn candidates(
        &mut self,
        typed: &Typed,
        statement: &str,
        schema: Option<&str>,
        list: &[(String, String)],
        db: &mut Database,
    ) -> Vec<Item> {
        let tables = complete::tables_in(statement);
        // R-SQL-76: a WITH's names are tables of the statement's own.
        let ctes = complete::ctes(statement);
        let cte_of = |t: &complete::TableRef| {
            t.library
                .is_none()
                .then(|| ctes.iter().find(|c| c.name == t.table))
                .flatten()
        };
        let mut items = Vec::new();
        let prefix = typed.prefix.as_str();
        if let Some(qualifier) = &typed.qualifier {
            // An alias first: `FROM x` inside a WITH is not the `x` of `FROM t x` (R-SQL-76).
            let table = tables
                .iter()
                .find(|t| t.alias.as_deref() == Some(qualifier))
                .or_else(|| {
                    tables
                        .iter()
                        .find(|t| t.alias.is_none() && t.table == *qualifier)
                });
            let cte = table
                .and_then(cte_of)
                .or_else(|| ctes.iter().find(|c| c.name == *qualifier));
            if let Some(cte) = cte {
                cte_columns(cte, prefix, &mut items);
            } else {
                match table {
                    Some(t) => {
                        let library = t
                            .library
                            .as_deref()
                            .or(schema)
                            .unwrap_or_default()
                            .to_owned();
                        self.column_items(&library, &t.table, prefix, db, &mut items);
                    }
                    None => self.object_items(qualifier, prefix, db, &mut items),
                }
            }
        } else {
            for cte in tables.iter().filter_map(cte_of) {
                cte_columns(cte, prefix, &mut items);
            }
            for cte in ctes.iter().filter(|c| starts(&c.name, prefix)) {
                items.push(Item {
                    label: cte.name.clone(),
                    detail: "WITH".to_owned(),
                    insert: catalog::identifier(&cte.name),
                    snippet: None,
                });
            }
            for t in tables.iter().filter(|t| cte_of(t).is_none()) {
                let library = t
                    .library
                    .as_deref()
                    .or(schema)
                    .unwrap_or_default()
                    .to_owned();
                self.column_items(&library, &t.table, prefix, db, &mut items);
            }
            if let Some(schema) = schema {
                self.object_items(schema, prefix, db, &mut items);
            }
            // R-SQL-83: the libraries of every group, as the pinned ones were (R-SQL-36).
            let grouped = self.groups.iter().flat_map(|g| &g.libraries);
            let libraries = list.iter().map(|(l, _)| l).chain(grouped);
            for library in libraries {
                if starts(library, prefix) && !items.iter().any(|i: &Item| i.label == *library) {
                    items.push(Item {
                        label: library.clone(),
                        detail: "library".to_owned(),
                        insert: catalog::identifier(library),
                        snippet: None,
                    });
                }
            }
            items.extend(complete::keywords(prefix).map(|k| Item {
                label: k.to_owned(),
                detail: "keyword".to_owned(),
                insert: k.to_owned(),
                snippet: None,
            }));
        }
        // The window arranges and cuts the list (R-SQL-63).
        items
    }

    /// The libraries of the job's library list `list` and of the groups starting with `prefix`
    /// (in capitals), in that order, each once: what a library placeholder offers (R-SQL-115).
    pub fn library_names(&self, prefix: &str, list: &[(String, String)]) -> Vec<String> {
        let grouped = self.groups.iter().flat_map(|g| &g.libraries);
        let mut names: Vec<String> = Vec::new();
        for library in list.iter().map(|(l, _)| l).chain(grouped) {
            if library.to_uppercase().starts_with(prefix) && !names.contains(library) {
                names.push(library.clone());
            }
        }
        names
    }

    /// The objects but indexes of `library` starting with `prefix` (in capitals), asked for
    /// when not read: what a table placeholder offers (R-SQL-115).
    pub fn table_names(&mut self, library: &str, prefix: &str, db: &mut Database) -> Vec<String> {
        self.objects_of(library, db)
            .map_or_else(Vec::new, |objects| {
                objects
                    .iter()
                    .filter(|o| o.kind != Kind::Index && o.name.to_uppercase().starts_with(prefix))
                    .map(|o| o.name.clone())
                    .collect()
            })
    }

    /// The objects of `library` (asked for when not read), but its indexes.
    fn objects_of(&mut self, library: &str, db: &mut Database) -> Option<&[Object]> {
        text::system_name(library)?;
        match self.objects_load(library, db) {
            Load::Ready(objects) => Some(objects),
            Load::Reading | Load::Failed(_) => None,
        }
    }

    fn object_items(
        &mut self,
        library: &str,
        prefix: &str,
        db: &mut Database,
        items: &mut Vec<Item>,
    ) {
        let Some(objects) = self.objects_of(library, db) else {
            return;
        };
        for o in objects
            .iter()
            .filter(|o| o.kind != Kind::Index && starts(&o.name, prefix))
        {
            items.push(Item {
                label: o.name.clone(),
                detail: o.kind.singular().to_owned(),
                insert: catalog::identifier(&o.name),
                snippet: None,
            });
        }
    }

    /// The columns of the table or view with system name `system` in `library` (asked for when
    /// not read).
    fn columns_of(
        &mut self,
        library: &str,
        system: &str,
        db: &mut Database,
    ) -> Option<&[ColumnInfo]> {
        match self.columns_load(library, system, db) {
            Load::Ready(columns) => Some(columns),
            Load::Reading | Load::Failed(_) => None,
        }
    }

    /// The columns of the table or view `table` (its SQL name, as typed or upper case) in
    /// `library`, for an upload (R-SQL-73): `None` while being read, else the table's SQL name
    /// and its columns, or why there are none. What is missing is asked for.
    pub fn table_columns(
        &mut self,
        library: &str,
        table: &str,
        db: &mut Database,
    ) -> Option<Result<(String, Vec<ColumnInfo>), String>> {
        let Some(library) = text::system_name(library) else {
            return Some(Err(format!(
                "\"{}\" is not an IBM i library name.",
                library.trim()
            )));
        };
        let wanted = table.trim().trim_matches('"');
        if wanted.is_empty() {
            return Some(Err("Name the table.".to_owned()));
        }
        let object = match self.objects_load(&library, db) {
            Load::Reading => return None,
            Load::Failed(why) => return Some(Err(why.clone())),
            Load::Ready(objects) => objects
                .iter()
                .find(|o| {
                    o.kind == catalog::Kind::Table
                        && (o.name == wanted || o.name == wanted.to_uppercase())
                })
                .cloned(),
        };
        let Some(object) = object else {
            return Some(Err(format!("There is no table {wanted} in {library}.")));
        };
        match self.columns_load(&library, &object.system_name, db) {
            Load::Reading => None,
            Load::Failed(why) => Some(Err(why.clone())),
            Load::Ready(columns) => Some(Ok((object.name, columns.clone()))),
        }
    }

    /// The table or view `table` (its SQL name) in `library`, once its library is read.
    fn table_of(&mut self, library: &str, table: &str, db: &mut Database) -> Option<Object> {
        self.objects_of(library, db)?
            .iter()
            .find(|o| o.kind.has_columns() && o.name == table)
            .cloned()
    }

    /// The columns of `table` (its SQL name) in `library`: its system name is looked up first.
    fn column_items(
        &mut self,
        library: &str,
        table: &str,
        prefix: &str,
        db: &mut Database,
        items: &mut Vec<Item>,
    ) {
        let Some(object) = self.table_of(library, table, db) else {
            return;
        };
        let Some(columns) = self.columns_of(library, &object.system_name, db) else {
            return;
        };
        for c in columns.iter().filter(|c| starts(&c.name, prefix)) {
            items.push(Item {
                label: c.name.clone(),
                detail: column_detail(c),
                insert: catalog::identifier(&c.name),
                snippet: None,
            });
        }
    }

    /// What hovering `name` (after `qualifier.`, if any) in `statement` shows: the table or
    /// view, its text and its columns (R-SQL-37). `None` for anything else, or until read.
    pub fn describe(
        &mut self,
        qualifier: Option<&str>,
        name: &str,
        statement: &str,
        schema: Option<&str>,
        db: &mut Database,
    ) -> Option<String> {
        let tables = complete::tables_in(statement);
        let by_alias = tables.iter().find(|t| t.alias.as_deref() == Some(name));
        let (library, table) = match (qualifier, by_alias) {
            // `k.name`: a column of the table aliased `k`, not a table.
            (Some(q), _) if tables.iter().any(|t| t.alias.as_deref() == Some(q)) => return None,
            (Some(q), _) => (q.to_owned(), name.to_owned()),
            (None, Some(t)) => (t.library.as_deref().or(schema)?.to_owned(), t.table.clone()),
            (None, None) => {
                let named = tables.iter().find(|t| t.table == name);
                let library = named.and_then(|t| t.library.as_deref()).or(schema)?;
                (library.to_owned(), name.to_owned())
            }
        };
        let object = self.table_of(&library, &table, db)?;
        let mut lines = vec![format!(
            "{} ({})",
            catalog::qualified(&library, &object.name),
            object.kind.singular()
        )];
        if !object.text.is_empty() {
            lines.push(object.text.clone());
        }
        lines.push(String::new());
        match self.columns_of(&library, &object.system_name, db) {
            Some(columns) => {
                for c in columns.iter().take(HOVER_COLUMNS) {
                    lines.push(format!("{}  {}", c.name, column_detail(c)));
                }
                if columns.len() > HOVER_COLUMNS {
                    lines.push(format!("… and {} more", columns.len() - HOVER_COLUMNS));
                }
            }
            None => lines.push("Reading the columns…".to_owned()),
        }
        Some(lines.join("\n"))
    }
}

/// At most this many columns are listed on hover.
const HOVER_COLUMNS: usize = 25;

/// A column's type, and its heading when it has one.
fn column_detail(c: &ColumnInfo) -> String {
    if c.heading.is_empty() {
        c.data_type.clone()
    } else {
        format!("{} · {}", c.data_type, c.heading)
    }
}

/// The columns of a `WITH`'s table `cte` that start with `prefix` (R-SQL-76).
fn cte_columns(cte: &complete::Cte, prefix: &str, items: &mut Vec<Item>) {
    for column in cte.columns.iter().filter(|c| starts(c, prefix)) {
        if !items.iter().any(|i| i.label == *column) {
            items.push(Item {
                label: column.clone(),
                detail: format!("column of {}", cte.name),
                insert: catalog::identifier(column),
                snippet: None,
            });
        }
    }
}

/// Whether `name` starts with the typed `prefix` (in capitals) and is not all of it.
fn starts(name: &str, prefix: &str) -> bool {
    let name = name.to_uppercase();
    name.starts_with(prefix) && name != prefix
}
