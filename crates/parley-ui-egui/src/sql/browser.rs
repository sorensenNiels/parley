//! The schema browser (R-SQL-34): the SQL job's library list and the libraries pinned, each
//! opening to its tables, views, indexes, procedures and functions, a table to its columns.
//! Read through the second connection (D21), lazily, a level at a time.

use std::collections::BTreeMap;

use egui::{CollapsingHeader, RichText, ScrollArea, TextEdit, Ui};
use parley_core::sql::catalog::{self, ColumnInfo, Kind, Object};
use parley_core::sql::text;

use parley_core::sql::assistant::Lookup;
use parley_core::sql::complete::{self, Typed};

use super::database::{Answered, Ask, Database, State};
use super::editor::Item;
use super::snippets::field_row;
use crate::{icons, theme};

/// Something read, being read, or not readable.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Load<T> {
    Reading,
    Ready(T),
    Failed(String),
}

/// What the operator picked in the browser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pick {
    /// Text to put in the editor at the cursor.
    Insert(String),
    /// A new script tab, run at once if `run` (R-SQL-35).
    Script {
        title: String,
        text: String,
        run: bool,
    },
}

/// The browser's state.
#[derive(Debug, Default)]
pub struct Browser {
    /// Libraries pinned for this session, kept in the settings.
    pinned: Vec<String>,
    /// The library typed to pin.
    pin_field: String,
    pin_error: Option<String>,
    /// The library pinned last, shown open at once: pinned closed, it seemed to show nothing.
    just_pinned: Option<String>,
    query: String,
    /// Why the last DDL could not be read.
    error: Option<String>,
    objects: BTreeMap<String, Load<Vec<Object>>>,
    columns: BTreeMap<(String, String), Load<Vec<ColumnInfo>>>,
    /// The signatures of routines, by library and name (R-SQL-65).
    parameters: BTreeMap<(String, String), Load<Vec<catalog::Signature>>>,
}

impl Browser {
    pub fn with_pinned(pinned: Vec<String>) -> Self {
        Self {
            pinned,
            ..Self::default()
        }
    }

    /// The signatures of the routine `name` in `qualifier`, or else the current `schema`
    /// (R-SQL-65): read once per window, asked for the first time it is wanted; `None` while
    /// read, or when there is no such routine.
    pub fn signatures(
        &mut self,
        qualifier: Option<&str>,
        name: &str,
        schema: Option<&str>,
        db: &mut Database,
    ) -> Option<&[catalog::Signature]> {
        let library = qualifier.or(schema)?.to_owned();
        let key = (library.clone(), name.to_owned());
        if !self.parameters.contains_key(&key) {
            self.parameters.insert(key.clone(), Load::Reading);
            db.ask(Ask::Parameters {
                library,
                name: name.to_owned(),
            });
        }
        match self.parameters.get(&key) {
            Some(Load::Ready(found)) if !found.is_empty() => Some(found),
            Some(Load::Ready(_) | Load::Reading | Load::Failed(_)) | None => None,
        }
    }

    /// The libraries pinned, to keep.
    pub fn pinned(&self) -> &[String] {
        &self.pinned
    }

    /// Takes the catalog's answers; a DDL read opens a script.
    pub fn receive(&mut self, answers: Vec<Answered>) -> Vec<Pick> {
        let mut picks = Vec::new();
        for (ask, rows) in answers {
            match ask {
                Ask::Objects(library) => {
                    let load =
                        rows.map_or_else(Load::Failed, |r| Load::Ready(catalog::objects_from(&r)));
                    self.objects.insert(library, load);
                }
                Ask::Columns { library, table } => {
                    let load =
                        rows.map_or_else(Load::Failed, |r| Load::Ready(catalog::columns_from(&r)));
                    self.columns.insert((library, table), load);
                }
                Ask::Parameters { library, name } => {
                    let load = rows
                        .map_or_else(Load::Failed, |r| Load::Ready(catalog::signatures_from(&r)));
                    self.parameters.insert((library, name), load);
                }
                Ask::Ddl { title, .. } => match rows {
                    Ok(rows) => {
                        self.error = None;
                        picks.push(Pick::Script {
                            title,
                            text: catalog::ddl_from(&rows),
                            run: false,
                        });
                    }
                    Err(e) => self.error = Some(format!("No SQL for {title}: {e}")),
                },
            }
        }
        picks
    }

    /// The panel. `list` is the SQL job's library list: system name and kind.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        list: &[(String, String)],
        db: &mut Database,
    ) -> Option<Pick> {
        if let Some(why) = unusable(&db.state) {
            theme::warning(ui, why);
            return None;
        }
        if matches!(db.state, State::Started | State::Connecting) {
            // Said, so a library that stays at "Reading…" has a reason in view.
            theme::hint(
                ui,
                "The catalog job is signing on; libraries are read after that.",
            );
        }
        self.search_row(ui);
        if let Some(error) = &self.error {
            theme::error(ui, error);
        }
        let mut picked = None;
        ScrollArea::vertical()
            .id_salt("sql-browser")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // A long name is cut, not let widen the panel.
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                theme::section_label(ui, "Library list");
                if list.is_empty() {
                    theme::hint(ui, "Read after sign-on.");
                }
                for (library, kind) in list {
                    picked = self.library(ui, library, Some(kind), db).or(picked.take());
                }
                ui.add_space(6.0);
                theme::section_label(ui, "Pinned");
                let pinned = self.pinned.clone();
                if pinned.is_empty() {
                    theme::hint(ui, "Pin a library to keep it here for this session.");
                }
                for library in &pinned {
                    picked = self.library(ui, library, None, db).or(picked.take());
                }
                ui.add_space(6.0);
                self.pin_row(ui);
            });
        picked
    }

    fn search_row(&mut self, ui: &mut Ui) {
        let (_, again) = field_row(
            ui,
            Some(icons::MAGNIFYING_GLASS),
            TextEdit::singleline(&mut self.query).hint_text("Filter by name or text"),
            icons::ARROWS_CLOCKWISE,
            "Read the catalog again",
        );
        if again
            .on_hover_text("Read the open libraries and tables again")
            .clicked()
        {
            self.objects.clear();
            self.columns.clear();
        }
    }

    fn pin_row(&mut self, ui: &mut Ui) {
        let (field, add) = field_row(
            ui,
            None,
            TextEdit::singleline(&mut self.pin_field).hint_text("Library to pin"),
            icons::PLUS,
            "Pin library",
        );
        let entered = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if add.clicked() || entered {
            self.pin(&self.pin_field.clone());
        }
        if let Some(error) = &self.pin_error {
            theme::error(ui, error);
        }
    }

    fn pin(&mut self, typed: &str) {
        match text::system_name(typed) {
            Some(name) => {
                if !self.pinned.contains(&name) {
                    self.pinned.push(name.clone());
                }
                self.just_pinned = Some(name);
                self.pin_field.clear();
                self.pin_error = None;
            }
            None => self.pin_error = Some(format!("\"{}\" is not a library name.", typed.trim())),
        }
    }

    /// One library: opening it reads its objects.
    fn library(
        &mut self,
        ui: &mut Ui,
        library: &str,
        kind: Option<&str>,
        db: &mut Database,
    ) -> Option<Pick> {
        let mut picked = None;
        let header = CollapsingHeader::new(RichText::new(library))
            .id_salt(("sql-library", library, kind.is_some()))
            .default_open(kind.is_none() && self.just_pinned.as_deref() == Some(library))
            .show(ui, |ui| {
                let Some(load) = self.objects.get(library).cloned() else {
                    self.objects.insert(library.to_owned(), Load::Reading);
                    db.ask(Ask::Objects(library.to_owned()));
                    return;
                };
                match load {
                    Load::Reading => reading(ui),
                    Load::Failed(why) => theme::error(ui, why),
                    Load::Ready(objects) => picked = self.groups(ui, library, &objects, db),
                }
            });
        let response = match kind {
            Some(kind) => header.header_response.on_hover_text(kind),
            None => header.header_response,
        };
        if response.double_clicked() {
            picked = Some(Pick::Insert(catalog::identifier(library)));
        }
        response.context_menu(|ui| {
            if self.pinned.iter().any(|p| p == library) {
                if ui.button("Unpin").clicked() {
                    self.pinned.retain(|p| p != library);
                }
            } else if ui.button("Pin").clicked() {
                self.pin(library);
            }
        });
        picked
    }

    /// A library's objects, a group per kind, filtered by the search.
    fn groups(
        &mut self,
        ui: &mut Ui,
        library: &str,
        objects: &[Object],
        db: &mut Database,
    ) -> Option<Pick> {
        let query = self.query.trim().to_lowercase();
        let filtering = !query.is_empty();
        let mut picked = None;
        if objects.is_empty() {
            theme::hint(ui, "Nothing here.");
        }
        for kind in Kind::ALL {
            let shown: Vec<&Object> = objects
                .iter()
                .filter(|o| o.kind == kind && matches(o, &query))
                .collect();
            if shown.is_empty() {
                continue;
            }
            CollapsingHeader::new(format!("{} ({})", kind.plural(), shown.len()))
                .id_salt(("sql-kind", library, kind))
                .open(filtering.then_some(true))
                .show(ui, |ui| {
                    for object in shown {
                        if let Some(p) = self.object(ui, library, object, db) {
                            picked = Some(p);
                        }
                    }
                });
        }
        if objects.len() >= catalog::MAX_OBJECTS as usize {
            theme::hint(ui, format!("The first {} only.", catalog::MAX_OBJECTS));
        }
        picked
    }

    /// An object: a table or view opens to its columns; double-click inserts its name.
    fn object(
        &mut self,
        ui: &mut Ui,
        library: &str,
        object: &Object,
        db: &mut Database,
    ) -> Option<Pick> {
        let qualified = catalog::qualified(library, &object.name);
        let mut picked = None;
        let response = if object.kind.has_columns() {
            let key = (library.to_owned(), object.system_name.clone());
            CollapsingHeader::new(object.name.as_str())
                .id_salt(("sql-object", library, &object.system_name))
                .show(ui, |ui| {
                    let Some(load) = self.columns.get(&key).cloned() else {
                        self.columns.insert(key.clone(), Load::Reading);
                        db.ask(Ask::Columns {
                            library: key.0.clone(),
                            table: key.1.clone(),
                        });
                        return;
                    };
                    match load {
                        Load::Reading => reading(ui),
                        Load::Failed(why) => theme::error(ui, why),
                        Load::Ready(columns) => picked = column_list(ui, &columns),
                    }
                })
                .header_response
        } else {
            ui.selectable_label(false, object.name.as_str())
        };
        let hover = describe(object);
        let response = if hover.is_empty() {
            response
        } else {
            response.on_hover_text(hover)
        };
        if response.double_clicked() {
            picked = Some(Pick::Insert(qualified.clone()));
        }
        response.context_menu(|ui| {
            if ui.button("Insert name").clicked() {
                picked = Some(Pick::Insert(qualified.clone()));
                ui.close();
            }
            if ui.button("Show SQL").clicked() {
                db.ask(Ask::Ddl {
                    library: library.to_owned(),
                    name: object.system_name.clone(),
                    kind: object.kind,
                    title: qualified.clone(),
                });
                ui.close();
            }
            if object.kind.has_columns() && ui.button("Select first rows").clicked() {
                picked = Some(Pick::Script {
                    title: qualified.clone(),
                    text: catalog::first_rows(library, &object.name),
                    run: true,
                });
                ui.close();
            }
        });
        picked
    }
}

// ---------------------------------------------------------------- completion (R-SQL-36)

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
        let mut items = Vec::new();
        let prefix = typed.prefix.as_str();
        if let Some(qualifier) = &typed.qualifier {
            let table = tables.iter().find(|t| {
                t.alias.as_deref() == Some(qualifier)
                    || (t.alias.is_none() && t.table == *qualifier)
            });
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
        } else {
            for t in &tables {
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
            let libraries = list.iter().map(|(l, _)| l).chain(&self.pinned);
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

    /// The objects of `library` (asked for when not read), but its indexes.
    fn objects_of(&mut self, library: &str, db: &mut Database) -> Option<&[Object]> {
        text::system_name(library)?;
        if !self.objects.contains_key(library) {
            self.objects.insert(library.to_owned(), Load::Reading);
            db.ask(Ask::Objects(library.to_owned()));
        }
        match self.objects.get(library) {
            Some(Load::Ready(objects)) => Some(objects),
            Some(Load::Reading | Load::Failed(_)) | None => None,
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
        let key = (library.to_owned(), system.to_owned());
        if !self.columns.contains_key(&key) {
            self.columns.insert(key.clone(), Load::Reading);
            db.ask(Ask::Columns {
                library: key.0.clone(),
                table: key.1.clone(),
            });
        }
        match self.columns.get(&key) {
            Some(Load::Ready(columns)) => Some(columns),
            Some(Load::Reading | Load::Failed(_)) | None => None,
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

// ---------------------------------------------------------------- the assistant (R-SQL-44)

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
        if !self.objects.contains_key(&library) {
            self.objects.insert(library.clone(), Load::Reading);
            db.ask(Ask::Objects(library.clone()));
        }
        let objects = match self.objects.get(&library)? {
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
        let key = (library.clone(), object.system_name.clone());
        if !self.columns.contains_key(&key) {
            self.columns.insert(key.clone(), Load::Reading);
            db.ask(Ask::Columns {
                library: key.0.clone(),
                table: key.1.clone(),
            });
        }
        match self.columns.get(&key)? {
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

/// Whether `name` starts with the typed `prefix` (in capitals) and is not all of it.
fn starts(name: &str, prefix: &str) -> bool {
    let name = name.to_uppercase();
    name.starts_with(prefix) && name != prefix
}

/// The columns of a table: name and type; double-click inserts the name.
fn column_list(ui: &mut Ui, columns: &[ColumnInfo]) -> Option<Pick> {
    let mut picked = None;
    let muted = theme::palette(ui).muted;
    for column in columns {
        let response = ui
            .horizontal(|ui| {
                let name = ui.selectable_label(false, column.name.as_str());
                ui.label(RichText::new(&column.data_type).color(muted).small());
                name
            })
            .inner;
        let mut hover = Vec::new();
        for part in [&column.heading, &column.text] {
            if !part.is_empty() && !hover.contains(&part.as_str()) {
                hover.push(part.as_str());
            }
        }
        let nullable = if column.nullable {
            "null allowed"
        } else {
            "not null"
        };
        let hover = format!("{}\n{} {nullable}", hover.join("\n"), column.data_type);
        if response.on_hover_text(hover.trim()).double_clicked() {
            picked = Some(Pick::Insert(catalog::identifier(&column.name)));
        }
    }
    if columns.is_empty() {
        theme::hint(ui, "No columns.");
    }
    picked
}

fn reading(ui: &mut Ui) {
    ui.horizontal(|ui| {
        ui.spinner();
        theme::hint(ui, "Reading…");
    });
}

/// Why the browser cannot read the catalog, if it cannot.
fn unusable(state: &State) -> Option<String> {
    match state {
        State::Off => Some("Sign on to browse the database.".to_owned()),
        State::Failed(why) => Some(format!("No catalog job, so no schema browser: {why}")),
        State::Started | State::Connecting | State::Ready { .. } => None,
    }
}

/// An object's hover text: its system name when it differs, and its text.
fn describe(object: &Object) -> String {
    let mut lines = Vec::new();
    if object.system_name != object.name && !object.system_name.is_empty() {
        lines.push(format!("System name {}", object.system_name));
    }
    if !object.text.is_empty() {
        lines.push(object.text.clone());
    }
    lines.join("\n")
}

/// Whether `object` matches the lower-case `query` by name or text.
fn matches(object: &Object, query: &str) -> bool {
    query.is_empty()
        || object.name.to_lowercase().contains(query)
        || object.text.to_lowercase().contains(query)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn object(kind: Kind, name: &str, text: &str) -> Object {
        Object {
            kind,
            name: name.to_owned(),
            system_name: name.to_owned(),
            text: text.to_owned(),
        }
    }

    #[test]
    fn the_search_matches_name_or_text_r_sql_34() {
        let o = object(Kind::Table, "KUNDER", "Kunder i Århus");
        assert!(matches(&o, ""));
        assert!(matches(&o, "kund"));
        assert!(matches(&o, "århus"));
        assert!(!matches(&o, "ordre"));
    }

    #[test]
    fn only_a_library_name_can_be_pinned_and_once_r_sql_34() {
        let mut b = Browser::default();
        b.pin(" qgpl ");
        b.pin("QGPL");
        assert_eq!(b.pinned(), ["QGPL"]);
        assert_eq!(b.just_pinned.as_deref(), Some("QGPL"), "shown open at once");
        b.pin("not a name");
        assert!(b.pin_error.is_some());
        b.pin("ÆØÅ");
        assert_eq!(b.pinned().len(), 1);
    }

    #[test]
    fn answers_fill_the_tree_and_failures_are_kept_r_sql_34() {
        let mut b = Browser::default();
        b.receive(vec![
            (
                Ask::Objects("QGPL".to_owned()),
                Ok(vec![vec![
                    Some("T".to_owned()),
                    Some("KUNDER".to_owned()),
                    Some("KUNDER".to_owned()),
                    None,
                ]]),
            ),
            (
                Ask::Columns {
                    library: "QGPL".to_owned(),
                    table: "GONE".to_owned(),
                },
                Err("not found".to_owned()),
            ),
        ]);
        assert!(matches!(&b.objects["QGPL"], Load::Ready(o) if o.len() == 1));
        assert_eq!(
            b.columns[&("QGPL".to_owned(), "GONE".to_owned())],
            Load::Failed("not found".to_owned())
        );
    }

    #[test]
    fn the_database_tab_opens_a_library_to_its_tables_r_sql_34() {
        use egui_kittest::kittest::Queryable;

        use super::super::tests::{drawn, window};
        use super::super::tools::JobContext;

        let mut w = window();
        w.database.state = State::Ready {
            job: "1/QUSER/QZDASOINIT".to_owned(),
        };
        w.job = Some(JobContext {
            schema: "QGPL".to_owned(),
            libraries: vec![("QGPL".to_owned(), "USER".to_owned())],
        });
        w.browser.receive(vec![(
            Ask::Objects("QGPL".to_owned()),
            Ok(vec![vec![
                Some("T".to_owned()),
                Some("KUNDER".to_owned()),
                Some("KUNDER".to_owned()),
                Some("Kunder æøå".to_owned()),
            ]]),
        )]);
        let mut harness = drawn(w, Vec::new());
        harness.get_by_label("Database").click();
        harness.run();
        harness.get_by_label("QGPL").click();
        harness.run();
        harness.get_by_label("Tables (1)").click();
        harness.run();
        assert!(
            harness.query_by_label("KUNDER").is_some(),
            "the table is listed"
        );
        assert!(
            harness.query_by_label("PINNED").is_some(),
            "the pinned section"
        );
    }

    #[test]
    fn the_sql_of_an_object_opens_in_a_new_script_tab_r_sql_35() {
        use super::super::tests::window;

        let mut w = window();
        w.scripts.set_text("VALUES 1", 0);
        let ddl = |rows: Result<Vec<parley_core::sql::protocol::Row>, String>| {
            (
                Ask::Ddl {
                    library: "QGPL".to_owned(),
                    name: "KUNDER".to_owned(),
                    kind: Kind::Table,
                    title: "QGPL.KUNDER".to_owned(),
                },
                rows,
            )
        };
        let line = |t: &str| {
            vec![
                Some("1".to_owned()),
                Some("0".to_owned()),
                Some(t.to_owned()),
            ]
        };
        for pick in w.browser.receive(vec![ddl(Ok(vec![
            line("CREATE TABLE QGPL.KUNDER ("),
            line("  NAVN VARCHAR(50) -- æøå )"),
        ]))]) {
            w.use_pick(pick);
        }
        assert_eq!(
            w.scripts.text(),
            "CREATE TABLE QGPL.KUNDER (\n  NAVN VARCHAR(50) -- æøå )"
        );
        assert_eq!(w.browser.receive(vec![ddl(Err("SQL0204".to_owned()))]), []);
        assert!(
            w.browser
                .error
                .as_deref()
                .is_some_and(|e| e.contains("QGPL.KUNDER"))
        );
    }

    #[test]
    fn the_side_panel_keeps_its_width_on_every_tab_r_sql_34() {
        use egui_kittest::kittest::Queryable;

        use super::super::tests::{drawn, window};
        use super::super::tools::JobContext;

        for side in ["History", "Snippets", "Database"] {
            let mut w = window();
            w.database.state = State::Ready {
                job: "1/QUSER/QZDASOINIT".to_owned(),
            };
            w.job = Some(JobContext {
                schema: "QGPL".to_owned(),
                libraries: vec![("QGPL".to_owned(), "USER".to_owned())],
            });
            let long = "KUNDER_MED_ET_MEGET_LANGT_NAVN_ÆØÅ_DER_BLIVER_VED_OG_VED";
            w.browser.receive(vec![(
                Ask::Objects("QGPL".to_owned()),
                Ok(vec![vec![
                    Some("T".to_owned()),
                    Some(long.to_owned()),
                    Some("KUNDER".to_owned()),
                    None,
                ]]),
            )]);
            let mut harness = drawn(w, Vec::new());
            harness.get_by_label(side).click();
            harness.run_steps(2);
            if side == "Database" {
                harness.get_by_label("QGPL").click();
                harness.run_steps(2);
                harness.get_by_label("Tables (1)").click();
                harness.run_steps(2);
            }
            let before = harness.get_by_label(side).rect();
            harness.run_steps(4);
            let after = harness.get_by_label(side).rect();
            // The panel grew a few points each frame when a row guessed its width.
            assert!(
                (after.left() - before.left()).abs() < 0.5,
                "{side} stays put: {before:?} then {after:?}"
            );
        }
    }

    #[test]
    fn completion_offers_columns_tables_libraries_and_keywords_r_sql_36() {
        let mut b = Browser::with_pinned(vec!["KUNDELIB".to_owned()]);
        let cell = |t: &str| Some(t.to_owned());
        b.receive(vec![
            (
                Ask::Objects("QGPL".to_owned()),
                Ok(vec![
                    vec![cell("T"), cell("KUNDER"), cell("KUNDER"), None],
                    vec![cell("I"), cell("KUNDER_IX"), cell("KUNDE00001"), None],
                ]),
            ),
            (
                Ask::Columns {
                    library: "QGPL".to_owned(),
                    table: "KUNDER".to_owned(),
                },
                Ok(vec![
                    vec![
                        cell("NAVN"),
                        cell("VARCHAR"),
                        cell("50"),
                        None,
                        cell("N"),
                        cell("Navn æøå"),
                        None,
                    ],
                    vec![
                        cell("NUMMER"),
                        cell("DECIMAL"),
                        cell("7"),
                        cell("0"),
                        cell("N"),
                        None,
                        None,
                    ],
                ]),
            ),
        ]);
        let mut db = Database::default();
        let labels = |items: Vec<Item>| items.into_iter().map(|i| i.label).collect::<Vec<_>>();
        let typed = |text: &str| complete::typed_at(text, text.len()).expect("typed");

        let statement = "SELECT k.n FROM kunder k";
        let items = b.candidates(&typed("SELECT k.n"), statement, Some("QGPL"), &[], &mut db);
        assert_eq!(items[0].detail, "VARCHAR(50) · Navn æøå");
        assert_eq!(labels(items), ["NAVN", "NUMMER"]);

        let items = b.candidates(&typed("SELECT * FROM qgpl.k"), "", None, &[], &mut db);
        assert_eq!(labels(items), ["KUNDER"], "no indexes");

        let list = [("QSYS2".to_owned(), "SYSTEM".to_owned())];
        let items = b.candidates(
            &typed("SELECT ku"),
            "SELECT ku",
            Some("QGPL"),
            &list,
            &mut db,
        );
        assert_eq!(labels(items), ["KUNDER", "KUNDELIB"]);

        let items = b.candidates(&typed("SEL"), "SEL", None, &list, &mut db);
        assert!(labels(items).contains(&"SELECT".to_owned()));

        // A library not read yet is asked for, and offers nothing until answered.
        let items = b.candidates(&typed("SELECT * FROM other.x"), "", None, &[], &mut db);
        assert_eq!(items, []);
        assert_eq!(b.objects.get("OTHER"), Some(&Load::Reading));
    }

    #[test]
    fn hovering_a_table_shows_its_text_and_columns_r_sql_37() {
        let mut b = Browser::default();
        let cell = |t: &str| Some(t.to_owned());
        b.receive(vec![
            (
                Ask::Objects("QGPL".to_owned()),
                Ok(vec![vec![
                    cell("T"),
                    cell("KUNDER"),
                    cell("KUNDER"),
                    cell("Kunder æøå"),
                ]]),
            ),
            (
                Ask::Columns {
                    library: "QGPL".to_owned(),
                    table: "KUNDER".to_owned(),
                },
                Ok(vec![vec![
                    cell("NAVN"),
                    cell("VARCHAR"),
                    cell("50"),
                    None,
                    cell("N"),
                    cell("Navn"),
                    None,
                ]]),
            ),
        ]);
        let mut db = Database::default();
        let statement = "SELECT k.navn FROM qgpl.kunder k";
        let shown = b
            .describe(Some("QGPL"), "KUNDER", statement, None, &mut db)
            .expect("a table");
        assert_eq!(
            shown,
            "QGPL.KUNDER (table)\nKunder æøå\n\nNAVN  VARCHAR(50) · Navn"
        );
        assert_eq!(
            b.describe(None, "K", statement, None, &mut db),
            Some(shown),
            "an alias shows its table"
        );
        assert_eq!(
            b.describe(Some("K"), "NAVN", statement, None, &mut db),
            None,
            "a column"
        );
        assert_eq!(
            b.describe(None, "SELECT", statement, Some("QGPL"), &mut db),
            None
        );
        assert_eq!(
            b.describe(None, "KUNDER", "SELECT 1", None, &mut db),
            None,
            "no schema"
        );
    }

    #[test]
    fn the_assistant_gets_names_only_and_waits_while_reading_r_sql_44() {
        let mut b = Browser::default();
        let mut db = Database::default();
        let cell = |t: &str| Some(t.to_owned());
        let tables = Lookup::Tables {
            library: "qgpl".to_owned(),
        };
        assert_eq!(
            b.tool_answer(&tables, &mut db),
            None,
            "asked, then waited for"
        );
        b.receive(vec![
            (
                Ask::Objects("QGPL".to_owned()),
                Ok(vec![
                    vec![
                        cell("T"),
                        cell("KUNDER"),
                        cell("KUNDER"),
                        cell("Kunder æøå"),
                    ],
                    vec![cell("I"), cell("KUNDER_IX"), cell("KUNDE00001"), None],
                ]),
            ),
            (
                Ask::Columns {
                    library: "QGPL".to_owned(),
                    table: "KUNDER".to_owned(),
                },
                Ok(vec![vec![
                    cell("NAVN"),
                    cell("VARCHAR"),
                    cell("50"),
                    None,
                    cell("N"),
                    cell("Navn"),
                    None,
                ]]),
            ),
        ]);
        assert_eq!(
            b.tool_answer(&tables, &mut db).as_deref(),
            Some("KUNDER (table) - Kunder æøå")
        );
        let columns = Lookup::Columns {
            library: "QGPL".to_owned(),
            table: "kunder".to_owned(),
        };
        assert_eq!(
            b.tool_answer(&columns, &mut db).as_deref(),
            Some("QGPL.KUNDER (table) - Kunder æøå\nNAVN VARCHAR(50) not null - Navn")
        );
        let missing = Lookup::Columns {
            library: "QGPL".to_owned(),
            table: "ORDRE".to_owned(),
        };
        assert!(
            b.tool_answer(&missing, &mut db)
                .is_some_and(|t| t.starts_with("There is no table"))
        );
        let bad = Lookup::Tables {
            library: "not a lib".to_owned(),
        };
        assert!(
            b.tool_answer(&bad, &mut db)
                .is_some_and(|t| t.contains("not an IBM i library"))
        );
    }
}
