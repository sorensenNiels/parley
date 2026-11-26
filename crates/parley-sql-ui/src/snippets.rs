//! The SQL window's snippets (R-SQL-24..27): the Snippets panel beside History, saving and
//! editing a snippet, and the form that asks for its placeholders. The files are the app's
//! ([`SnippetStore`]); the starters come with Parley.

use egui::{RichText, ScrollArea, TextEdit, Ui};
use parley_sql_core::snippet::{self, STARTERS, Snippet, ValueType};

use super::browser;

mod finder;
mod values;

use parley_ui_egui::{dialog, icons, theme};
use values::Asking;
pub use values::{RecentValues, Report, Wanted};

/// The snippet folders, in the app (R-SQL-24). Folders are given as text.
pub trait SnippetStore: std::fmt::Debug {
    /// The operator's own folder, beside the settings.
    fn mine(&self) -> Option<String>;
    /// The `.sql` files of `folder`: (file name without `.sql`, text); a missing folder is empty.
    fn list(&self, folder: &str) -> Result<Vec<(String, String)>, String>;
    /// Writes `name`.sql in `folder`, making the folder if needed.
    fn save(&self, folder: &str, name: &str, text: &str) -> Result<(), String>;
    fn delete(&self, folder: &str, name: &str) -> Result<(), String>;
}

/// Where a snippet is kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Mine,
    Shared,
    /// Comes with Parley; read-only (R-SQL-27).
    Starter,
}

impl Source {
    const fn heading(self) -> &'static str {
        match self {
            Self::Mine => "Mine",
            Self::Shared => "Shared",
            Self::Starter => "Did you know",
        }
    }
}

/// One snippet in the panel.
#[derive(Debug, Clone)]
struct Entry {
    source: Source,
    /// Its file name, without `.sql`.
    file: String,
    snippet: Snippet,
}

/// A snippet dragged from the panel, by its place there (R-SQL-118).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dragged(pub usize);

/// What the window does with a snippet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Use {
    /// Put this SQL in the editor at the cursor.
    Insert(String),
    /// Run this SQL.
    Run(String),
}

/// The save or edit dialog's fields.
#[derive(Debug, Clone)]
struct Editing {
    /// The snippet edited (its source and file); `None` for a new one.
    original: Option<(Source, String)>,
    /// The dialog's title: "New snippet", "Save as snippet" or "Edit snippet".
    heading: &'static str,
    title: String,
    tags: String,
    about: String,
    sql: String,
    shared: bool,
    error: Option<String>,
}

/// What a snippet's SQL asks for when used (R-SQL-108): "Asks for: user (text, QSYSOPR), days".
fn asks_for(sql: &str) -> String {
    let names: Vec<String> = snippet::placeholders(sql)
        .into_iter()
        .map(|p| {
            let about: Vec<&str> = [p.kind.word(), p.default.as_deref().unwrap_or_default()]
                .into_iter()
                .filter(|s| !s.is_empty())
                .collect();
            if about.is_empty() {
                p.name
            } else {
                format!("{} ({})", p.name, about.join(", "))
            }
        })
        .collect();
    if names.is_empty() {
        "Asks for no values. Write ${name} or ${name:default} where a value is asked for when the snippet is used.".to_owned()
    } else {
        format!("Asks for: {}", names.join(", "))
    }
}

/// The mistakes in a snippet's placeholders (R-SQL-110): unknown types, and text or dates
/// quoted in the SQL as well as by Parley.
fn placeholder_mistakes(sql: &str) -> Vec<String> {
    let unknown = snippet::placeholders(sql)
        .into_iter()
        .filter_map(|p| match p.kind {
            ValueType::Unknown(word) => Some(format!(
                "{}: \"{word}\" is not a type; use text, number, date, name, list, library or table.",
                p.name
            )),
            ValueType::Any
            | ValueType::Text
            | ValueType::Number
            | ValueType::Date
            | ValueType::Name
            | ValueType::List
            | ValueType::Library
            | ValueType::Table => None,
        });
    let quoted = snippet::quoted_twice(sql)
        .into_iter()
        .map(|name| format!("{name}: Parley writes its quotes; remove the ' around it."));
    unknown.chain(quoted).collect()
}

/// The values written in the SQL, each with "Ask for it", which makes it a placeholder
/// (R-SQL-111). A fixed height: the list grows and shrinks as the SQL is typed.
fn literal_offers(ui: &mut Ui, sql: &mut String) {
    theme::section_label(ui, "Values in the SQL");
    let found = snippet::literals(sql);
    let mut asked = None;
    ScrollArea::vertical()
        .id_salt("sql-snippet-literals")
        .max_height(84.0)
        .min_scrolled_height(84.0)
        .show(ui, |ui| {
            if found.is_empty() {
                theme::hint(
                    ui,
                    "None to ask for: no text or numbers written in the SQL.",
                );
            }
            for (i, literal) in found.iter().enumerate() {
                ui.horizontal(|ui| {
                    let button = ui
                        .add(theme::ghost_button("Ask for it"))
                        .on_hover_text(literal.placeholder());
                    if button.clicked() {
                        asked = Some(i);
                    }
                    ui.label(RichText::new(&literal.written).monospace());
                    theme::hint(ui, format!("as {} ({})", literal.name, literal.kind.word()));
                });
            }
        });
    if let Some(literal) = asked.and_then(|i| found.get(i)) {
        *sql = snippet::ask_for(sql, literal);
    }
}

/// The snippets panel's state.
#[derive(Debug, Default)]
pub struct SnippetPanel {
    entries: Vec<Entry>,
    /// Why a folder could not be read.
    errors: Vec<String>,
    loaded: bool,
    pub(super) query: String,
    editing: Option<Editing>,
    asking: Option<Asking>,
    /// The entry asked about before deleting.
    deleting: Option<usize>,
    /// SQL run from the editor asks for its `${…}` values first, as the settings say (R-SQL-120).
    pub(super) ask_in_editor: bool,
    /// Run snippet, while open (R-SQL-116).
    finding: Option<finder::Finder>,
    /// What was run last from a snippet, until the window takes it (R-SQL-117).
    report: Option<Report>,
    /// Placeholder values used on this connection, by name, newest first (R-SQL-26,
    /// R-SQL-113).
    remembered: RecentValues,
}

impl SnippetPanel {
    /// Reads the snippets again: the starters, mine, and the shared folder when there is one.
    pub fn load(&mut self, store: Option<&dyn SnippetStore>, shared: &str) {
        self.entries.clear();
        self.errors.clear();
        if let Some(store) = store {
            let folders = [
                (Source::Mine, store.mine()),
                (
                    Source::Shared,
                    Some(shared.trim().to_owned()).filter(|f| !f.is_empty()),
                ),
            ];
            for (source, folder) in folders {
                let Some(folder) = folder else {
                    continue;
                };
                match store.list(&folder) {
                    Ok(files) => self
                        .entries
                        .extend(files.into_iter().map(|(file, text)| Entry {
                            source,
                            snippet: Snippet::parse(&text, &file),
                            file,
                        })),
                    Err(e) => self.errors.push(format!("{}: {e}", source.heading())),
                }
            }
        }
        self.entries
            .extend(STARTERS.iter().map(|(file, text)| Entry {
                source: Source::Starter,
                file: (*file).to_owned(),
                snippet: Snippet::parse(text, file),
            }));
        self.entries.sort_by(|a, b| {
            a.snippet
                .title
                .to_lowercase()
                .cmp(&b.snippet.title.to_lowercase())
        });
        self.loaded = true;
    }

    /// Opens "Save as snippet…" with `sql` (R-SQL-25).
    pub fn save_as(&mut self, sql: &str) {
        self.open_new("Save as snippet", sql);
    }

    /// Opens "New snippet…" with `sql`, the editor's selection or nothing (R-SQL-106).
    pub fn new_snippet(&mut self, sql: &str) {
        self.open_new("New snippet", sql);
    }

    /// The dialog for a new snippet of `sql`, its title and tags suggested (R-SQL-119).
    fn open_new(&mut self, heading: &'static str, sql: &str) {
        let (title, tags) = snippet::suggest(sql);
        self.editing = Some(Editing {
            original: None,
            heading,
            title,
            tags: tags.join(", "),
            about: String::new(),
            sql: sql.trim().to_owned(),
            shared: false,
            error: None,
        });
    }

    /// The panel: search, then the snippets by where they are kept.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        store: Option<&dyn SnippetStore>,
        shared: &str,
    ) -> Option<Use> {
        if !self.loaded {
            self.load(store, shared);
        }
        let (_, again) = field_row(
            ui,
            Some(icons::MAGNIFYING_GLASS),
            TextEdit::singleline(&mut self.query).hint_text("Search snippets"),
            icons::ARROWS_CLOCKWISE,
            "Read snippets again",
        );
        if again.on_hover_text("Read the folders again").clicked() {
            self.load(store, shared);
        }
        for error in &self.errors {
            theme::error(ui, error);
        }
        let mut chosen = None;
        let searching = !self.query.trim().is_empty();
        // R-SQL-106: how to make one's own, until there is one.
        let none_mine = !self.entries.iter().any(|e| e.source == Source::Mine);
        if store.is_some() && none_mine && !searching {
            theme::hint(
                ui,
                "Make your own: + above, or right-click SQL in the editor and Save as snippet.",
            );
        }
        ScrollArea::vertical()
            .id_salt("sql-snippets")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for source in [Source::Mine, Source::Shared, Source::Starter] {
                    let shown: Vec<usize> = self
                        .entries
                        .iter()
                        .enumerate()
                        .filter(|(_, e)| e.source == source && e.snippet.matches(&self.query))
                        .map(|(i, _)| i)
                        .collect();
                    if shown.is_empty() {
                        continue;
                    }
                    if let Some(act) = self.group(ui, source, &shown, searching) {
                        chosen = Some(act);
                    }
                }
            });
        match chosen? {
            Item::Use(i, run) => self.start(i, run),
            Item::Edit(i) => {
                self.edit(i);
                None
            }
            Item::Delete(i) => {
                self.deleting = Some(i);
                None
            }
        }
    }

    /// The snippets of `source` in `shown`, as a group that folds like a library group
    /// (R-SQL-88); open while searching, so that every match shows.
    fn group(&self, ui: &mut Ui, source: Source, shown: &[usize], searching: bool) -> Option<Item> {
        let id = ("sql-snippet-group", source.heading());
        let open = searching || browser::open_now(ui, id, true);
        let mut act = None;
        let (_, header, _) = browser::group_header(ui, id, open, |ui| {
            let label = ui.add(
                egui::Label::new(theme::semibold(source.heading()))
                    .truncate()
                    .sense(egui::Sense::click()),
            );
            ui.label(RichText::new(shown.len().to_string()).color(theme::palette(ui).muted));
            label
        })
        .body(|ui| {
            for &i in shown {
                if let Some(item) = self.item(ui, i) {
                    act = Some(item);
                }
            }
        });
        if header.inner.clicked() {
            let id = ui.make_persistent_id(id);
            let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(
                ui.ctx(),
                id,
                open,
            );
            state.set_open(!open);
            state.store(ui.ctx());
        }
        act
    }

    /// One snippet on one line (R-SQL-88): its title, its tags on the right, and while the
    /// pointer is on it a Run button in their place. Double-click inserts it; the hover shows
    /// its about and SQL; the right-click menu has Insert, Run, Edit and Delete.
    fn item(&self, ui: &mut Ui, i: usize) -> Option<Item> {
        let entry = self.entries.get(i)?;
        let title = &entry.snippet.title;
        let p = theme::palette(ui);
        let size = egui::vec2(ui.available_width(), ui.spacing().interact_size.y);
        let (rect, row) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());
        row.widget_info(|| {
            egui::WidgetInfo::labeled(egui::accesskit::Role::Button, true, title.as_str())
        });
        // R-SQL-118: dragged onto the editor, it goes in where it is dropped.
        row.dnd_set_drag_payload(Dragged(i));
        if row.dragged() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        }
        let pointed = ui.rect_contains_pointer(rect);
        if pointed {
            let fill = ui.visuals().widgets.hovered.weak_bg_fill;
            ui.painter()
                .rect_filled(rect, ui.visuals().widgets.hovered.corner_radius, fill);
        }
        let mut act = None;
        let inner = rect.shrink2(egui::vec2(4.0, 0.0));
        let mut line = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(inner)
                .layout(egui::Layout::right_to_left(egui::Align::Center)),
        );
        if pointed {
            let run = icons::button(icons::PLAY, "").small();
            if icons::add_named(&mut line, true, run, &format!("Run {title}"))
                .on_hover_text("Run")
                .clicked()
            {
                act = Some(Item::Use(i, true));
            }
        } else if !entry.snippet.tags.is_empty() {
            line.scope(|ui| {
                ui.set_max_width(inner.width() * 0.4);
                ui.add(
                    egui::Label::new(
                        RichText::new(entry.snippet.tags.join(" · "))
                            .small()
                            .color(p.muted),
                    )
                    .truncate()
                    .selectable(false),
                );
            });
        }
        line.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
            ui.add(
                egui::Label::new(title.as_str())
                    .truncate()
                    .selectable(false),
            );
        });
        let row = row.on_hover_ui(|ui| about(ui, &entry.snippet));
        if row.double_clicked() {
            act = Some(Item::Use(i, false));
        }
        row.context_menu(|ui| {
            let mut pick = |ui: &mut Ui, name: &str, item: Item| {
                if ui.button(name).clicked() {
                    act = Some(item);
                    ui.close();
                }
            };
            pick(ui, "Insert", Item::Use(i, false));
            pick(ui, "Run", Item::Use(i, true));
            if entry.source != Source::Starter {
                ui.separator();
                pick(ui, "Edit…", Item::Edit(i));
                pick(ui, "Delete…", Item::Delete(i));
            }
        });
        act
    }

    fn edit(&mut self, i: usize) {
        let Some(entry) = self.entries.get(i) else {
            return;
        };
        self.editing = Some(Editing {
            original: Some((entry.source, entry.file.clone())),
            heading: "Edit snippet",
            title: entry.snippet.title.clone(),
            tags: entry.snippet.tags.join(", "),
            about: entry.snippet.about.clone(),
            sql: entry.snippet.sql.clone(),
            shared: entry.source == Source::Shared,
            error: None,
        });
    }

    /// The snippets whose title or a tag has a word starting with `prefix` (in capitals), for
    /// the completion list (R-SQL-64); all of them for an empty prefix.
    pub fn completions(&self, prefix: &str) -> Vec<super::editor::Item> {
        self.entries
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                std::iter::once(&e.snippet.title)
                    .chain(&e.snippet.tags)
                    .flat_map(|t| t.split(|c: char| !c.is_alphanumeric() && c != '_'))
                    .any(|word| word.to_uppercase().starts_with(prefix))
            })
            .map(|(i, e)| super::editor::Item {
                label: e.snippet.title.clone(),
                detail: "snippet".to_owned(),
                insert: String::new(),
                snippet: Some(i),
            })
            .collect()
    }

    /// The panel's dialogs, drawn over the SQL window: save or edit, placeholders, delete. The
    /// snippets are read first if they were not, for the completion list (R-SQL-64).
    pub fn dialogs(
        &mut self,
        ctx: &egui::Context,
        store: Option<&dyn SnippetStore>,
        shared: &str,
    ) -> Option<Use> {
        if !self.loaded {
            self.load(store, shared);
        }
        self.edit_dialog(ctx, store, shared);
        self.delete_dialog(ctx, store, shared);
        // A snippet chosen in Run snippet opens the values form when it has placeholders.
        let found = self.finder_dialog(ctx);
        found.or_else(|| self.placeholder_dialog(ctx))
    }

    fn edit_dialog(&mut self, ctx: &egui::Context, store: Option<&dyn SnippetStore>, shared: &str) {
        let Some(mut form) = self.editing.take() else {
            return;
        };
        let has_shared = !shared.trim().is_empty();
        let (close, (cancel, save)) = dialog::dialog(
            ctx,
            "sql-snippet",
            form.heading,
            560.0,
            |ui| {
                egui::Grid::new("sql-snippet-fields")
                .num_columns(2)
                .spacing([12.0, 8.0])
                .show(ui, |ui| {
                    let label = ui.label("Title");
                    theme::sized_edit(ui, 400.0, TextEdit::singleline(&mut form.title))
                        .labelled_by(label.id);
                    ui.end_row();
                    let label = ui.label("Tags");
                    theme::sized_edit(
                        ui,
                        400.0,
                        TextEdit::singleline(&mut form.tags)
                            .hint_text("did you know, work management"),
                    )
                    .labelled_by(label.id);
                    ui.end_row();
                    let label = ui.label("About");
                    theme::sized_edit(ui, 400.0, TextEdit::singleline(&mut form.about))
                        .labelled_by(label.id);
                    ui.end_row();
                    if form.original.is_none() {
                        let label = ui.label("Kept in");
                        ui.add_enabled_ui(has_shared, |ui| {
                            let mut place = form.shared;
                            theme::segmented(ui, &mut place, &[(false, "Mine"), (true, "Shared")]);
                            form.shared = place;
                        })
                        .response
                        .labelled_by(label.id)
                        .on_disabled_hover_text(
                            "Name a shared folder in the settings, under Add-ons, to share snippets",
                        );
                        ui.end_row();
                    }
                });
                ui.add_space(6.0);
                let sql_label = ui.label("SQL");
                ScrollArea::vertical()
                    .id_salt("sql-snippet-sql")
                    .max_height(200.0)
                    .min_scrolled_height(200.0)
                    .show(ui, |ui| {
                        ui.add(
                            TextEdit::multiline(&mut form.sql)
                                .code_editor()
                                .desired_width(f32::INFINITY)
                                .desired_rows(8),
                        )
                        .labelled_by(sql_label.id);
                    });
                // R-SQL-108: what the snippet will ask for, as it is typed.
                theme::hint(ui, asks_for(&form.sql));
                for mistake in placeholder_mistakes(&form.sql) {
                    theme::warning(ui, mistake);
                }
                ui.add_space(6.0);
                literal_offers(ui, &mut form.sql);
                if let Some(error) = &form.error {
                    theme::error(ui, error);
                }
                let ready =
                    !form.title.trim().is_empty() && !form.sql.trim().is_empty() && store.is_some();
                dialog::actions(ui, theme::primary_button(ui, "Save"), ready)
            },
        );
        if close || cancel {
            return;
        }
        if save {
            match self.write(&form, store, shared) {
                Ok(()) => {
                    self.load(store, shared);
                    return;
                }
                Err(e) => form.error = Some(e),
            }
        }
        self.editing = Some(form);
    }

    /// Writes the form's snippet: a new file named after its title, or the file edited.
    fn write(
        &self,
        form: &Editing,
        store: Option<&dyn SnippetStore>,
        shared: &str,
    ) -> Result<(), String> {
        let store = store.ok_or("Parley cannot keep snippets here")?;
        let source = form.original.as_ref().map_or(
            if form.shared {
                Source::Shared
            } else {
                Source::Mine
            },
            |(s, _)| *s,
        );
        let folder = match source {
            Source::Mine => store
                .mine()
                .ok_or("There is no folder for snippets on this system")?,
            Source::Shared => shared.trim().to_owned(),
            Source::Starter => {
                return Err("A snippet that comes with Parley cannot be changed".to_owned());
            }
        };
        let snippet = Snippet {
            title: form.title.trim().to_owned(),
            tags: form
                .tags
                .split(',')
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .map(str::to_owned)
                .collect(),
            about: form.about.trim().to_owned(),
            needs: String::new(),
            sql: form.sql.trim().to_owned(),
        };
        let file = if let Some((_, file)) = &form.original {
            file.clone()
        } else {
            let base = file_name(&snippet.title);
            let taken = |name: &str| {
                self.entries
                    .iter()
                    .any(|e| e.source == source && e.file.eq_ignore_ascii_case(name))
            };
            if taken(&base) {
                return Err(format!(
                    "A snippet named {base} is already kept there: choose another title"
                ));
            }
            base
        };
        store.save(&folder, &file, &snippet.to_text())
    }

    fn delete_dialog(
        &mut self,
        ctx: &egui::Context,
        store: Option<&dyn SnippetStore>,
        shared: &str,
    ) {
        let Some(i) = self.deleting else {
            return;
        };
        let Some(entry) = self.entries.get(i).cloned() else {
            self.deleting = None;
            return;
        };
        let (close, (cancel, delete)) = dialog::dialog(
            ctx,
            "sql-snippet-delete",
            "Delete the snippet?",
            400.0,
            |ui| {
                ui.label(format!(
                    "{} is deleted from {}.",
                    entry.snippet.title,
                    entry.source.heading()
                ));
                dialog::actions(ui, theme::danger_button(ui, "Delete"), true)
            },
        );
        if close || cancel {
            self.deleting = None;
        }
        if delete {
            self.deleting = None;
            let folder = match entry.source {
                Source::Mine => store.and_then(SnippetStore::mine),
                Source::Shared => Some(shared.trim().to_owned()),
                Source::Starter => None,
            };
            if let (Some(store), Some(folder)) = (store, folder)
                && let Err(e) = store.delete(&folder, &entry.file)
            {
                self.errors
                    .push(format!("{} not deleted: {e}", entry.snippet.title));
                return;
            }
            self.load(store, shared);
        }
    }
}

/// What was clicked on a snippet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Item {
    /// Insert (false) or run (true) snippet `.0`.
    Use(usize, bool),
    Edit(usize),
    Delete(usize),
}

/// A snippet's hover: what it does, what it needs, and its SQL.
fn about(ui: &mut Ui, snippet: &Snippet) {
    ui.set_max_width(420.0);
    ui.label(theme::semibold(&snippet.title));
    if !snippet.about.is_empty() {
        ui.label(&snippet.about);
    }
    if !snippet.needs.is_empty() {
        theme::hint(ui, format!("Needs {}", snippet.needs));
    }
    ui.add_space(4.0);
    ui.label(RichText::new(&snippet.sql).monospace().small());
    theme::hint(
        ui,
        "Double-click to put it in the editor; right-click for more.",
    );
}

/// A file name from a title: letters, digits, `-` and `_`; blanks become `-`.
fn file_name(title: &str) -> String {
    let name: String = title
        .trim()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let name = name
        .split('-')
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if name.is_empty() {
        "snippet".to_owned()
    } else {
        name
    }
}

/// A text field filling the side panel's width, after an optional icon and before a small icon
/// button named `name` for screen readers. The field is sized from what is left once the button
/// is placed: a width guessed beforehand made the panel grow a little every frame.
pub(super) fn field_row(
    ui: &mut Ui,
    leading: Option<&str>,
    field: TextEdit<'_>,
    button: &str,
    name: &str,
) -> (egui::Response, egui::Response) {
    ui.horizontal(|ui| {
        if let Some(icon) = leading {
            ui.label(icons::icon(icon).color(theme::palette(ui).muted));
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let clicked = icons::add_named(ui, true, icons::button(button, "").small(), name);
            let width = ui.available_width().max(40.0);
            (theme::sized_edit(ui, width, field), clicked)
        })
        .inner
    })
    .inner
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::BTreeMap;

    use super::*;

    /// Snippet folders in memory: folder → (file → text).
    #[derive(Debug, Default)]
    struct Folders(RefCell<BTreeMap<String, BTreeMap<String, String>>>);

    impl SnippetStore for Folders {
        fn mine(&self) -> Option<String> {
            Some("mine".to_owned())
        }
        fn list(&self, folder: &str) -> Result<Vec<(String, String)>, String> {
            if folder == "broken" {
                return Err("cannot be read".to_owned());
            }
            Ok(self
                .0
                .borrow()
                .get(folder)
                .map(|f| f.iter().map(|(n, t)| (n.clone(), t.clone())).collect())
                .unwrap_or_default())
        }
        fn save(&self, folder: &str, name: &str, text: &str) -> Result<(), String> {
            self.0
                .borrow_mut()
                .entry(folder.to_owned())
                .or_default()
                .insert(name.to_owned(), text.to_owned());
            Ok(())
        }
        fn delete(&self, folder: &str, name: &str) -> Result<(), String> {
            self.0
                .borrow_mut()
                .get_mut(folder)
                .and_then(|f| f.remove(name))
                .map(drop)
                .ok_or_else(|| "not there".to_owned())
        }
    }

    fn form(title: &str, shared: bool) -> Editing {
        Editing {
            original: None,
            heading: "Save as snippet",
            title: title.to_owned(),
            tags: "did you know, æøå".to_owned(),
            about: "About.".to_owned(),
            sql: "VALUES 1".to_owned(),
            shared,
            error: None,
        }
    }

    #[test]
    fn the_starters_are_there_without_a_store_r_sql_27() {
        let mut panel = SnippetPanel::default();
        panel.load(None, "");
        assert_eq!(panel.entries.len(), STARTERS.len());
        assert!(panel.entries.iter().all(|e| e.source == Source::Starter));
    }

    #[test]
    fn a_snippet_saved_is_a_file_named_by_its_title_and_listed_as_mine_r_sql_24() {
        let store = Folders::default();
        let mut panel = SnippetPanel::default();
        panel.load(Some(&store), "");
        panel
            .write(&form("Låste objekter?", false), Some(&store), "")
            .expect("saved");
        let text = store.0.borrow()["mine"]["Låste-objekter"].clone();
        assert!(
            text.starts_with("-- title: Låste objekter?\n-- tags: did you know, æøå\n"),
            "{text}"
        );
        panel.load(Some(&store), "");
        assert!(
            panel
                .entries
                .iter()
                .any(|e| e.source == Source::Mine && e.snippet.title == "Låste objekter?")
        );
        let again = panel.write(&form("Låste objekter?", false), Some(&store), "");
        assert!(
            again.expect_err("taken").contains("already kept"),
            "the same name is not overwritten"
        );
    }

    #[test]
    fn a_shared_folder_holds_shared_snippets_and_a_broken_one_is_named_r_sql_24() {
        let store = Folders::default();
        let mut panel = SnippetPanel::default();
        panel
            .write(&form("Team query", true), Some(&store), "//server/team")
            .expect("saved");
        panel.load(Some(&store), "//server/team");
        assert!(
            panel
                .entries
                .iter()
                .any(|e| e.source == Source::Shared && e.snippet.title == "Team query")
        );
        panel.load(Some(&store), "broken");
        assert_eq!(panel.errors, ["Shared: cannot be read"]);
    }

    #[test]
    fn a_starter_cannot_be_written_r_sql_27() {
        let store = Folders::default();
        let panel = SnippetPanel::default();
        let mut f = form("x", false);
        f.original = Some((Source::Starter, "system-status".to_owned()));
        assert!(panel.write(&f, Some(&store), "").is_err());
    }

    #[test]
    fn placeholders_ask_with_defaults_then_remember_the_values_r_sql_26() {
        let mut panel = SnippetPanel::default();
        panel.load(None, "");
        let locks = panel
            .entries
            .iter()
            .position(|e| e.file == "largest-objects")
            .expect("a starter");
        assert_eq!(panel.start(locks, true), None, "asks first");
        let asking = panel.asking.clone().expect("the form");
        assert_eq!(
            asking.fields,
            [
                ("library".to_owned(), "QGPL".to_owned()),
                ("rows".to_owned(), "20".to_owned())
            ]
        );
        panel.remembered.insert(
            "library".to_owned(),
            vec!["MSGSUDTA".to_owned(), "QGPL".to_owned()],
        );
        panel.asking = None;
        panel.start(locks, false);
        assert_eq!(
            panel.asking.expect("the form").fields[0].1,
            "MSGSUDTA",
            "the last value used comes first"
        );
    }

    #[test]
    fn a_snippet_without_placeholders_is_used_at_once_r_sql_25() {
        let mut panel = SnippetPanel::default();
        panel.load(None, "");
        let status = panel
            .entries
            .iter()
            .position(|e| e.file == "system-status")
            .expect("a starter");
        assert_eq!(
            panel.start(status, true),
            Some(Use::Run(
                "SELECT * FROM QSYS2.SYSTEM_STATUS_INFO".to_owned()
            ))
        );
    }

    /// The panel's dialogs drawn alone, with the snippet folders in memory.
    fn drawn(
        panel: SnippetPanel,
    ) -> egui_kittest::Harness<'static, (SnippetPanel, Folders, Option<Use>)> {
        egui_kittest::Harness::builder()
            .with_size(egui::vec2(900.0, 700.0))
            .with_max_steps(16)
            .build_ui_state(
                |ui, state: &mut (SnippetPanel, Folders, Option<Use>)| {
                    parley_ui_egui::test_support::install_fonts(ui);
                    let (panel, store, used) = state;
                    if let Some(u) = panel.dialogs(ui.ctx(), Some(&*store), "") {
                        *used = Some(u);
                    }
                },
                (panel, Folders::default(), None),
            )
    }

    #[test]
    fn save_as_snippet_keeps_its_buttons_in_place_and_writes_the_file_r_sql_25() {
        use egui_kittest::kittest::Queryable;
        let mut panel = SnippetPanel::default();
        panel.save_as("  SELECT * FROM QSYS2.SYSTEM_STATUS_INFO  ");
        let mut harness = drawn(panel);
        harness.run();
        let save = harness.get_by_label("Save").rect();
        harness.run_steps(5);
        assert_eq!(harness.get_by_label("Save").rect(), save);
        // The suggested title (R-SQL-119) gives way to one typed.
        harness
            .state_mut()
            .0
            .editing
            .as_mut()
            .expect("the dialog")
            .title
            .clear();
        harness.run();
        harness.get_by_label("Title").focus();
        harness.get_by_label("Title").type_text("Status æøå");
        harness.run();
        harness.get_by_label("Save").click();
        harness.run();
        let state = harness.state();
        assert!(state.0.editing.is_none(), "closed once saved");
        let text = state.1.0.borrow()["mine"]["Status-æøå"].clone();
        assert!(
            text.ends_with("SELECT * FROM QSYS2.SYSTEM_STATUS_INFO\n"),
            "{text}"
        );
    }

    #[test]
    fn delete_the_snippet_keeps_its_buttons_in_place_and_cancel_keeps_it_r_sql_25() {
        use egui_kittest::kittest::Queryable;
        let mut panel = SnippetPanel::default();
        panel.load(None, "");
        assert!(!panel.entries.is_empty(), "the starters");
        panel.deleting = Some(0);
        let mut harness = drawn(panel);
        harness.run();
        harness.get_by_label("Delete the snippet?");
        crate::test_support::keeps_its_place(&mut harness, "Delete");
        harness.get_by_label("Cancel").click();
        harness.run();
        assert!(harness.state().0.deleting.is_none());
    }

    #[test]
    fn the_values_form_keeps_its_buttons_in_place_and_fills_the_sql_r_sql_26() {
        use egui_kittest::kittest::Queryable;
        let panel = SnippetPanel {
            asking: Some(Asking {
                sql: "SELECT * FROM T WHERE L = '${library:QGPL}'".to_owned(),
                fields: vec![("library".to_owned(), "QGPL".to_owned())],
                run: true,
                ..Asking::default()
            }),
            ..SnippetPanel::default()
        };
        let mut harness = drawn(panel);
        harness.run();
        let run = harness.get_by_label("Run").rect();
        harness.run_steps(5);
        assert_eq!(harness.get_by_label("Run").rect(), run);
        harness.get_by_label("Run").click();
        harness.run();
        let state = harness.state();
        assert_eq!(
            state.2,
            Some(Use::Run("SELECT * FROM T WHERE L = 'QGPL'".to_owned()))
        );
        assert_eq!(
            state.0.remembered.get("library").map(Vec::as_slice),
            Some(["QGPL".to_owned()].as_slice())
        );
    }

    #[test]
    fn the_dialog_says_which_values_a_snippet_asks_for_r_sql_108() {
        assert_eq!(
            asks_for(
                "SELECT * FROM T WHERE U = '${user:QSYSOPR}' AND D > ${days} OR U = '${user}'"
            ),
            "Asks for: user (QSYSOPR), days"
        );
        assert_eq!(
            asks_for("SELECT 'æøå' FROM T WHERE X = '${navn:æøå}'"),
            "Asks for: navn (æøå)"
        );
        assert!(asks_for("SELECT 1 FROM SYSIBM.SYSDUMMY1").starts_with("Asks for no values."));
        assert!(
            asks_for("SELECT '${' FROM T").starts_with("Asks for no values."),
            "not closed"
        );
    }

    #[test]
    fn the_values_form_shows_the_filled_sql_and_can_insert_instead_of_run_r_sql_109() {
        use egui_kittest::kittest::Queryable;
        let panel = SnippetPanel {
            asking: Some(Asking {
                sql: "SELECT * FROM T WHERE L = '${library:QGPL}'".to_owned(),
                fields: vec![("library".to_owned(), "QGPL".to_owned())],
                run: true,
                ..Asking::default()
            }),
            ..SnippetPanel::default()
        };
        let mut harness = drawn(panel);
        harness.run();
        harness.get_by_label("SELECT * FROM T WHERE L = 'QGPL'");
        harness.get_by_label("library").focus();
        harness.get_by_label("library").type_text("æøå");
        harness.run();
        harness.get_by_label("SELECT * FROM T WHERE L = 'QGPLæøå'");
        crate::test_support::keeps_its_place(&mut harness, "Insert");
        harness.get_by_label("Insert").click();
        harness.run();
        assert_eq!(
            harness.state().2,
            Some(Use::Insert(
                "SELECT * FROM T WHERE L = 'QGPLæøå'".to_owned()
            ))
        );
    }

    #[test]
    fn a_new_snippet_starts_from_the_text_given_and_keeps_its_buttons_in_place_r_sql_106() {
        use egui_kittest::kittest::Queryable;
        let mut panel = SnippetPanel::default();
        panel.new_snippet("");
        let mut harness = drawn(panel);
        harness.run();
        harness.get_by_label("New snippet");
        crate::test_support::keeps_its_place(&mut harness, "Save");
        harness.get_by_label("SQL").focus();
        harness.get_by_label("SQL").type_text("SELECT '${a:1}'");
        harness.run();
        harness.get_by_label("Asks for: a (1)");
    }

    #[test]
    fn typed_placeholders_say_their_type_and_mistakes_are_named_r_sql_110() {
        assert_eq!(
            asks_for("WHERE U = ${user as text:QSYSOPR} AND N > ${n as number} AND X = ${x}"),
            "Asks for: user (text, QSYSOPR), n (number), x"
        );
        assert_eq!(
            placeholder_mistakes(
                "WHERE U = '${user as text}' AND N = ${n as nummer:1} AND D = '${d}'"
            ),
            [
                "n: \"nummer\" is not a type; use text, number, date, name, list, library or table.",
                "user: Parley writes its quotes; remove the ' around it.",
            ]
        );
        assert_eq!(
            placeholder_mistakes("WHERE U = ${user as text}"),
            Vec::<String>::new()
        );
    }

    #[test]
    fn a_value_that_does_not_fit_its_type_waits_to_be_put_right_r_sql_110() {
        use egui_kittest::kittest::Queryable;
        let panel = SnippetPanel {
            asking: Some(Asking {
                sql: "SELECT * FROM T WHERE N > ${days as number} AND U = ${user as text}"
                    .to_owned(),
                fields: vec![
                    ("days".to_owned(), "syv".to_owned()),
                    ("user".to_owned(), "O'Brien".to_owned()),
                ],
                run: true,
                ..Asking::default()
            }),
            ..SnippetPanel::default()
        };
        let mut harness = drawn(panel);
        harness.run();
        harness.get_by_label("days: not a number");
        harness.get_by_label("Run").click();
        harness.run();
        assert_eq!(harness.state().2, None, "Run waits");
        if let Some(asking) = harness.state_mut().0.asking.as_mut() {
            asking.fields[0].1 = "7,5".to_owned();
        }
        harness.run();
        assert!(harness.query_by_label("days: not a number").is_none());
        harness.get_by_label("Run").click();
        harness.run();
        assert_eq!(
            harness.state().2,
            Some(Use::Run(
                "SELECT * FROM T WHERE N > 7.5 AND U = 'O''Brien'".to_owned()
            ))
        );
    }

    #[test]
    fn asking_for_a_value_written_in_the_sql_makes_it_a_placeholder_r_sql_111() {
        use egui_kittest::kittest::Queryable;
        let mut panel = SnippetPanel::default();
        panel.new_snippet("SELECT * FROM QSYS2.USER_INFO WHERE USER_NAME = 'QSYSOPR'");
        let mut harness = drawn(panel);
        harness.run();
        harness.get_by_label("'QSYSOPR'");
        crate::test_support::keeps_its_place(&mut harness, "Save");
        harness.get_by_label("Ask for it").click();
        harness.run();
        assert_eq!(
            harness.state().0.editing.as_ref().map(|f| f.sql.as_str()),
            Some("SELECT * FROM QSYS2.USER_INFO WHERE USER_NAME = ${user_name as text:QSYSOPR}")
        );
        harness.get_by_label("Asks for: user_name (text, QSYSOPR)");
        harness.get_by_label("None to ask for: no text or numbers written in the SQL.");
    }

    #[test]
    fn the_last_five_values_are_kept_newest_first_but_not_passwords_r_sql_113() {
        let mut panel = SnippetPanel::default();
        for v in ["A", "B", "C", "A", "D", "E", "æøå", "", "  "] {
            panel.remember("lib", v);
        }
        panel.remember("Password", "hemmelig");
        panel.remember("user_pwd", "x");
        panel.remember("api secret", "x");
        let kept: Vec<(&str, Vec<&str>)> = panel
            .recent()
            .iter()
            .map(|(k, v)| (k.as_str(), v.iter().map(String::as_str).collect()))
            .collect();
        assert_eq!(kept, [("lib", vec!["æøå", "E", "D", "A", "C"])]);
    }

    #[test]
    fn the_values_form_offers_the_values_used_before_r_sql_113() {
        use egui_kittest::kittest::Queryable;
        let mut panel = SnippetPanel::default();
        panel.set_recent(RecentValues::from([(
            "library".to_owned(),
            vec!["MSGSUDTA".to_owned(), "QGPL".to_owned()],
        )]));
        panel.asking = Some(Asking {
            sql: "SELECT * FROM T WHERE L = ${library as text}".to_owned(),
            fields: vec![("library".to_owned(), "MSGSUDTA".to_owned())],
            run: true,
            ..Asking::default()
        });
        let mut harness = drawn(panel);
        harness.run();
        crate::test_support::keeps_its_place(&mut harness, "Run");
        harness
            .get_by_label(parley_ui_egui::icons::CARET_DOWN)
            .click();
        harness.run();
        harness.get_by_label("QGPL").click();
        harness.run();
        harness.get_by_label("Run").click();
        harness.run();
        assert_eq!(
            harness.state().2,
            Some(Use::Run("SELECT * FROM T WHERE L = 'QGPL'".to_owned()))
        );
        let kept = harness.state().0.recent().get("library").cloned();
        assert_eq!(kept, Some(vec!["QGPL".to_owned(), "MSGSUDTA".to_owned()]));
    }

    #[test]
    fn a_library_or_table_field_wants_names_and_a_table_looks_in_the_library_typed_r_sql_115() {
        let mut asking = Asking {
            sql: "SELECT * FROM ${lib as library}.${t as table} WHERE X = ${x as text}".to_owned(),
            fields: vec![
                ("lib".to_owned(), "qsys2".to_owned()),
                ("t".to_owned(), " sys".to_owned()),
                ("x".to_owned(), String::new()),
            ],
            ..Asking::default()
        };
        assert_eq!(asking.wanted(), None, "no field had the keyboard");
        asking.focused = Some("lib".to_owned());
        assert_eq!(
            asking.wanted(),
            Some(Wanted::Libraries {
                prefix: "QSYS2".to_owned()
            })
        );
        asking.focused = Some("t".to_owned());
        assert_eq!(
            asking.wanted(),
            Some(Wanted::Tables {
                library: Some("QSYS2".to_owned()),
                prefix: "SYS".to_owned()
            })
        );
        asking.fields[0].1 = "1 not a name".to_owned();
        assert_eq!(
            asking.wanted(),
            Some(Wanted::Tables {
                library: None,
                prefix: "SYS".to_owned()
            }),
            "the job's current library then"
        );
        asking.focused = Some("x".to_owned());
        assert_eq!(asking.wanted(), None);
    }

    #[test]
    fn a_name_offered_is_put_in_the_field_last_typed_in_r_sql_115() {
        use egui_kittest::kittest::Queryable;
        let mut panel = SnippetPanel {
            asking: Some(Asking {
                sql: "SELECT * FROM ${lib as library}.T".to_owned(),
                fields: vec![("lib".to_owned(), "Q".to_owned())],
                run: true,
                focused: Some("lib".to_owned()),
                ..Asking::default()
            }),
            ..SnippetPanel::default()
        };
        panel.offer_names(vec!["QGPL".to_owned(), "QSYS2".to_owned()]);
        let mut harness = drawn(panel);
        harness.run();
        crate::test_support::keeps_its_place(&mut harness, "Run");
        harness.get_by_label("QSYS2").click();
        harness.run();
        harness.get_by_label("Run").click();
        harness.run();
        assert_eq!(
            harness.state().2,
            Some(Use::Run("SELECT * FROM QSYS2.T".to_owned()))
        );
    }

    #[test]
    fn a_new_snippet_starts_with_a_title_and_tags_from_its_sql_r_sql_119() {
        let mut panel = SnippetPanel::default();
        panel.save_as("SELECT * FROM QSYS2.ACTIVE_JOB_INFO");
        let form = panel.editing.clone().expect("the dialog");
        assert_eq!(
            (form.title.as_str(), form.tags.as_str()),
            ("Active job info", "qsys2")
        );
        panel.new_snippet("");
        let form = panel.editing.expect("the dialog");
        assert_eq!((form.title.as_str(), form.tags.as_str()), ("", ""));
    }

    #[test]
    fn a_snippet_run_says_its_title_with_the_values_and_an_insert_says_nothing_r_sql_117() {
        use egui_kittest::kittest::Queryable;
        let mut panel = SnippetPanel::default();
        panel.load(None, "");
        let status = panel
            .entries
            .iter()
            .position(|e| e.snippet.title == "System status")
            .expect("a starter");
        assert!(matches!(panel.start(status, false), Some(Use::Insert(_))));
        assert_eq!(panel.take_report(), None, "inserted");
        assert!(matches!(panel.start(status, true), Some(Use::Run(_))));
        let report = panel.take_report().expect("run");
        assert_eq!(report.title, "System status");
        assert_eq!(panel.take_report(), None, "taken once");

        panel.asking = Some(Asking {
            sql: "SELECT * FROM T WHERE L = ${library as text} AND N = ${n}".to_owned(),
            fields: vec![
                ("library".to_owned(), "QGPL".to_owned()),
                ("n".to_owned(), String::new()),
            ],
            run: true,
            snippet: "Objects".to_owned(),
            ..Asking::default()
        });
        let mut harness = drawn(panel);
        harness.run();
        harness.get_by_label("Run").click();
        harness.run();
        let report = harness.state_mut().0.take_report().expect("run");
        assert_eq!(report.title, "Objects — QGPL");
        assert_eq!(
            report.sql, "SELECT * FROM T WHERE L = ${library as text} AND N = ${n}",
            "placeholders kept to run again"
        );
        let again = harness.state_mut().0.run_again(&report);
        assert_eq!(again, None, "the values are asked for again");
        let asking = harness.state().0.asking.clone().expect("the form");
        assert_eq!(asking.fields[0].1, "QGPL", "the value last used");
    }

    #[test]
    fn the_panel_says_how_to_make_a_snippet_until_there_is_one_r_sql_106() {
        use egui_kittest::kittest::Queryable;
        const HINT: &str =
            "Make your own: + above, or right-click SQL in the editor and Save as snippet.";
        let mut harness = egui_kittest::Harness::builder()
            .with_size(egui::vec2(320.0, 600.0))
            .build_ui_state(
                |ui, (panel, store): &mut (SnippetPanel, Folders)| {
                    parley_ui_egui::test_support::install_fonts(ui);
                    panel.show(ui, Some(&*store), "");
                },
                (SnippetPanel::default(), Folders::default()),
            );
        harness.run();
        harness.get_by_label(HINT);
        let (panel, store) = harness.state_mut();
        store
            .save("mine", "mine", "-- title: Mine\nSELECT 1")
            .expect("in memory");
        panel.load(Some(&*store), "");
        harness.run();
        assert!(harness.query_by_label(HINT).is_none());
    }

    #[test]
    fn file_names_keep_letters_and_digits_r_sql_24() {
        assert_eq!(
            file_name("Who has an object locked?"),
            "Who-has-an-object-locked"
        );
        assert_eq!(file_name("  æøå / ÆØÅ  "), "æøå-ÆØÅ");
        assert_eq!(file_name("***"), "snippet");
    }
}
