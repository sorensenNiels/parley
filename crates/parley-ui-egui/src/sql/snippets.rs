//! The SQL window's snippets (R-SQL-24..27): the Snippets panel beside History, saving and
//! editing a snippet, and the form that asks for its placeholders. The files are the app's
//! ([`SnippetStore`]); the starters come with Parley.

use std::collections::BTreeMap;

use egui::{RichText, ScrollArea, TextEdit, Ui};
use parley_core::sql::snippet::{self, STARTERS, Snippet};

use crate::{dialog, icons, theme};

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
    title: String,
    tags: String,
    about: String,
    sql: String,
    shared: bool,
    error: Option<String>,
}

/// The placeholder form.
#[derive(Debug, Clone)]
struct Asking {
    sql: String,
    fields: Vec<(String, String)>,
    run: bool,
}

/// The snippets panel's state.
#[derive(Debug, Default)]
pub struct SnippetPanel {
    entries: Vec<Entry>,
    /// Why a folder could not be read.
    errors: Vec<String>,
    loaded: bool,
    query: String,
    editing: Option<Editing>,
    asking: Option<Asking>,
    /// The entry asked about before deleting.
    deleting: Option<usize>,
    /// Placeholder values typed in this run of Parley, by name (R-SQL-26).
    remembered: BTreeMap<String, String>,
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
        self.editing = Some(Editing {
            original: None,
            title: String::new(),
            tags: String::new(),
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
                    ui.add_space(6.0);
                    theme::section_label(ui, source.heading());
                    for i in shown {
                        if let Some(act) = self.item(ui, i) {
                            chosen = Some(act);
                        }
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

    /// One snippet: its title (double-click inserts it), its tags, and Run, Edit and Delete.
    fn item(&self, ui: &mut Ui, i: usize) -> Option<Item> {
        let entry = self.entries.get(i)?;
        let p = theme::palette(ui);
        let mut act = None;
        ui.horizontal(|ui| {
            let title = egui::Button::new(RichText::new(&entry.snippet.title))
                .frame_when_inactive(false)
                .truncate();
            let width = (ui.available_width() - 70.0).max(60.0);
            let response = ui
                .add_sized([width, 20.0], title)
                .on_hover_ui(|ui| about(ui, &entry.snippet));
            if response.double_clicked() {
                act = Some(Item::Use(i, false));
            }
            let run = icons::button(icons::PLAY, "").small();
            if icons::add_named(ui, true, run, &format!("Run {}", entry.snippet.title))
                .on_hover_text("Run")
                .clicked()
            {
                act = Some(Item::Use(i, true));
            }
            if entry.source != Source::Starter {
                let edit = icons::button(icons::PENCIL, "").small();
                if icons::add_named(ui, true, edit, &format!("Edit {}", entry.snippet.title))
                    .on_hover_text("Edit")
                    .clicked()
                {
                    act = Some(Item::Edit(i));
                }
                if ui
                    .add(theme::icon_button(ui, "🗙"))
                    .on_hover_text("Delete")
                    .clicked()
                {
                    act = Some(Item::Delete(i));
                }
            }
        });
        if !entry.snippet.tags.is_empty() {
            ui.label(
                RichText::new(entry.snippet.tags.join(" · "))
                    .small()
                    .color(p.muted),
            );
        }
        act
    }

    /// Uses snippet `i`: at once, or after the placeholder form (R-SQL-26).
    pub fn start(&mut self, i: usize, run: bool) -> Option<Use> {
        let sql = self.entries.get(i)?.snippet.sql.clone();
        let placeholders = snippet::placeholders(&sql);
        if placeholders.is_empty() {
            return Some(if run { Use::Run(sql) } else { Use::Insert(sql) });
        }
        let fields = placeholders
            .into_iter()
            .map(|p| {
                let value = self
                    .remembered
                    .get(&p.name)
                    .cloned()
                    .or(p.default)
                    .unwrap_or_default();
                (p.name, value)
            })
            .collect();
        self.asking = Some(Asking { sql, fields, run });
        None
    }

    fn edit(&mut self, i: usize) {
        let Some(entry) = self.entries.get(i) else {
            return;
        };
        self.editing = Some(Editing {
            original: Some((entry.source, entry.file.clone())),
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
        self.placeholder_dialog(ctx)
    }

    fn edit_dialog(&mut self, ctx: &egui::Context, store: Option<&dyn SnippetStore>, shared: &str) {
        let Some(mut form) = self.editing.take() else {
            return;
        };
        let has_shared = !shared.trim().is_empty();
        let title = if form.original.is_some() {
            "Edit snippet"
        } else {
            "Save as snippet"
        };
        let (close, (cancel, save)) = dialog::dialog(ctx, "sql-snippet", title, 560.0, |ui| {
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
            theme::hint(
                ui,
                "Write ${name} or ${name:default} where a value is asked for when the snippet is used.",
            );
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
            if let Some(error) = &form.error {
                theme::error(ui, error);
            }
            let ready =
                !form.title.trim().is_empty() && !form.sql.trim().is_empty() && store.is_some();
            dialog::actions(ui, theme::primary_button(ui, "Save"), ready)
        });
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

    /// R-SQL-26: a field per placeholder; Insert or Run puts the values in.
    fn placeholder_dialog(&mut self, ctx: &egui::Context) -> Option<Use> {
        let mut asking = self.asking.take()?;
        let action = if asking.run { "Run" } else { "Insert" };
        let (close, (cancel, go)) = dialog::dialog(
            ctx,
            "sql-snippet-values",
            "Values for the snippet",
            440.0,
            |ui| {
                egui::Grid::new("sql-snippet-values")
                    .num_columns(2)
                    .spacing([12.0, 8.0])
                    .show(ui, |ui| {
                        for (name, value) in &mut asking.fields {
                            let label = ui.label(name.as_str());
                            theme::sized_edit(ui, 260.0, TextEdit::singleline(value))
                                .labelled_by(label.id);
                            ui.end_row();
                        }
                    });
                theme::hint(ui, "The values go into the SQL as typed.");
                dialog::actions(ui, theme::primary_button(ui, action), true)
            },
        );
        if close || cancel {
            return None;
        }
        if !go {
            self.asking = Some(asking);
            return None;
        }
        for (name, value) in &asking.fields {
            self.remembered.insert(name.clone(), value.clone());
        }
        let values: BTreeMap<&str, &str> = asking
            .fields
            .iter()
            .map(|(n, v)| (n.as_str(), v.as_str()))
            .collect();
        let sql = snippet::fill(&asking.sql, |name| {
            values.get(name).map(|v| (*v).to_owned())
        });
        Some(if asking.run {
            Use::Run(sql)
        } else {
            Use::Insert(sql)
        })
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
    theme::hint(ui, "Double-click to put it in the editor.");
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
        panel
            .remembered
            .insert("library".to_owned(), "MSGSUDTA".to_owned());
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
                    crate::test_support::install_fonts(ui);
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
    fn the_values_form_keeps_its_buttons_in_place_and_fills_the_sql_r_sql_26() {
        use egui_kittest::kittest::Queryable;
        let panel = SnippetPanel {
            asking: Some(Asking {
                sql: "SELECT * FROM T WHERE L = '${library:QGPL}'".to_owned(),
                fields: vec![("library".to_owned(), "QGPL".to_owned())],
                run: true,
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
            state.0.remembered.get("library").map(String::as_str),
            Some("QGPL")
        );
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
