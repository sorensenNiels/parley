//! The schema browser (R-SQL-34), the side panel's Libraries (R-SQL-83): the SQL job's library
//! list and the operator's library groups, each library opening to its tables, views, indexes,
//! procedures and functions, a table to its columns. Read through the second connection (D21),
//! lazily, a level at a time.

mod assistant;
mod complete;
mod generate;
mod groups;

use std::collections::BTreeMap;

use egui::collapsing_header::CollapsingState;
use egui::{CollapsingHeader, Rect, RichText, ScrollArea, Sense, TextEdit, Ui};
use parley_sql_core::catalog::{self, ColumnInfo, Kind, Object};

use super::database::{Answered, Ask, Database, State};
use super::snippets::field_row;
use super::system::SystemPick;
use parley_sql_core::services::View;
use parley_ui_egui::{icons, theme};

pub use generate::{Backup, Chosen, Goal};
pub use groups::{LibraryGroup, from_pinned};

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
    /// Download every row of a table or view (R-SQL-71).
    Download { library: String, name: String },
    /// Upload a CSV file into a table (R-SQL-73).
    Upload { library: String, name: String },
    /// Something from the System node (R-SQL-78..80).
    System(SystemPick),
    /// A backup's SQL, ready for its rows (R-SQL-95, R-SQL-96).
    Backup(Backup),
    /// Generated SQL that could not be made: why, for Messages (R-SQL-93..96).
    Failed(String),
}

/// The root of the tree, the IBM i (R-SQL-83), and the System node's views with their labels
/// (R-SQL-78..80).
#[derive(Debug, Clone, Copy)]
pub struct Root<'a> {
    pub name: &'a str,
    /// Said on hover: the host and release.
    pub about: &'a str,
    pub views: &'a [(View, String)],
}

/// The browser's state.
#[derive(Debug)]
pub struct Browser {
    /// The operator's library groups for this connection, kept in the settings (R-SQL-83).
    groups: Vec<LibraryGroup>,
    /// Whether the "Library list" group is open.
    list_open: bool,
    /// Whether the System node is open (R-SQL-78).
    system_open: bool,
    /// The name dialog open: a new group, a new name, or a library to add.
    asking: Option<groups::Asking>,
    /// The group asked about before deleting.
    deleting: Option<usize>,
    /// The library added last, by group, shown open at once: added closed, it seemed to show
    /// nothing.
    just_added: Option<(usize, String)>,
    query: String,
    /// Why the last DDL could not be read.
    error: Option<String>,
    objects: BTreeMap<String, Load<Vec<Object>>>,
    columns: BTreeMap<(String, String), Load<Vec<ColumnInfo>>>,
    /// The signatures of routines, by library and name (R-SQL-65).
    parameters: BTreeMap<(String, String), Load<Vec<catalog::Signature>>>,
    /// The dialog of Generate SQL… or Back up as SQL… (R-SQL-93, R-SQL-96), and what it chose.
    generating: Option<generate::Asking>,
    chosen: Option<generate::Chosen>,
    /// Generated SQL being read, and the last batch's number (R-SQL-93..96).
    batches: Vec<generate::Batch>,
    serial: u64,
}

impl Default for Browser {
    fn default() -> Self {
        Self {
            groups: Vec::new(),
            list_open: true,
            system_open: false,
            asking: None,
            deleting: None,
            just_added: None,
            query: String::new(),
            error: None,
            objects: BTreeMap::new(),
            columns: BTreeMap::new(),
            parameters: BTreeMap::new(),
            generating: None,
            chosen: None,
            batches: Vec::new(),
            serial: 0,
        }
    }
}

impl Browser {
    /// The objects of `library` (system name): read, being read, or not; asked for the first
    /// time they are wanted.
    fn objects_load(&mut self, library: &str, db: &mut Database) -> &Load<Vec<Object>> {
        self.objects.entry(library.to_owned()).or_insert_with(|| {
            db.ask(Ask::Objects(library.to_owned()));
            Load::Reading
        })
    }

    /// The columns of the table or view `system` (system name) in `library`, as
    /// [`Self::objects_load`].
    fn columns_load(
        &mut self,
        library: &str,
        system: &str,
        db: &mut Database,
    ) -> &Load<Vec<ColumnInfo>> {
        self.columns
            .entry((library.to_owned(), system.to_owned()))
            .or_insert_with(|| {
                db.ask(Ask::Columns {
                    library: library.to_owned(),
                    table: system.to_owned(),
                });
                Load::Reading
            })
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

    /// Takes the catalog's answers; a DDL read opens a script, and generated SQL may ask more.
    pub fn receive(&mut self, answers: Vec<Answered>, db: &mut Database) -> Vec<Pick> {
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
                // The System node's, taken by `system` before the browser sees them.
                Ask::Service(_) | Ask::Text { .. } => {}
                Ask::Batch { batch, step, .. } => {
                    picks.extend(self.receive_batch(batch, step, rows, db));
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

    /// The tree, under `root`, the IBM i (R-SQL-83). `list` is the SQL job's library list:
    /// system name and kind.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        root: &Root<'_>,
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
                let (_, header, _) = group_header(ui, "sql-system", true, |ui| {
                    ui.add(
                        egui::Label::new(theme::semibold(root.name))
                            .truncate()
                            .sense(egui::Sense::click()),
                    )
                    .on_hover_text(root.about)
                })
                .body(|ui| {
                    picked = self.library_list(ui, list, db);
                    let mut rects = Vec::with_capacity(self.groups.len());
                    let mut dragging = None;
                    for i in 0..self.groups.len() {
                        let (pick, rect, dragged) = self.group(ui, i, db);
                        picked = pick.or(picked.take());
                        rects.push(rect);
                        if dragged {
                            dragging = Some(i);
                        }
                    }
                    self.drop_group(ui, dragging, &rects);
                    if self.groups.is_empty() {
                        ui.add_space(6.0);
                        theme::hint(
                            ui,
                            "Make a group, such as Development or Testing, with New group, to keep libraries at hand.",
                        );
                    }
                    picked = self.system_node(ui, root.views).or(picked.take());
                });
                header
                    .inner
                    .context_menu(|ui| picked = self.root_menu(ui, root.views).or(picked.take()));
            });
        picked
    }

    /// The IBM i's menu, on the root of the tree (R-SQL-83): the System views too (R-SQL-78).
    fn root_menu(&mut self, ui: &mut Ui, views: &[(View, String)]) -> Option<Pick> {
        let mut picked = None;
        if ui.button("New group…").clicked() {
            self.new_group();
            ui.close();
        }
        if ui.button("Read the catalog again").clicked() {
            self.objects.clear();
            self.columns.clear();
            ui.close();
        }
        ui.separator();
        for (view, _) in views {
            if ui.button(view.title()).clicked() {
                picked = Some(Pick::System(SystemPick::Show(*view)));
                ui.close();
            }
        }
        picked
    }

    /// The System node: the user's spooled files, message queue and jobs (R-SQL-78..80).
    fn system_node(&mut self, ui: &mut Ui, views: &[(View, String)]) -> Option<Pick> {
        let mut picked = None;
        let (_, header, _) = group_header(ui, "sql-system-node", self.system_open, |ui| {
            ui.add(
                egui::Label::new(theme::semibold("System"))
                    .truncate()
                    .sense(egui::Sense::click()),
            )
            .on_hover_text("Your spooled files, message queue and jobs on this IBM i")
        })
        .body(|ui| {
            for (view, label) in views {
                if ui.selectable_label(false, label.as_str()).clicked() {
                    picked = Some(Pick::System(SystemPick::Show(*view)));
                }
            }
        });
        let label = header.inner;
        let mut open = open_now(ui, "sql-system-node", self.system_open);
        if label.clicked() {
            open = !open;
        }
        label.context_menu(|ui| {
            if ui.button("Refresh").clicked() {
                picked = Some(Pick::System(SystemPick::Refresh));
                ui.close();
            }
            ui.separator();
            for (view, _) in views {
                if ui.button(view.title()).clicked() {
                    picked = Some(Pick::System(SystemPick::Show(*view)));
                    ui.close();
                }
            }
        });
        self.system_open = open;
        picked
    }

    /// Whether the System node is open, to keep in the settings and to read its lists.
    pub const fn system_open(&self) -> bool {
        self.system_open
    }

    /// The group "Library list": the SQL job's, which cannot be changed (R-SQL-83).
    fn library_list(
        &mut self,
        ui: &mut Ui,
        list: &[(String, String)],
        db: &mut Database,
    ) -> Option<Pick> {
        let mut picked = None;
        let open = self.list_open;
        group_header(ui, "sql-library-list", open, |ui| {
            ui.label(theme::semibold("Library list"))
                .on_hover_text("The SQL job's library list");
        })
        .body(|ui| {
            if list.is_empty() {
                theme::hint(ui, "Read after sign-on.");
            }
            for (library, kind) in list {
                picked = self
                    .library(ui, library, Some(kind), None, db)
                    .or(picked.take());
            }
        });
        self.list_open = open_now(ui, "sql-library-list", open);
        picked
    }

    /// Group `i`, with its menu (R-SQL-83).
    /// Group `i` with its libraries; also where it is drawn and whether its menu button is being
    /// dragged (R-SQL-104).
    fn group(&mut self, ui: &mut Ui, i: usize, db: &mut Database) -> (Option<Pick>, Rect, bool) {
        let Some(group) = self.groups.get(i) else {
            return (None, Rect::NOTHING, false);
        };
        let (name, libraries, open) = (group.name.clone(), group.libraries.clone(), group.open);
        let id = ("sql-library-group", i);
        let mut picked = None;
        let mut handle = None;
        let (_, header, body) = group_header(ui, id, open, |ui| {
            let label = ui.add(
                egui::Label::new(theme::semibold(&name))
                    .truncate()
                    .sense(egui::Sense::click()),
            );
            // The menu button is also the handle a group is dragged by (R-SQL-104): the drag is
            // sensed behind the button, so a click still opens the menu (a sense on top would
            // take the click).
            let menu = theme::button_row(ui, |ui| {
                let glyph = icons::icon(icons::LIST).color(theme::palette(ui).muted);
                // By name, unique to the group: by place, the drag would pass to the group that
                // took its place.
                let behind = egui::UiBuilder::new()
                    .id_salt(("sql-group-handle", &name))
                    .sense(Sense::drag());
                let shown = ui.scope_builder(behind, |ui| {
                    ui.menu_button(glyph, |ui| self.group_menu(ui, i)).response
                });
                handle = Some(shown.response);
                shown.inner
            });
            let menu = menu.on_hover_text("Menu; drag to move the group");
            menu.widget_info(|| {
                egui::WidgetInfo::labeled(
                    egui::accesskit::Role::Button,
                    true,
                    format!("{name} menu"),
                )
            });
            label
        })
        .body(|ui| {
            if libraries.is_empty() {
                theme::hint(ui, "Add a library from the group's menu.");
            }
            for library in &libraries {
                picked = self
                    .library(ui, library, None, Some(i), db)
                    .or(picked.take());
            }
        });
        let label = header.inner;
        let mut open = open_now(ui, id, open);
        if label.clicked() {
            open = !open;
        }
        label.context_menu(|ui| self.group_menu(ui, i));
        if let Some(group) = self.groups.get_mut(i) {
            group.open = open;
        }
        let rect = body.map_or(header.response.rect, |b| {
            header.response.rect.union(b.response.rect)
        });
        let dragged = handle.is_some_and(|h| {
            if h.dragged() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
            } else if h.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
            }
            h.dragged()
        });
        (picked, rect, dragged)
    }

    /// The group being dragged, `dragging`, takes the place of the group under the pointer, as
    /// it moves (R-SQL-104); `rects` are where the groups were drawn.
    fn drop_group(&mut self, ui: &Ui, dragging: Option<usize>, rects: &[Rect]) {
        if let (Some(from), Some(pointer)) = (dragging, ui.ctx().pointer_interact_pos())
            && let Some(to) = rects.iter().position(|r| r.y_range().contains(pointer.y))
            && to != from
        {
            self.move_group_to(from, to);
        }
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

    /// One library, in group `group` or in the library list (`None`): opening it reads its
    /// objects.
    fn library(
        &mut self,
        ui: &mut Ui,
        library: &str,
        kind: Option<&str>,
        group: Option<usize>,
        db: &mut Database,
    ) -> Option<Pick> {
        let mut picked = None;
        let just_added = group.is_some_and(|g| {
            self.just_added
                .as_ref()
                .is_some_and(|(i, l)| *i == g && l == library)
        });
        // By the group's name, which is its own (R-SQL-83), so a library stays open when its
        // group moves (R-SQL-104).
        let in_group = group
            .and_then(|g| self.groups.get(g))
            .map(|g| g.name.clone());
        let header = CollapsingHeader::new(RichText::new(library))
            .id_salt(("sql-library", library, in_group))
            .default_open(just_added)
            .show(ui, |ui| match self.objects_load(library, db).clone() {
                Load::Reading => reading(ui),
                Load::Failed(why) => theme::error(ui, why),
                Load::Ready(objects) => picked = self.groups(ui, library, &objects, db),
            });
        let response = match kind {
            Some(kind) => header.header_response.on_hover_text(kind),
            None => header.header_response,
        };
        if response.double_clicked() {
            picked = Some(Pick::Insert(catalog::identifier(library)));
        }
        response.context_menu(|ui| self.library_menu(ui, library, group));
        picked
    }

    /// A library's menu: its SQL or a backup (R-SQL-93, R-SQL-96); into another group, or out
    /// of this one (R-SQL-83).
    fn library_menu(&mut self, ui: &mut Ui, library: &str, group: Option<usize>) {
        if ui.button("Generate SQL…").clicked() {
            self.ask_generate(library, false);
            ui.close();
        }
        if ui.button("Back up as SQL…").clicked() {
            self.ask_generate(library, true);
            ui.close();
        }
        ui.separator();
        let others = self.groups_without(library);
        ui.add_enabled_ui(!others.is_empty(), |ui| {
            ui.menu_button("Add to group", |ui| {
                for (i, name) in others {
                    if ui.button(name).clicked() {
                        self.add_to(i, library);
                        ui.close();
                    }
                }
            });
        });
        if let Some(i) = group
            && ui.button("Remove from group").clicked()
        {
            self.remove_from(i, library);
            ui.close();
        }
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
        // R-SQL-127: as many as the settings say were read.
        let most = db.max_objects;
        if objects.len() >= usize::try_from(most).unwrap_or(usize::MAX) {
            theme::hint(ui, format!("The first {most} only."));
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
                    match self.columns_load(&key.0, &key.1, db).clone() {
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
            if object.kind == Kind::Table && ui.button("Show SQL with indexes and views").clicked()
            {
                self.generate_table(library, object, db);
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
            if object.kind.has_columns() && ui.button("Download…").clicked() {
                picked = Some(Pick::Download {
                    library: library.to_owned(),
                    name: object.name.clone(),
                });
                ui.close();
            }
            if object.kind == catalog::Kind::Table && ui.button("Upload CSV…").clicked() {
                picked = Some(Pick::Upload {
                    library: library.to_owned(),
                    name: object.name.clone(),
                });
                ui.close();
            }
        });
        picked
    }
}

/// A group's header row, open as `open` says; its caret may change that, which
/// [`open_now`] reads after the body.
pub(super) fn group_header<R>(
    ui: &mut Ui,
    id: impl std::hash::Hash + std::fmt::Debug,
    open: bool,
    add: impl FnOnce(&mut Ui) -> R,
) -> egui::collapsing_header::HeaderResponse<'_, R> {
    let id = ui.make_persistent_id(id);
    let mut state = CollapsingState::load_with_default_open(ui.ctx(), id, open);
    state.set_open(open);
    state.show_header(ui, add)
}

/// Whether the group `id` is open after this frame's clicks on its caret.
pub(super) fn open_now(ui: &Ui, id: impl std::hash::Hash + std::fmt::Debug, was: bool) -> bool {
    CollapsingState::load(ui.ctx(), ui.make_persistent_id(id)).map_or(was, |s| s.is_open())
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
        State::Failed(why) => Some(format!("No catalog job, so no Explorer: {why}")),
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
    use crate::editor::Item;
    use parley_sql_core::assistant::Lookup;

    fn object(kind: Kind, name: &str, text: &str) -> Object {
        Object {
            kind,
            name: name.to_owned(),
            system_name: name.to_owned(),
            text: text.to_owned(),
        }
    }

    #[test]
    fn library_names_come_from_the_library_list_then_the_groups_once_r_sql_115() {
        let group = LibraryGroup {
            name: "Mine".to_owned(),
            libraries: vec!["QSYS2".to_owned(), "MYLIB".to_owned(), "QÆØÅ".to_owned()],
            open: true,
        };
        let browser = Browser::with_groups(vec![group], false, false);
        let list = [
            ("QGPL".to_owned(), "SYSTEM".to_owned()),
            ("QSYS2".to_owned(), "SYSTEM".to_owned()),
        ];
        assert_eq!(browser.library_names("Q", &list), ["QGPL", "QSYS2", "QÆØÅ"]);
        assert_eq!(browser.library_names("", &list).len(), 4);
        assert_eq!(
            browser.library_names("QGPL", &list),
            ["QGPL"],
            "typed whole"
        );
        assert_eq!(browser.library_names("X", &list), Vec::<String>::new());
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
    fn answers_fill_the_tree_and_failures_are_kept_r_sql_34() {
        let mut b = Browser::default();
        b.receive(
            vec![
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
            ],
            &mut Database::default(),
        );
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
            server: "SYSÆØÅ".to_owned(),
        });
        w.browser.receive(
            vec![(
                Ask::Objects("QGPL".to_owned()),
                Ok(vec![vec![
                    Some("T".to_owned()),
                    Some("KUNDER".to_owned()),
                    Some("KUNDER".to_owned()),
                    Some("Kunder æøå".to_owned()),
                ]]),
            )],
            &mut Database::default(),
        );
        let mut harness = drawn(w, Vec::new());
        harness.get_by_label("QGPL").click();
        harness.run();
        harness.get_by_label("Tables (1)").click();
        harness.run();
        assert!(
            harness.query_by_label("KUNDER").is_some(),
            "the table is listed"
        );
        assert!(
            harness.query_by_label("SYSÆØÅ").is_some(),
            "R-SQL-83: the IBM i is the tree's root"
        );
    }

    #[test]
    fn the_sql_of_an_object_opens_in_a_new_script_tab_r_sql_35() {
        use super::super::tests::window;

        let mut w = window();
        w.scripts.set_text("VALUES 1", 0);
        let ddl = |rows: Result<Vec<parley_sql_core::protocol::Row>, String>| {
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
        for pick in w.browser.receive(
            vec![ddl(Ok(vec![
                line("CREATE TABLE QGPL.KUNDER ("),
                line("  NAVN VARCHAR(50) -- æøå )"),
            ]))],
            &mut Database::default(),
        ) {
            w.use_pick(pick);
        }
        assert_eq!(
            w.scripts.text(),
            "CREATE TABLE QGPL.KUNDER (\n  NAVN VARCHAR(50) -- æøå )"
        );
        assert_eq!(
            w.browser.receive(
                vec![ddl(Err("SQL0204".to_owned()))],
                &mut Database::default()
            ),
            []
        );
        assert!(
            w.browser
                .error
                .as_deref()
                .is_some_and(|e| e.contains("QGPL.KUNDER"))
        );
    }

    #[test]
    fn the_side_panel_keeps_its_width_with_a_long_name_open_r_sql_82() {
        use egui_kittest::kittest::Queryable;

        use super::super::tests::{drawn, window};
        use super::super::tools::JobContext;

        {
            let mut w = window();
            w.database.state = State::Ready {
                job: "1/QUSER/QZDASOINIT".to_owned(),
            };
            w.job = Some(JobContext {
                schema: "QGPL".to_owned(),
                libraries: vec![("QGPL".to_owned(), "USER".to_owned())],
                ..JobContext::default()
            });
            let long = "KUNDER_MED_ET_MEGET_LANGT_NAVN_ÆØÅ_DER_BLIVER_VED_OG_VED";
            w.browser.receive(
                vec![(
                    Ask::Objects("QGPL".to_owned()),
                    Ok(vec![vec![
                        Some("T".to_owned()),
                        Some(long.to_owned()),
                        Some("KUNDER".to_owned()),
                        None,
                    ]]),
                )],
                &mut Database::default(),
            );
            let mut harness = drawn(w, Vec::new());
            harness.get_by_label("QGPL").click();
            harness.run_steps(2);
            harness.get_by_label("Tables (1)").click();
            harness.run_steps(2);
            let edge = "Hide the side panel";
            let before = harness.get_by_label(edge).rect();
            harness.run_steps(4);
            let after = harness.get_by_label(edge).rect();
            // The panel grew a few points each frame when a row guessed its width.
            assert!(
                (after.right() - before.right()).abs() < 0.5,
                "the panel's edge stays put: {before:?} then {after:?}"
            );
        }
    }

    #[test]
    fn a_group_is_dragged_by_its_menu_button_which_still_opens_on_a_click_r_sql_104() {
        use egui_kittest::kittest::Queryable;

        use super::super::tests::{drawn, window};
        use super::groups::LibraryGroup;

        let mut w = window();
        w.database.state = State::Ready {
            job: "1/QUSER/QZDASOINIT".to_owned(),
        };
        let groups = ["Udvikling æøå", "Test"]
            .into_iter()
            .map(|name| LibraryGroup {
                name: name.to_owned(),
                ..LibraryGroup::default()
            })
            .collect();
        w.browser = Browser::with_groups(groups, true, false);
        let mut harness = drawn(w, Vec::new());
        let names = |h: &egui_kittest::Harness<'static, super::super::SqlWindow>| -> Vec<String> {
            h.state()
                .browser
                .library_groups()
                .iter()
                .map(|g| g.name.clone())
                .collect()
        };

        let from = harness.get_by_label("Udvikling æøå menu").rect().center();
        let to = harness.get_by_label("Test").rect().center();
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        harness.event(egui::Event::PointerMoved(from));
        harness.event(button(from, true));
        harness.run();
        for step in 1..=4 {
            #[expect(clippy::cast_precision_loss, reason = "a step of four")]
            let at = from + (to - from) * (step as f32 / 4.0);
            harness.event(egui::Event::PointerMoved(at));
            harness.run();
        }
        harness.event(button(to, false));
        harness.run();
        assert_eq!(names(&harness), ["Test", "Udvikling æøå"]);
        assert!(
            harness.query_by_label("Rename…").is_none(),
            "a drag opens no menu"
        );

        harness.get_by_label("Test menu").click();
        harness.run();
        assert!(
            harness.query_by_label("Rename…").is_some(),
            "a click opens the menu"
        );
        assert_eq!(names(&harness), ["Test", "Udvikling æøå"]);
    }

    #[test]
    fn completion_offers_a_withs_names_and_their_columns_r_sql_76() {
        let mut b = Browser::default();
        let mut db = Database::default();
        let labels = |items: Vec<Item>| items.into_iter().map(|i| i.label).collect::<Vec<_>>();
        let statement = "WITH t (kunde, beløb) AS (SELECT K, B FROM X) SELECT x. FROM t x";
        let typed = parley_sql_core::complete::typed_at("SELECT x.", 9).expect("typed");
        let items = b.candidates(&typed, statement, None, &[], &mut db);
        assert_eq!(labels(items), ["KUNDE", "BELØB"]);
        let typed = parley_sql_core::complete::typed_at("SELECT k", 8).expect("typed");
        let items = b.candidates(&typed, statement, None, &[], &mut db);
        assert_eq!(
            items.first().map(|i| i.detail.as_str()),
            Some("column of T")
        );
        assert_eq!(labels(items).first().map(String::as_str), Some("KUNDE"));
        let statement = "WITH totaler (k) AS (VALUES 1) SELECT * FROM TO";
        let typed = parley_sql_core::complete::typed_at(statement, statement.len()).expect("typed");
        let items = b.candidates(&typed, statement, None, &[], &mut db);
        assert!(
            items
                .iter()
                .any(|i| i.label == "TOTALER" && i.detail == "WITH")
        );
    }

    #[test]
    fn completion_offers_columns_tables_libraries_and_keywords_r_sql_36() {
        let mut b = Browser::with_groups(from_pinned(vec!["KUNDELIB".to_owned()]), true, false);
        let cell = |t: &str| Some(t.to_owned());
        b.receive(
            vec![
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
            ],
            &mut Database::default(),
        );
        let mut db = Database::default();
        let labels = |items: Vec<Item>| items.into_iter().map(|i| i.label).collect::<Vec<_>>();
        let typed =
            |text: &str| parley_sql_core::complete::typed_at(text, text.len()).expect("typed");

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
        b.receive(
            vec![
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
            ],
            &mut Database::default(),
        );
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
        b.receive(
            vec![
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
            ],
            &mut Database::default(),
        );
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
