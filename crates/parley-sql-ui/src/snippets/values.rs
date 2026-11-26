//! The values form (R-SQL-26): a field per placeholder with its type's hint (R-SQL-110), the
//! values used before (R-SQL-113), names from the host (R-SQL-115) and the SQL as it will run
//! (R-SQL-109).

use std::collections::BTreeMap;

use egui::{RichText, ScrollArea, TextEdit, Ui};
use parley_sql_core::snippet::{self, ValueType};
use parley_ui_egui::{dialog, icons, theme};

use super::{SnippetPanel, Use};

/// The placeholder form.
#[derive(Debug, Clone, Default)]
pub(super) struct Asking {
    pub(super) sql: String,
    pub(super) fields: Vec<(String, String)>,
    pub(super) run: bool,
    /// The field that last had the keyboard, by name (R-SQL-115).
    pub(super) focused: Option<String>,
    /// The names offered for it, from the window (R-SQL-115).
    pub(super) names: Vec<String>,
    /// The snippet's title, for its result's (R-SQL-117); empty for the editor's SQL.
    pub(super) snippet: String,
    /// SQL run from the editor (R-SQL-120): Run as written, no Insert.
    pub(super) from_editor: bool,
}

/// A snippet run, for its result's title and Run again (R-SQL-117).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    /// The snippet's title.
    pub snippet: String,
    /// Its SQL, placeholders and all.
    pub sql: String,
    /// The result's title: the snippet's, with the values given.
    pub title: String,
}

impl Report {
    fn new(snippet: &str, sql: &str, values: &[(String, String)]) -> Self {
        let values: Vec<&str> = values
            .iter()
            .map(|(_, v)| v.trim())
            .filter(|v| !v.is_empty())
            .collect();
        let title = if values.is_empty() {
            snippet.to_owned()
        } else {
            format!("{snippet} — {}", values.join(", "))
        };
        Self {
            snippet: snippet.to_owned(),
            sql: sql.to_owned(),
            title,
        }
    }
}

/// The names a library or table field wants offered (R-SQL-115).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Wanted {
    /// Libraries starting with `prefix` (in capitals).
    Libraries { prefix: String },
    /// Tables of `library` (the job's current one when `None`) starting with `prefix`.
    Tables {
        library: Option<String>,
        prefix: String,
    },
}

/// At most this many names are offered (R-SQL-115).
const MAX_NAMES: usize = 50;

impl Asking {
    /// The SQL with the values, each as its type writes it (R-SQL-26, R-SQL-110).
    pub(super) fn filled(&self) -> String {
        snippet::fill(&self.sql, |name| self.value(name).map(str::to_owned))
    }

    pub(super) fn value(&self, name: &str) -> Option<&str> {
        self.fields
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    /// The names the field that last had the keyboard wants, when it is a library or table's
    /// (R-SQL-115). A table's library is the first library field's value, when it is a name.
    pub(super) fn wanted(&self) -> Option<Wanted> {
        let focused = self.focused.as_deref()?;
        let kinds = snippet::placeholders(&self.sql);
        let prefix = self
            .value(focused)
            .unwrap_or_default()
            .trim()
            .to_uppercase();
        match kinds.iter().find(|p| p.name == focused)?.kind {
            ValueType::Library => Some(Wanted::Libraries { prefix }),
            ValueType::Table => {
                let library = kinds
                    .iter()
                    .find(|p| p.kind == ValueType::Library)
                    .and_then(|p| ValueType::Library.render(self.value(&p.name)?).ok());
                Some(Wanted::Tables { library, prefix })
            }
            ValueType::Any
            | ValueType::Text
            | ValueType::Number
            | ValueType::Date
            | ValueType::Name
            | ValueType::List
            | ValueType::Unknown(_) => None,
        }
    }

    /// The values that do not fit their type, as "name: why" (R-SQL-110).
    pub(super) fn refused(&self) -> Vec<String> {
        snippet::placeholders(&self.sql)
            .into_iter()
            .filter_map(|p| {
                let value = self.value(&p.name).unwrap_or_default();
                let why = p.kind.render(value).err()?;
                Some(format!("{}: {why}", p.name))
            })
            .collect()
    }
}

/// A field per placeholder, its type's hint and the values used before (R-SQL-26, R-SQL-110,
/// R-SQL-113); the name of the field that has the keyboard.
fn value_fields(
    ui: &mut Ui,
    fields: &mut [(String, String)],
    kinds: &[snippet::Placeholder],
    recent: &RecentValues,
) -> Option<String> {
    let mut focused = None;
    egui::Grid::new("sql-snippet-values")
        .num_columns(3)
        .spacing([12.0, 8.0])
        .show(ui, |ui| {
            for (name, value) in fields {
                let label = ui.label(name.as_str());
                let hint = kinds
                    .iter()
                    .find(|p| &p.name == name)
                    .map_or("", |p| p.kind.hint());
                let field = TextEdit::singleline(value).hint_text(hint);
                let response = theme::sized_edit(ui, 260.0, field)
                    .labelled_by(label.id)
                    .on_hover_text(hint);
                if response.has_focus() {
                    focused = Some(name.clone());
                }
                if let Some(used) = recent.get(name.as_str()).filter(|v| !v.is_empty()) {
                    recent_menu(ui, used, value);
                }
                ui.end_row();
            }
        });
    focused
}

/// The names offered for the library or table field last typed in, to click (R-SQL-115); a
/// fixed height, and only when the snippet has such a field.
fn name_choices(ui: &mut Ui, asking: &mut Asking, kinds: &[snippet::Placeholder]) {
    let has_names = kinds
        .iter()
        .any(|p| matches!(p.kind, ValueType::Library | ValueType::Table));
    if !has_names {
        return;
    }
    ui.add_space(6.0);
    theme::section_label(ui, "Names");
    let mut chosen = None;
    ScrollArea::vertical()
        .id_salt("sql-snippet-names")
        .max_height(72.0)
        .min_scrolled_height(72.0)
        .show(ui, |ui| {
            if asking.wanted().is_none() {
                theme::hint(ui, "Click in a library or table field to see its names.");
            } else if asking.names.is_empty() {
                theme::hint(ui, "No names known that start with what is typed.");
            }
            ui.horizontal_wrapped(|ui| {
                for name in &asking.names {
                    if ui.add(theme::ghost_button(name.as_str())).clicked() {
                        chosen = Some(name.clone());
                    }
                }
            });
        });
    if let Some(name) = chosen
        && let Some(focused) = asking.focused.clone()
        && let Some((_, value)) = asking.fields.iter_mut().find(|(n, _)| *n == focused)
    {
        *value = name;
    }
}

/// The values used before for one placeholder, newest first; a click puts one in the field
/// (R-SQL-113).
fn recent_menu(ui: &mut Ui, used: &[String], value: &mut String) {
    ui.menu_button(icons::icon(icons::CARET_DOWN), |ui| {
        for v in used {
            if ui.button(v.as_str()).clicked() {
                value.clone_from(v);
                ui.close();
            }
        }
    })
    .response
    .on_hover_text("Values used before");
}

/// What a button of the values form does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Go {
    Run,
    Insert,
    /// Runs the SQL with its `${…}` unchanged (R-SQL-120).
    AsWritten,
}

impl Go {
    const fn label(self) -> &'static str {
        match self {
            Self::Run => "Run",
            Self::Insert => "Insert",
            Self::AsWritten => "Run as written",
        }
    }
}

/// The values form's buttons (R-SQL-109): Cancel, then `buttons`, the first filled. Run and
/// Insert wait for `ready`; Run as written never does. `(cancel, Some(go))` once one is clicked.
pub(super) fn values_actions(ui: &mut Ui, buttons: [Go; 2], ready: bool) -> (bool, Option<Go>) {
    ui.add_space(16.0);
    theme::button_row(ui, |ui| {
        let enabled = |go: Go| ready || go == Go::AsWritten;
        let [first, second] = buttons;
        let mut go = None;
        if ui
            .add_enabled(enabled(first), theme::primary_button(ui, first.label()))
            .clicked()
        {
            go = Some(first);
        }
        if ui
            .add_enabled(enabled(second), egui::Button::new(second.label()))
            .clicked()
        {
            go = Some(second);
        }
        let cancel = ui.button("Cancel").clicked();
        (cancel, go)
    })
}

/// Placeholder values used, by placeholder name, newest first, as the settings keep them for a
/// connection (R-SQL-113).
pub type RecentValues = BTreeMap<String, Vec<String>>;

/// How many values are kept for each placeholder (R-SQL-113).
const RECENT: usize = 5;

/// Whether a placeholder named `name` may hold a password, so its values are never kept
/// (R-SQL-113).
fn secret(name: &str) -> bool {
    let name = name.to_lowercase();
    ["password", "passwd", "pwd", "secret"]
        .iter()
        .any(|w| name.contains(w))
}

impl SnippetPanel {
    /// The names the values form wants offered, while it is open (R-SQL-115).
    pub fn names_wanted(&self) -> Option<Wanted> {
        self.asking.as_ref()?.wanted()
    }

    /// The names to offer in the values form (R-SQL-115).
    pub fn offer_names(&mut self, mut names: Vec<String>) {
        names.truncate(MAX_NAMES);
        if let Some(asking) = self.asking.as_mut() {
            asking.names = names;
        }
    }

    /// The placeholder values used on this connection before (R-SQL-113).
    pub fn set_recent(&mut self, values: RecentValues) {
        self.remembered = values;
    }

    /// The placeholder values used, as the settings keep them (R-SQL-113).
    pub const fn recent(&self) -> &RecentValues {
        &self.remembered
    }

    /// Keeps `value` as the newest of placeholder `name`; not an empty one, nor a password's.
    pub(super) fn remember(&mut self, name: &str, value: &str) {
        if secret(name) || value.trim().is_empty() {
            return;
        }
        let values = self.remembered.entry(name.to_owned()).or_default();
        values.retain(|v| v != value);
        values.insert(0, value.to_owned());
        values.truncate(RECENT);
    }

    /// Uses snippet `i`: at once, or after the placeholder form (R-SQL-26).
    pub fn start(&mut self, i: usize, run: bool) -> Option<Use> {
        let entry = self.entries.get(i)?;
        let (sql, title) = (entry.snippet.sql.clone(), entry.snippet.title.clone());
        self.use_sql(sql, title, run)
    }

    /// Runs the snippet `snippet` with SQL `sql` again, asking for its values (R-SQL-117).
    pub fn run_again(&mut self, report: &Report) -> Option<Use> {
        self.use_sql(report.sql.clone(), report.snippet.clone(), true)
    }

    /// What was run last from a snippet, for its result (R-SQL-117); taken once.
    pub const fn take_report(&mut self) -> Option<Report> {
        self.report.take()
    }

    /// Uses the SQL of snippet `title`: at once, or after the placeholder form (R-SQL-26).
    fn use_sql(&mut self, sql: String, title: String, run: bool) -> Option<Use> {
        let placeholders = snippet::placeholders(&sql);
        if placeholders.is_empty() {
            if !run {
                return Some(Use::Insert(sql));
            }
            self.report = Some(Report::new(&title, &sql, &[]));
            return Some(Use::Run(sql));
        }
        self.asking = Some(Asking {
            fields: self.fields_for(placeholders),
            sql,
            run,
            snippet: title,
            ..Asking::default()
        });
        None
    }

    /// Asks for the values of the `${…}` in `sql`, run from the editor, before it runs; `false`
    /// when it has none (R-SQL-120).
    pub fn ask_for_editor(&mut self, sql: &str) -> bool {
        let placeholders = snippet::placeholders(sql);
        if placeholders.is_empty() {
            return false;
        }
        self.asking = Some(Asking {
            fields: self.fields_for(placeholders),
            sql: sql.to_owned(),
            run: true,
            from_editor: true,
            ..Asking::default()
        });
        true
    }

    /// A field per placeholder, with the value used last, else its default (R-SQL-113).
    fn fields_for(&self, placeholders: Vec<snippet::Placeholder>) -> Vec<(String, String)> {
        placeholders
            .into_iter()
            .map(|p| {
                let value = self
                    .remembered
                    .get(&p.name)
                    .and_then(|v| v.first())
                    .cloned()
                    .or(p.default)
                    .unwrap_or_default();
                (p.name, value)
            })
            .collect()
    }

    /// R-SQL-26: a field per placeholder; Insert or Run puts the values in.
    pub(super) fn placeholder_dialog(&mut self, ctx: &egui::Context) -> Option<Use> {
        let mut asking = self.asking.take()?;
        let kinds = snippet::placeholders(&asking.sql);
        let recent = &self.remembered;
        let (title, buttons) = match (asking.from_editor, asking.run) {
            (true, _) => ("Values for the SQL", [Go::Run, Go::AsWritten]),
            (false, true) => ("Values for the snippet", [Go::Run, Go::Insert]),
            (false, false) => ("Values for the snippet", [Go::Insert, Go::Run]),
        };
        let (close, (cancel, go)) = dialog::dialog(ctx, "sql-snippet-values", title, 440.0, |ui| {
            if let Some(focused) = value_fields(ui, &mut asking.fields, &kinds, recent) {
                asking.focused = Some(focused);
            }
            name_choices(ui, &mut asking, &kinds);
            // R-SQL-110: a value that does not fit its type waits to be put right.
            let refused = asking.refused();
            for why in &refused {
                theme::error(ui, why);
            }
            // R-SQL-109: the SQL as it will run, filled as the values are typed.
            ui.add_space(6.0);
            let preview = ui.label("SQL");
            ScrollArea::vertical()
                .id_salt("sql-snippet-filled")
                .max_height(120.0)
                .min_scrolled_height(120.0)
                .show(ui, |ui| {
                    let filled = asking.filled();
                    ui.add(egui::Label::new(RichText::new(filled).monospace()).wrap())
                        .labelled_by(preview.id);
                });
            values_actions(ui, buttons, refused.is_empty())
        });
        if close || cancel {
            return None;
        }
        let Some(go) = go else {
            self.asking = Some(asking);
            return None;
        };
        if go == Go::AsWritten {
            return Some(Use::Run(asking.sql));
        }
        for (name, value) in &asking.fields {
            self.remember(name, value);
        }
        let sql = asking.filled();
        if go == Go::Insert {
            return Some(Use::Insert(sql));
        }
        if !asking.from_editor {
            self.report = Some(Report::new(&asking.snippet, &asking.sql, &asking.fields));
        }
        Some(Use::Run(sql))
    }
}
