//! Parley SQL's settings (R-SQL-47, R-SQL-69): its own, in its own storage. The dialog's frame,
//! its Theme and Text groups and the About page are Parley's (`parley_ui_egui::settings`); the
//! pages are Parley SQL's: Appearance with the SQL window's group, Shortcuts (R-SQL-60), SQL
//! (Java, limits, snippets, formatting) and the SQL assistant (R-SQL-48).
//! Changes show at once; Cancel puts the old ones back.

mod assistant;
mod sql_keys;

use std::collections::BTreeMap;
use std::path::PathBuf;

use egui::Ui;
use serde::{Deserialize, Serialize};

use parley_sql_core::text::Style;
use parley_ui_egui::addons::{AddOnSettings, Toggle};
use parley_ui_egui::menu::Program;
use parley_ui_egui::settings::about::About;
use parley_ui_egui::settings::{
    self as shared, Clicked, Look, Sections, SettingsPage, Theme, group,
};
use parley_ui_egui::theme::{self, Density};
use parley_ui_egui::updates::UpdateStatus;

pub use parley_ui_egui::settings::Closed;
pub use shared::apply;

use crate::assistant::ApiKeys;

/// The SQL assistant (R-SQL-44, R-ADD-3, D25, D27): off until turned on in its settings page,
/// and kept with the add-ons under the id it has always had.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Assistant;

impl Assistant {
    pub const fn name(self) -> &'static str {
        "SQL assistant"
    }

    pub const fn about(self) -> &'static str {
        "Off: nothing of it shows in the SQL window and nothing is sent."
    }
}

impl Toggle for Assistant {
    fn id(self) -> &'static str {
        "sql-assistant"
    }

    fn on_by_default(self) -> bool {
        false
    }
}

/// Parley SQL's settings, kept by the launcher. Stored flat, under the names Parley SQL has
/// always used, so settings saved by an earlier version load; fields it no longer has (the
/// terminal's) are ignored when read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each is an on/off choice of its own in the settings dialog"
)]
pub struct Settings {
    pub theme: Theme,
    /// Everything's size, in percent.
    #[serde(alias = "zoom")]
    pub text_size: u16,
    pub density: Density,
    /// Look for a new version when Parley SQL starts (R-UI-11).
    pub check_for_updates: bool,
    /// Whether the SQL assistant is on ([`Assistant`]).
    pub addons: AddOnSettings,
    /// The Java the SQL window runs its helper with; empty: `JAVA_HOME`, then `PATH` (R-SQL-12).
    pub java_path: String,
    /// Rows the SQL window fetches at first and with each Fetch more (R-SQL-18).
    pub sql_rows: u32,
    /// What separates fields in CSV downloads and exports (R-SQL-100).
    pub sql_csv_delimiter: parley_sql_core::transfer::CsvDelimiter,
    /// Seconds before the SQL helper stops a statement; 0 for none (R-SQL-18).
    pub sql_timeout: u32,
    /// What the SQL window's column headings say (R-SQL-29).
    pub sql_header: crate::grid::HeaderMode,
    /// Where the SQL window's results go: below the editor, beside it, or automatic (R-SQL-49).
    pub sql_results: crate::layout::ResultsPlace,
    /// Brackets and quotes close themselves in the SQL editor (R-SQL-55).
    pub sql_close_pairs: bool,
    /// How Format SQL writes: keyword case, indentation, commas (R-SQL-77).
    pub sql_format: Style,
    /// SQL run from the editor asks for its `${…}` values first (R-SQL-120).
    pub sql_placeholders: bool,
    /// Runs are kept in the history (R-SQL-121, D66).
    pub sql_history_keep: bool,
    /// How many runs the history keeps per connection (R-SQL-121).
    pub sql_history_size: usize,
    /// Placeholder values are kept across restarts (R-SQL-122, D66).
    pub sql_remember_values: bool,
    /// Ask before risky statements (R-SQL-123).
    pub sql_confirm_risky: bool,
    /// What a NULL shows as in the grid (R-SQL-124).
    pub sql_null_text: String,
    /// What a NULL is in CSV (R-SQL-124).
    pub sql_csv_null: parley_sql_core::transfer::CsvNull,
    /// CSV starts with a byte-order mark (R-SQL-126).
    pub sql_csv_bom: bool,
    /// CSV has a header row (R-SQL-126).
    pub sql_csv_header: bool,
    /// Show Results or Messages after a run (R-SQL-125).
    pub sql_show_outcome: bool,
    /// The most objects read per library (R-SQL-127).
    pub sql_max_objects: u32,
    /// Parley SQL's shortcuts that differ from their own keys, by action (R-SQL-60).
    pub sql_keys: crate::keys::Overrides,
    /// A folder of snippets shared with others, besides the operator's own; empty for none (R-SQL-24).
    pub sql_snippets_shared: String,
    /// The statements run in the SQL window, by saved connection, oldest first (R-SQL-7).
    pub sql_history: BTreeMap<String, Vec<parley_sql_core::history::Entry>>,
    /// The SQL window's scripts, by saved connection (R-SQL-20).
    pub sql_scripts: BTreeMap<String, Vec<crate::editor::SavedScript>>,
    /// The libraries pinned in the SQL window's schema browser, by saved connection (R-SQL-34):
    /// read once to make a connection's group "Pinned", then removed (R-SQL-83).
    pub sql_pinned: BTreeMap<String, Vec<String>>,
    /// The SQL window's library groups, by saved connection (R-SQL-83).
    pub sql_groups: BTreeMap<String, Vec<crate::browser::LibraryGroup>>,
    /// The snippets' placeholder values used, by saved connection (R-SQL-113).
    pub sql_snippet_values: BTreeMap<String, crate::RecentValues>,
    /// The SQL window's side panel: its width, folded, its sections (R-SQL-82, R-SQL-84).
    pub sql_side: crate::side::SideLayout,
    /// The SQL assistant's provider, model and address; never a key (R-SQL-48, D27).
    pub assistant: parley_sql_core::assistant::AssistantConfig,
    /// The last connections made, newest first, at most [`parley_ui_egui::connect::RECENT_KEPT`]; one not saved has
    /// no name (R-SQL-105).
    pub recent_connections: Vec<crate::app::SqlConnection>,
    /// "All connections" is open on the start screen (R-SQL-105).
    pub all_connections_open: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: Theme::System,
            text_size: 100,
            density: Density::Comfortable,
            check_for_updates: true,
            addons: AddOnSettings::default(),
            java_path: String::new(),
            sql_rows: parley_sql_core::protocol::FIRST_ROWS,
            sql_csv_delimiter: parley_sql_core::transfer::CsvDelimiter::default(),
            sql_timeout: 0,
            sql_header: crate::grid::HeaderMode::default(),
            sql_results: crate::layout::ResultsPlace::default(),
            sql_close_pairs: true,
            sql_format: Style::default(),
            sql_keys: crate::keys::Overrides::new(),
            sql_snippets_shared: String::new(),
            sql_placeholders: true,
            sql_history_keep: true,
            sql_history_size: parley_sql_core::history::LIMIT,
            sql_remember_values: true,
            sql_confirm_risky: true,
            sql_null_text: crate::grid::NULL.to_owned(),
            sql_csv_null: parley_sql_core::transfer::CsvNull::default(),
            sql_csv_bom: true,
            sql_csv_header: true,
            sql_show_outcome: true,
            sql_max_objects: parley_sql_core::catalog::MAX_OBJECTS,
            sql_history: BTreeMap::new(),
            sql_scripts: BTreeMap::new(),
            sql_pinned: BTreeMap::new(),
            sql_groups: BTreeMap::new(),
            sql_snippet_values: BTreeMap::new(),
            sql_side: crate::side::SideLayout::default(),
            assistant: parley_sql_core::assistant::AssistantConfig::default(),
            recent_connections: Vec::new(),
            all_connections_open: false,
        }
    }
}

impl Settings {
    /// What the SQL window is to do, as these settings say (R-SQL-123..127).
    pub fn sql_choices(&self) -> crate::Choices {
        crate::Choices {
            confirm_risky: self.sql_confirm_risky,
            show_outcome: self.sql_show_outcome,
            null_text: self.sql_null_text.clone(),
            csv: parley_sql_core::transfer::CsvOptions {
                delimiter: self.sql_csv_delimiter,
                bom: self.sql_csv_bom,
                header: self.sql_csv_header,
                null: self.sql_csv_null,
            },
            max_objects: self.sql_max_objects,
        }
    }

    /// How Parley SQL looks with these settings. It has no terminal: its monospace font is
    /// Parley's default.
    pub fn look(&self) -> Look {
        Look {
            theme: self.theme,
            text_size: self.text_size,
            density: self.density,
            terminal_font: parley_ui_egui::fonts::DEFAULT_TERMINAL_FONT.to_owned(),
        }
    }
}

/// A group of settings: one entry in the dialog's list on the left.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Page {
    Appearance,
    /// Parley SQL's shortcuts (R-SQL-60).
    Shortcuts,
    /// The SQL window's Java, limits, shared snippets and formatting (R-SQL-47).
    Sql,
    /// The SQL assistant: on or off, the provider and its key (R-SQL-48).
    Assistant,
    About,
}

impl SettingsPage for Page {
    fn name(self) -> &'static str {
        match self {
            Self::Appearance => "Appearance",
            Self::Shortcuts => "Shortcuts",
            Self::Sql => "SQL",
            Self::Assistant => "SQL assistant",
            Self::About => "About",
        }
    }
}

impl Page {
    /// Puts this page's settings back to their defaults; the other pages keep theirs.
    fn restore_defaults(self, settings: &mut Settings) {
        let defaults = Settings::default();
        match self {
            Self::Appearance => {
                settings.theme = defaults.theme;
                settings.text_size = defaults.text_size;
                settings.density = defaults.density;
                settings.sql_results = defaults.sql_results;
                settings.sql_close_pairs = defaults.sql_close_pairs;
            }
            Self::Shortcuts => settings.sql_keys = defaults.sql_keys,
            Self::Sql => {
                settings.java_path = defaults.java_path;
                settings.sql_rows = defaults.sql_rows;
                settings.sql_timeout = defaults.sql_timeout;
                settings.sql_snippets_shared = defaults.sql_snippets_shared;
                settings.sql_placeholders = defaults.sql_placeholders;
                settings.sql_format = defaults.sql_format;
                settings.sql_csv_delimiter = defaults.sql_csv_delimiter;
                settings.sql_history_keep = defaults.sql_history_keep;
                settings.sql_history_size = defaults.sql_history_size;
                settings.sql_remember_values = defaults.sql_remember_values;
                settings.sql_confirm_risky = defaults.sql_confirm_risky;
                settings.sql_null_text = defaults.sql_null_text;
                settings.sql_csv_null = defaults.sql_csv_null;
                settings.sql_csv_bom = defaults.sql_csv_bom;
                settings.sql_csv_header = defaults.sql_csv_header;
                settings.sql_show_outcome = defaults.sql_show_outcome;
                settings.sql_max_objects = defaults.sql_max_objects;
            }
            Self::Assistant => assistant::restore_defaults(settings),
            Self::About => settings.check_for_updates = defaults.check_for_updates,
        }
    }
}

/// Parley SQL's pages (R-SQL-47): no terminal, keyboard maps or add-on list; its own
/// shortcuts (R-SQL-60).
const SECTIONS: Sections<Page> = &[
    ("General", &[Page::Appearance, Page::Shortcuts]),
    ("App", &[Page::Sql, Page::Assistant, Page::About]),
];

/// The open settings dialog.
#[derive(Debug)]
pub struct Dialog {
    /// The settings when the dialog opened, for Cancel.
    original: Settings,
    page: Page,
    about: About,
    sql_keys: sql_keys::Page,
    /// The folder of the SQL helper's jars, if this installation has one (R-SQL-13).
    sql_helpers: Option<PathBuf>,
    /// The Java path last checked and what was found, so it is not looked for every frame.
    java_found: Option<(String, Result<String, String>)>,
    /// The SQL assistant's page, with the keychain (R-SQL-48).
    assistant: assistant::Page,
}

impl Dialog {
    pub fn open(_ctx: &egui::Context, settings: &Settings) -> Self {
        Self {
            original: settings.clone(),
            page: Page::Appearance,
            about: About::default(),
            sql_keys: sql_keys::Page::default(),
            sql_helpers: None,
            java_found: None,
            assistant: assistant::Page::default(),
        }
    }

    /// The folder of the SQL helper's jars, for the SQL page (R-SQL-12).
    pub fn with_sql_helpers(mut self, folder: Option<PathBuf>) -> Self {
        self.sql_helpers = folder;
        self
    }

    /// The keychain the SQL assistant's page saves API keys in (R-SQL-48); without one, the
    /// page says to use the environment.
    pub fn with_api_keys(mut self, keys: Option<std::sync::Arc<dyn ApiKeys>>) -> Self {
        self.assistant = assistant::Page::new(keys);
        self
    }

    /// True once after "Check now" was clicked.
    pub fn take_check_request(&mut self) -> bool {
        self.about.take_check_request()
    }

    pub fn original(&self) -> &Settings {
        &self.original
    }

    /// Draws the dialog over `settings`, which change as the operator picks.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        settings: &mut Settings,
        updates: &UpdateStatus,
    ) -> Option<Closed> {
        let mut page = self.page;
        let clicked = shared::layout::frame(ctx, &mut page, SECTIONS, |ui, page| {
            self.page_content(ui, page, settings, updates);
        });
        self.page = page;
        match clicked? {
            Clicked::Keep => Some(Closed::Keep),
            Clicked::Revert => Some(Closed::Revert),
            Clicked::RestoreDefaults => {
                if self.page == Page::Shortcuts {
                    self.sql_keys.restore(settings);
                } else {
                    self.page.restore_defaults(settings);
                }
                None
            }
        }
    }

    fn page_content(
        &mut self,
        ui: &mut Ui,
        page: Page,
        settings: &mut Settings,
        updates: &UpdateStatus,
    ) {
        match page {
            Page::Appearance => appearance_page(ui, settings),
            Page::Shortcuts => self.sql_keys.show(ui, settings),
            Page::Sql => self.sql_page(ui, settings),
            Page::Assistant => self.assistant.show(ui, settings),
            Page::About => {
                self.about.show(
                    ui,
                    Program::ParleySql,
                    &mut settings.check_for_updates,
                    updates,
                );
            }
        }
    }

    /// Parley SQL's SQL page (R-SQL-47): the Java for the SQL helper, the SQL window's limits,
    /// what was found, and whether the helper is installed. SQL itself is always on; the
    /// assistant has a page of its own (R-SQL-48).
    fn sql_page(&mut self, ui: &mut Ui, settings: &mut Settings) {
        let note = (
            "The SQL window runs its work in Java, with IBM's JTOpen.",
            "Parley does not include Java. Install Java 11 or later, for example the free \
             Eclipse Temurin from adoptium.net, or name the java program here.",
        );
        let found = self.java_for(&settings.java_path).clone();
        let helpers = self.sql_helpers.clone();
        group(ui, "SQL", Some(note), |rows| {
            rows.row(
                "Java",
                Some("Empty: JAVA_HOME, then java on the PATH."),
                |ui, name_id| {
                    let edit =
                        egui::TextEdit::singleline(&mut settings.java_path).hint_text("Automatic");
                    theme::sized_edit(ui, 260.0, edit).labelled_by(name_id);
                },
            );
            rows.row(
                "Rows at a time",
                Some("Fetched at first, and by each Fetch more."),
                |ui, name_id| {
                    let value = egui::DragValue::new(&mut settings.sql_rows)
                        .range(100..=100_000)
                        .speed(50);
                    ui.add(value).labelled_by(name_id);
                },
            );
            rows.row(
                "Query timeout",
                Some("Seconds before a statement is stopped; 0 for none."),
                |ui, name_id| {
                    let value = egui::DragValue::new(&mut settings.sql_timeout)
                        .range(0..=86_400)
                        .suffix(" s");
                    ui.add(value).labelled_by(name_id);
                },
            );
            rows.row(
                "CSV delimiter",
                Some("Semicolon for Excel where the comma is the decimal separator."),
                |ui, _| {
                    use parley_sql_core::transfer::CsvDelimiter;
                    theme::segmented(
                        ui,
                        &mut settings.sql_csv_delimiter,
                        &[
                            (CsvDelimiter::Comma, "Comma ,"),
                            (CsvDelimiter::Semicolon, "Semicolon ;"),
                        ],
                    );
                },
            );
            rows.row(
                "Shared snippets",
                Some("A folder of snippets shared with others, such as a network folder."),
                |ui, name_id| {
                    let edit = egui::TextEdit::singleline(&mut settings.sql_snippets_shared)
                        .hint_text("None");
                    theme::sized_edit(ui, 260.0, edit).labelled_by(name_id);
                },
            );
            rows.switch(
                "Ask for ${…} values",
                Some("SQL run from the editor that holds ${name} asks for the values first."),
                &mut settings.sql_placeholders,
            );
            rows.fact("Found", |ui| match &found {
                Ok(text) => {
                    ui.label(text);
                }
                Err(text) => theme::warning(ui, text),
            });
            rows.fact("SQL helper", |ui| match &helpers {
                Some(folder) => {
                    ui.label(folder.display().to_string());
                }
                None => theme::warning(
                    ui,
                    "Not installed with this Parley (a development build needs make helper).",
                ),
            });
        });
        running_group(ui, settings);
        results_group(ui, settings);
        formatting_group(ui, &mut settings.sql_format);
    }

    /// What Java `path` finds, looked up only when it changed.
    fn java_for(&mut self, path: &str) -> &std::result::Result<String, String> {
        let stale = self
            .java_found
            .as_ref()
            .is_none_or(|(checked, _)| checked != path);
        if stale {
            let configured = (!path.trim().is_empty()).then(|| std::path::Path::new(path.trim()));
            let result = parley_sql_core::java::find(configured)
                .map(|java| format!("Java {} ({})", java.version, java.path.display()))
                .map_err(|e| e.to_string());
            self.java_found = Some((path.to_owned(), result));
        }
        &self.java_found.as_ref().expect("set just above").1
    }
}

/// Theme, text size and density, and where the SQL window's results go (R-SQL-49).
fn appearance_page(ui: &mut Ui, settings: &mut Settings) {
    shared::appearance::groups(
        ui,
        &mut settings.theme,
        &mut settings.text_size,
        &mut settings.density,
    );
    group(ui, "SQL window", None, |rows| {
        rows.row(
            "Results",
            Some("Beside the editor, or below it; Automatic: beside when the window is wide."),
            |ui, _| {
                use crate::layout::ResultsPlace;
                theme::segmented(
                    ui,
                    &mut settings.sql_results,
                    &[
                        (ResultsPlace::Below, "Below"),
                        (ResultsPlace::Beside, "Beside"),
                        (ResultsPlace::Automatic, "Automatic"),
                    ],
                );
            },
        );
        rows.switch(
            "Close brackets and quotes",
            Some("Typing ( ' or \" in the editor puts in the closing one too."),
            &mut settings.sql_close_pairs,
        );
    });
}

/// Running, the history and what is remembered (R-SQL-121..123, R-SQL-125, R-SQL-127).
fn running_group(ui: &mut Ui, settings: &mut Settings) {
    group(ui, "Running and history", None, |rows| {
        rows.switch(
            "Ask before risky statements",
            Some("DELETE or UPDATE without WHERE, DROP, TRUNCATE, ALTER and CL commands that remove."),
            &mut settings.sql_confirm_risky,
        );
        rows.switch(
            "Show the outcome after a run",
            Some(
                "Results after a query, Messages after other statements; off keeps the tab shown.",
            ),
            &mut settings.sql_show_outcome,
        );
        rows.switch(
            "Keep a history",
            Some("Each run's SQL is kept in the settings file; History's Clear removes it."),
            &mut settings.sql_history_keep,
        );
        rows.row(
            "Runs kept",
            Some("Per connection; the oldest go first."),
            |ui, name_id| {
                let value = egui::DragValue::new(&mut settings.sql_history_size)
                    .range(50..=5000)
                    .speed(10);
                ui.add_enabled(settings.sql_history_keep, value)
                    .labelled_by(name_id);
            },
        );
        rows.switch(
            "Remember values",
            Some("Values typed for ${…} are offered again after a restart; off forgets them."),
            &mut settings.sql_remember_values,
        );
        rows.row(
            "Objects per library",
            Some("Read for the Explorer and completion; more takes longer on big libraries."),
            |ui, name_id| {
                let value = egui::DragValue::new(&mut settings.sql_max_objects)
                    .range(1000..=100_000)
                    .speed(100);
                ui.add(value).labelled_by(name_id);
            },
        );
    });
}

/// How results show NULL, and how CSV is written (R-SQL-124, R-SQL-126).
fn results_group(ui: &mut Ui, settings: &mut Settings) {
    use parley_sql_core::transfer::CsvNull;
    group(ui, "Results and CSV", None, |rows| {
        rows.row(
            "NULL shows as",
            Some("In the grid and a row's values."),
            |ui, name_id| {
                let edit = egui::TextEdit::singleline(&mut settings.sql_null_text).char_limit(20);
                theme::sized_edit(ui, 120.0, edit).labelled_by(name_id);
            },
        );
        rows.row(
            "NULL in CSV",
            Some("Empty is the same as an empty text."),
            |ui, _| {
                theme::segmented(
                    ui,
                    &mut settings.sql_csv_null,
                    &[(CsvNull::Empty, "Empty"), (CsvNull::Null, "NULL")],
                );
            },
        );
        rows.switch(
            "CSV starts with a byte-order mark",
            Some("Excel then reads æøå; some other programs do not want it."),
            &mut settings.sql_csv_bom,
        );
        rows.switch(
            "CSV has a header row",
            Some("The column names as the first line."),
            &mut settings.sql_csv_header,
        );
    });
}

/// How Format SQL writes (R-SQL-77): keyword case, indentation and where list commas go.
fn formatting_group(ui: &mut Ui, style: &mut Style) {
    use parley_sql_core::text::KeywordCase;
    group(ui, "Formatting", None, |rows| {
        rows.row(
            "Keywords",
            Some("The case Format SQL gives them."),
            |ui, _| {
                theme::segmented(
                    ui,
                    &mut style.case,
                    &[
                        (KeywordCase::Upper, "Upper"),
                        (KeywordCase::Lower, "Lower"),
                        (KeywordCase::AsWritten, "As written"),
                    ],
                );
            },
        );
        rows.row("Indentation", None, |ui, _| {
            theme::segmented(ui, &mut style.indent, &[(2, "2 spaces"), (4, "4 spaces")]);
        });
        rows.row(
            "List commas",
            Some("Where Format SQL puts them."),
            |ui, _| {
                theme::segmented(
                    ui,
                    &mut style.commas_first,
                    &[(false, "At the end"), (true, "At the start")],
                );
            },
        );
    });
}
