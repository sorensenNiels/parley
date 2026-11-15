//! The browser's answers to the SQL assistant's lookups (R-SQL-44, D25): names, kinds, types,
//! headings and texts only.

use parley_sql_core::assistant::Lookup;
use parley_sql_core::catalog::{self, ColumnInfo, Kind, Object};
use parley_sql_core::text;

use super::{Browser, Load};
use crate::database::Database;

/// At most this many objects go to the assistant in one answer.
const ASSISTANT_OBJECTS: usize = 500;

impl Browser {
    /// The answer to a lookup the assistant asked for, as text: names, kinds, types,
    /// headings and texts only (D25). `None` while the catalog is still being read.
    pub fn tool_answer(&mut self, lookup: &Lookup, db: &mut Database) -> Option<String> {
        let library = match lookup {
            Lookup::Tables { library } | Lookup::Columns { library, .. } => library,
        };
        let Some(library) = text::system_name(library) else {
            return Some(format!("\"{library}\" is not an IBM i library name."));
        };
        let objects = match self.objects_load(&library, db) {
            Load::Reading => return None,
            Load::Failed(why) => return Some(format!("Could not read library {library}: {why}")),
            Load::Ready(objects) => objects.clone(),
        };
        let table = match lookup {
            Lookup::Tables { .. } => return Some(object_lines(&library, &objects)),
            Lookup::Columns { table, .. } => table,
        };
        let wanted = table.trim().trim_matches('"');
        let Some(object) = objects.iter().find(|o| {
            o.kind.has_columns() && (o.name == wanted || o.name == wanted.to_uppercase())
        }) else {
            return Some(format!("There is no table or view {wanted} in {library}."));
        };
        match self.columns_load(&library, &object.system_name, db) {
            Load::Reading => None,
            Load::Failed(why) => Some(format!("Could not read the columns of {wanted}: {why}")),
            Load::Ready(columns) => Some(column_lines(&library, object, columns)),
        }
    }
}

/// A library's objects for the assistant, a line each.
fn object_lines(library: &str, objects: &[Object]) -> String {
    let mut lines: Vec<String> = objects
        .iter()
        .filter(|o| o.kind != Kind::Index)
        .take(ASSISTANT_OBJECTS)
        .map(|o| {
            let text = if o.text.is_empty() {
                String::new()
            } else {
                format!(" - {}", o.text)
            };
            format!("{} ({}){text}", o.name, o.kind.singular())
        })
        .collect();
    if lines.is_empty() {
        lines.push(format!(
            "{library} has no tables, views, procedures or functions."
        ));
    } else if objects.len() > ASSISTANT_OBJECTS {
        lines.push(format!("… and {} more", objects.len() - ASSISTANT_OBJECTS));
    }
    lines.join("\n")
}

/// A table's columns for the assistant, a line each.
fn column_lines(library: &str, object: &Object, columns: &[ColumnInfo]) -> String {
    let mut lines = vec![format!(
        "{} ({}){}",
        catalog::qualified(library, &object.name),
        object.kind.singular(),
        if object.text.is_empty() {
            String::new()
        } else {
            format!(" - {}", object.text)
        }
    )];
    for c in columns {
        let null = if c.nullable { "" } else { " not null" };
        let mut about: Vec<&str> = Vec::new();
        for part in [c.heading.as_str(), c.text.as_str()] {
            if !part.is_empty() && !about.contains(&part) {
                about.push(part);
            }
        }
        let about = if about.is_empty() {
            String::new()
        } else {
            format!(" - {}", about.join(" / "))
        };
        lines.push(format!("{} {}{null}{about}", c.name, c.data_type));
    }
    lines.join("\n")
}
