//! Settings (R-UI-6, R-UI-8, R-UI-11, R-KEY-2, R-ADD-1): theme, text size, density, the
//! terminal font, keyboard maps, add-ons and looking for new versions, with what this build is on the About page, in a dialog
//! from Tools ▸ Settings… laid out as Tortuga's (D12): pages listed on the left, and on the
//! right groups of rows, each a name with a short line about it and the control beside it.
//! Changes show at once; Cancel puts the old ones back.

mod assistant;
mod keyboard;
mod sql_keys;

use egui::{Align, ComboBox, CornerRadius, Frame, Id, Layout, Margin, RichText, Stroke, Ui, vec2};
use serde::{Deserialize, Serialize};

use crate::addons::{AddOn, AddOnSettings};
use crate::fonts::{self, Installed};
use crate::keymap::KeyMap;
use crate::menu::Program;
use crate::theme::{self, Density};
use crate::updates::UpdateStatus;
use crate::{dialog, terminal};

/// The text sizes offered, in percent.
const TEXT_SIZES: [u16; 7] = [80, 90, 100, 110, 125, 150, 175];

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Theme {
    /// Follow the operating system.
    #[default]
    System,
    Light,
    Dark,
}

/// The application's settings, kept by the launcher.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub theme: Theme,
    /// Everything's size, in percent.
    #[serde(alias = "zoom")]
    pub text_size: u16,
    pub density: Density,
    /// The terminal font's family name: a bundled font or an installed one.
    pub terminal_font: String,
    /// Look for a new version when Parley starts (R-UI-11).
    pub check_for_updates: bool,
    /// Which add-ons are on (R-ADD-1).
    pub addons: AddOnSettings,
    /// The operator's keyboard maps; the Default map is built in, not kept (R-KEY-2).
    pub keyboard_maps: Vec<KeyMap>,
    /// The Java the SQL add-on runs its helper with; empty: `JAVA_HOME`, then `PATH` (R-SQL-12).
    pub java_path: String,
    /// Rows the SQL window fetches at first and with each Fetch more (R-SQL-18).
    pub sql_rows: u32,
    /// Seconds before the SQL helper stops a statement; 0 for none (R-SQL-18).
    pub sql_timeout: u32,
    /// What the SQL window's column headings say (R-SQL-29).
    pub sql_header: crate::sql::grid::HeaderMode,
    /// Where the SQL window's results go: below the editor, beside it, or automatic (R-SQL-49).
    pub sql_results: crate::sql::layout::ResultsPlace,
    /// Brackets and quotes close themselves in the SQL editor (R-SQL-55).
    pub sql_close_pairs: bool,
    /// Parley SQL's shortcuts that differ from their own keys, by action (R-SQL-60).
    pub sql_keys: crate::sql::keys::Overrides,
    /// A folder of snippets shared with others, besides the operator's own; empty for none (R-SQL-24).
    pub sql_snippets_shared: String,
    /// The statements run in the SQL window, by saved session, oldest first (R-SQL-7).
    pub sql_history: std::collections::BTreeMap<String, Vec<String>>,
    /// The SQL window's scripts, by saved session (R-SQL-20).
    pub sql_scripts: std::collections::BTreeMap<String, Vec<crate::sql::editor::SavedScript>>,
    /// The libraries pinned in the SQL window's schema browser, by saved session (R-SQL-34).
    pub sql_pinned: std::collections::BTreeMap<String, Vec<String>>,
    /// The SQL assistant's provider, model and address; never a key (R-SQL-48, D27).
    pub assistant: parley_core::sql::assistant::AssistantConfig,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: Theme::System,
            text_size: 100,
            density: Density::Comfortable,
            terminal_font: fonts::DEFAULT_TERMINAL_FONT.to_owned(),
            check_for_updates: true,
            addons: AddOnSettings::default(),
            keyboard_maps: Vec::new(),
            java_path: String::new(),
            sql_rows: parley_core::sql::protocol::FIRST_ROWS,
            sql_timeout: 0,
            sql_header: crate::sql::grid::HeaderMode::default(),
            sql_results: crate::sql::layout::ResultsPlace::default(),
            sql_close_pairs: true,
            sql_keys: crate::sql::keys::Overrides::new(),
            sql_snippets_shared: String::new(),
            sql_history: std::collections::BTreeMap::new(),
            sql_scripts: std::collections::BTreeMap::new(),
            sql_pinned: std::collections::BTreeMap::new(),
            assistant: parley_core::sql::assistant::AssistantConfig::default(),
        }
    }
}

/// Puts `new` into effect, changing only what differs from `old` (`None`: the first frame,
/// which installs the theme). Returns a note if the terminal font could not be loaded.
pub fn apply(ctx: &egui::Context, old: Option<&Settings>, new: &Settings) -> Option<String> {
    let note = match old {
        None => theme::install(ctx, &new.terminal_font),
        Some(old) if old.terminal_font != new.terminal_font => {
            theme::set_terminal_font(ctx, &new.terminal_font)
        }
        Some(_) => None,
    };
    if old.is_none_or(|o| o.density != new.density) {
        theme::set_sizes(ctx, new.density);
    }
    if old.is_none_or(|o| o.theme != new.theme) {
        ctx.set_theme(match new.theme {
            Theme::System => egui::ThemePreference::System,
            Theme::Light => egui::ThemePreference::Light,
            Theme::Dark => egui::ThemePreference::Dark,
        });
    }
    if old.is_none_or(|o| o.text_size != new.text_size) {
        ctx.set_zoom_factor(f32::from(new.text_size.clamp(50, 300)) / 100.0);
    }
    note
}

/// A group of settings: one entry in the dialog's list on the left.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Page {
    Appearance,
    Terminal,
    Keyboard,
    AddOns,
    /// Parley SQL's page for the SQL window (R-SQL-47).
    Sql,
    /// The SQL assistant: on or off, the provider and its key (R-SQL-48).
    Assistant,
    /// Parley SQL's shortcuts (R-SQL-60).
    Shortcuts,
    About,
}

impl Page {
    const fn name(self) -> &'static str {
        match self {
            Self::Appearance => "Appearance",
            Self::Terminal => "Terminal",
            Self::Keyboard => "Keyboard",
            Self::AddOns => "Add-ons",
            Self::Sql => "SQL",
            Self::Assistant => "SQL assistant",
            Self::Shortcuts => "Shortcuts",
            Self::About => "About",
        }
    }

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
            Self::Terminal => settings.terminal_font = defaults.terminal_font,
            // The map shown gets the Default map's keys: see `keyboard::Page::restore`.
            Self::Keyboard => {}
            Self::Shortcuts => settings.sql_keys = defaults.sql_keys,
            Self::AddOns => {
                for addon in AddOn::ON_ADDONS_PAGE {
                    settings.addons.set_enabled(addon, addon.on_by_default());
                }
                settings.java_path = defaults.java_path;
                settings.sql_rows = defaults.sql_rows;
                settings.sql_timeout = defaults.sql_timeout;
            }
            Self::Sql => {
                settings.java_path = defaults.java_path;
                settings.sql_rows = defaults.sql_rows;
                settings.sql_timeout = defaults.sql_timeout;
                settings.sql_snippets_shared = defaults.sql_snippets_shared;
            }
            Self::Assistant => assistant::restore_defaults(settings),
            Self::About => settings.check_for_updates = defaults.check_for_updates,
        }
    }
}

/// The pages under their headings, in the order the list shows them.
type Sections = &'static [(&'static str, &'static [Page])];

/// Parley's pages.
const SECTIONS: Sections = &[
    (
        "General",
        &[Page::Appearance, Page::Terminal, Page::Keyboard],
    ),
    ("App", &[Page::AddOns, Page::Assistant, Page::About]),
];

/// Parley SQL's pages (R-SQL-47): no terminal, keyboard maps or add-on list; its own
/// shortcuts (R-SQL-60).
const SQL_SECTIONS: Sections = &[
    ("General", &[Page::Appearance, Page::Shortcuts]),
    ("App", &[Page::Sql, Page::Assistant, Page::About]),
];

/// Where the source and the issues are.
const REPOSITORY: &str = "https://github.com/sorensenNiels/parley";

/// Width of the list of pages.
const NAV_WIDTH: f32 = 168.0;

/// The open settings dialog.
#[derive(Debug)]
pub struct Dialog {
    /// The settings when the dialog opened, for Cancel.
    original: Settings,
    installed: Installed,
    page: Page,
    /// "Check now" was clicked; [`crate::ParleyUi`] starts the check.
    check_requested: bool,
    /// What was last copied, to say so.
    copied: Option<String>,
    keyboard: keyboard::Page,
    sql_keys: sql_keys::Page,
    /// The folder of the SQL helper's jars, if this installation has one (R-SQL-13).
    sql_helpers: Option<std::path::PathBuf>,
    /// The Java path last checked and what was found, so it is not looked for every frame.
    java_found: Option<(String, std::result::Result<String, String>)>,
    /// Parley or Parley SQL: which pages, and the name on About.
    program: Program,
    /// The SQL assistant's page, with the keychain (R-SQL-48).
    assistant: assistant::Page,
}

/// How the dialog closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Closed {
    Keep,
    /// Put the original settings back.
    Revert,
}

impl Dialog {
    pub fn open(ctx: &egui::Context, settings: &Settings) -> Self {
        let ctx = ctx.clone();
        Self {
            original: settings.clone(),
            installed: Installed::search(move || ctx.request_repaint()),
            page: Page::Appearance,
            check_requested: false,
            copied: None,
            keyboard: keyboard::Page::new(crate::keymap::DEFAULT_MAP, None, false),
            sql_keys: sql_keys::Page::default(),
            sql_helpers: None,
            java_found: None,
            program: Program::Parley,
            assistant: assistant::Page::default(),
        }
    }

    /// The dialog for `program`: Parley SQL's pages are Appearance, SQL and About (R-SQL-47).
    pub const fn for_program(mut self, program: Program) -> Self {
        self.program = program;
        self
    }

    const fn sections(&self) -> Sections {
        match self.program {
            Program::Parley => SECTIONS,
            Program::ParleySql => SQL_SECTIONS,
        }
    }

    /// The folder of the SQL helper's jars, for the SQL add-on's rows (R-SQL-12).
    pub fn with_sql_helpers(mut self, folder: Option<std::path::PathBuf>) -> Self {
        self.sql_helpers = folder;
        self
    }

    /// The keychain the SQL assistant's page saves API keys in (R-SQL-48); without one, the
    /// page says to use the environment.
    pub fn with_api_keys(mut self, keys: Option<std::sync::Arc<dyn crate::ApiKeys>>) -> Self {
        self.assistant = assistant::Page::new(keys);
        self
    }

    /// Shows the keyboard map `map` on the Keyboard page, with a key for each of `macros`
    /// (`None` while the Macros add-on is off).
    pub fn with_keyboard(mut self, map: &str, macros: Option<Vec<String>>, sql: bool) -> Self {
        self.keyboard = keyboard::Page::new(map, macros, sql);
        self
    }

    /// Keyboard maps renamed while the dialog was open, in order: (old name, new name). Only
    /// for a dialog closed with Save.
    pub fn take_map_renames(&mut self) -> Vec<(String, String)> {
        self.keyboard.take_renames()
    }

    /// Opens on the About page (Help ▸ Check for updates…).
    pub fn on_about_page(mut self) -> Self {
        self.page = Page::About;
        self
    }

    /// The same dialog on the Keyboard page ("Change keys…" in the shortcut sheet, R-UI-17).
    pub fn on_keyboard_page(mut self) -> Self {
        self.page = Page::Keyboard;
        self
    }

    /// True once after "Check now" was clicked.
    pub fn take_check_request(&mut self) -> bool {
        std::mem::take(&mut self.check_requested)
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
        let (close, closed) = dialog::dialog(ctx, "settings", "Settings", 760.0, |ui| {
            // One size for every page, so the dialog doesn't jump when the page changes.
            let height = (ui.ctx().content_rect().height() - 210.0).clamp(260.0, 500.0);
            let (rect, _) =
                ui.allocate_exact_size(vec2(ui.available_width(), height), egui::Sense::hover());
            let nav_rect = egui::Rect::from_min_size(rect.min, vec2(NAV_WIDTH, height));
            let page_rect =
                egui::Rect::from_min_max(egui::pos2(nav_rect.right() + 16.0, rect.top()), rect.max);
            let mut nav_ui = ui.new_child(egui::UiBuilder::new().max_rect(nav_rect));
            let sections = self.sections();
            nav(&mut nav_ui, &mut self.page, sections, height);
            let mut page_ui = ui.new_child(egui::UiBuilder::new().max_rect(page_rect));
            // Nothing too wide for the page paints outside the dialog.
            page_ui.set_clip_rect(page_rect.expand2(vec2(0.0, 4.0)));
            egui::ScrollArea::vertical()
                .id_salt(("settings-page", self.page))
                .auto_shrink([false, false])
                .show(&mut page_ui, |ui| {
                    ui.add_space(4.0);
                    self.page_content(ui, settings, updates);
                });
            ui.add_space(16.0);
            ui.horizontal(|ui| {
                if ui
                    .button("Restore defaults")
                    .on_hover_text("Puts this page's settings back to their defaults.")
                    .clicked()
                {
                    if self.page == Page::Keyboard {
                        self.keyboard.restore(settings);
                    } else if self.page == Page::Shortcuts {
                        self.sql_keys.restore(settings);
                    } else {
                        self.page.restore_defaults(settings);
                    }
                }
                let save = theme::primary_button(ui, "Save");
                let (cancel, save) = theme::button_row(ui, |ui| {
                    let save = ui.add(save).clicked();
                    (ui.button("Cancel").clicked(), save)
                });
                match (cancel, save) {
                    (_, true) => Some(Closed::Keep),
                    (true, false) => Some(Closed::Revert),
                    (false, false) => None,
                }
            })
            .inner
        });
        if close { Some(Closed::Revert) } else { closed }
    }

    fn page_content(&mut self, ui: &mut Ui, settings: &mut Settings, updates: &UpdateStatus) {
        match self.page {
            Page::Appearance => {
                let sql = self.program == Program::ParleySql || settings.addons.enabled(AddOn::Sql);
                appearance_page(ui, settings, sql);
            }
            Page::Terminal => self.terminal_page(ui, settings),
            Page::Keyboard => self.keyboard.show(ui, settings),
            Page::Shortcuts => self.sql_keys.show(ui, settings),
            Page::AddOns => self.addons_page(ui, settings),
            Page::Sql => self.sql_page(ui, settings),
            Page::Assistant => {
                let needs_sql =
                    self.program == Program::Parley && !settings.addons.enabled(AddOn::Sql);
                self.assistant.show(ui, settings, needs_sql);
            }
            Page::About => self.about_page(ui, settings, updates),
        }
    }

    /// A switch per add-on (R-ADD-1), as Tortuga's Settings → Add-ons; then, while SQL is on,
    /// which Java runs its helper (R-SQL-12).
    fn addons_page(&mut self, ui: &mut Ui, settings: &mut Settings) {
        let note = (
            "An add-on that is off is not loaded. Changes take effect when you save.",
            "Add-ons are parts of Parley not everyone needs. Turning one on adds its menu items; \
             turning it off removes them and stops what it is doing.",
        );
        group(ui, "Add-ons", Some(note), |rows| {
            for addon in AddOn::ON_ADDONS_PAGE {
                let mut on = settings.addons.enabled(addon);
                rows.switch(addon.name(), Some(addon.about()), &mut on);
                settings.addons.set_enabled(addon, on);
            }
        });
        if settings.addons.enabled(AddOn::Sql) {
            self.sql_group(ui, settings);
        }
    }

    /// Parley SQL's SQL page (R-SQL-47): the Java and the SQL window's limits. SQL itself is
    /// always on; the assistant has a page of its own (R-SQL-48).
    fn sql_page(&mut self, ui: &mut Ui, settings: &mut Settings) {
        self.sql_group(ui, settings);
    }

    /// The Java for the SQL helper, what was found, and whether the helper is installed.
    fn sql_group(&mut self, ui: &mut Ui, settings: &mut Settings) {
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
                "Shared snippets",
                Some("A folder of snippets shared with others, such as a network folder."),
                |ui, name_id| {
                    let edit = egui::TextEdit::singleline(&mut settings.sql_snippets_shared)
                        .hint_text("None");
                    theme::sized_edit(ui, 260.0, edit).labelled_by(name_id);
                },
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
    }

    /// What Java `path` finds, looked up only when it changed.
    fn java_for(&mut self, path: &str) -> &std::result::Result<String, String> {
        let stale = self
            .java_found
            .as_ref()
            .is_none_or(|(checked, _)| checked != path);
        if stale {
            let configured = (!path.trim().is_empty()).then(|| std::path::Path::new(path.trim()));
            let result = parley_core::sql::java::find(configured)
                .map(|java| format!("Java {} ({})", java.version, java.path.display()))
                .map_err(|e| e.to_string());
            self.java_found = Some((path.to_owned(), result));
        }
        &self.java_found.as_ref().expect("set just above").1
    }

    /// What this build is, and looking for a new version (R-UI-11), on Linux with the
    /// `curl` command that downloads a package (R-UI-12). As Tortuga's About page.
    fn about_page(&mut self, ui: &mut Ui, settings: &mut Settings, updates: &UpdateStatus) {
        let p = theme::palette(ui);
        let name = self.program.name();
        ui.label(theme::semibold(name).size(18.0));
        ui.label(RichText::new(format!("Version {}", env!("CARGO_PKG_VERSION"))).color(p.muted));
        ui.add_space(14.0);
        group(ui, "About", None, |rows| {
            rows.fact("Built for", |ui| {
                ui.label(format!(
                    "{} {}",
                    std::env::consts::OS,
                    std::env::consts::ARCH
                ));
            });
            rows.fact("Licence", |ui| {
                ui.label(env!("CARGO_PKG_LICENSE").replace(" OR ", " or "));
            });
            rows.fact("Source", |ui| {
                ui.hyperlink(REPOSITORY);
            });
            rows.full_row(|ui| {
                if ui
                    .button("Copy version")
                    .on_hover_text(version_info())
                    .clicked()
                {
                    ui.ctx().copy_text(version_info());
                    self.copied = Some("The version is copied.".to_owned());
                }
            });
        });
        if *updates == UpdateStatus::Unsupported {
            theme::hint(ui, "This build doesn't look for new versions.");
            return;
        }
        group(ui, "Updates", None, |rows| {
            rows.switch(
                &format!("Look for a new version when {name} starts"),
                Some("Asks GitHub for the latest release; sends nothing else."),
                &mut settings.check_for_updates,
            );
            rows.full_row(|ui| self.update_row(ui, updates));
        });
        if let Some(copied) = &self.copied {
            theme::hint(ui, copied);
        }
        // R-SQL-13, D20: the IBM Public License asks for IBM's copyright in plain view.
        group(ui, "Includes", None, |rows| {
            rows.fact("JTOpen 21.0.7", |ui| {
                ui.label(
                    "For the SQL add-on. Copyright © 1997-2026 International Business Machines \
                     Corporation and others. All Rights Reserved. IBM Public License 1.0.",
                );
                ui.hyperlink_to("Its source", "https://github.com/IBM/JTOpen");
            });
        });
    }

    /// "Check now" and what the last check found.
    fn update_row(&mut self, ui: &mut Ui, updates: &UpdateStatus) {
        ui.horizontal(|ui| {
            let checking = *updates == UpdateStatus::Checking;
            if ui
                .add_enabled(!checking, egui::Button::new("Check now"))
                .clicked()
            {
                self.check_requested = true;
            }
            match updates {
                UpdateStatus::Unsupported | UpdateStatus::NotChecked | UpdateStatus::Failed(_) => {}
                UpdateStatus::Checking => {
                    ui.spinner();
                    ui.label("Looking for a new version…");
                }
                UpdateStatus::UpToDate => {
                    ui.label("Parley is up to date.");
                }
                UpdateStatus::Available(release) => {
                    ui.hyperlink_to(
                        format!("Parley {} is available", release.version),
                        &release.page,
                    )
                    .on_hover_text("Opens the release page, with its notes and installers.");
                    let packages = if cfg!(target_os = "linux") {
                        release.linux_packages()
                    } else {
                        Vec::new()
                    };
                    if let Some(command) = download_command_menu(ui, &packages) {
                        ui.ctx().copy_text(command.clone());
                        self.copied = Some(format!("Copied: {command}"));
                    }
                }
            }
        });
        if let UpdateStatus::Failed(e) = updates {
            theme::error(ui, e);
        }
    }

    fn terminal_page(&self, ui: &mut Ui, settings: &mut Settings) {
        let installed = self.installed.get();
        group(ui, "Font", None, |rows| {
            rows.row(
                "Font",
                Some("A bundled font, or a monospace font on this computer."),
                |ui, name| {
                    font_choice(ui, &mut settings.terminal_font, installed.as_deref())
                        .labelled_by(name);
                },
            );
            rows.full_row(|ui| {
                preview(ui);
                theme::hint(
                    ui,
                    "The font's size follows the window: the screen always fills it.",
                );
            });
        });
    }
}

/// The list of pages, `height` tall; a click shows another page.
fn nav(ui: &mut Ui, page: &mut Page, sections: Sections, height: f32) {
    let p = theme::palette(ui);
    Frame::new()
        .fill(p.chrome)
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::same(8))
        .show(ui, |ui| {
            ui.set_width(NAV_WIDTH - 16.0);
            ui.set_height(height - 16.0);
            ui.spacing_mut().item_spacing.y = 2.0;
            for (i, (heading, pages)) in sections.iter().enumerate() {
                if i > 0 {
                    ui.add_space(10.0);
                }
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    theme::section_label(ui, heading);
                });
                ui.add_space(2.0);
                for &value in *pages {
                    nav_item(ui, page, value);
                }
            }
        });
}

fn nav_item(ui: &mut Ui, page: &mut Page, value: Page) {
    let p = theme::palette(ui);
    let chosen = *page == value;
    let name = value.name();
    let text = if chosen {
        theme::semibold(name).color(p.text)
    } else {
        RichText::new(name).color(p.muted)
    };
    let mut button = egui::Button::new((text, egui::Atom::grow()))
        .min_size(vec2(ui.available_width(), 28.0))
        .corner_radius(CornerRadius::same(6));
    button = if chosen {
        button.fill(p.bg).stroke(Stroke::new(1.0, p.border))
    } else {
        button.frame_when_inactive(false)
    };
    let response = ui.add(button);
    response.widget_info(|| egui::WidgetInfo::selected(egui::Role::Tab, true, chosen, name));
    if chosen {
        let r = response.rect;
        let bar = egui::Rect::from_min_size(
            egui::pos2(r.left() + 1.0, r.top() + 7.0),
            vec2(3.0, r.height() - 14.0),
        );
        ui.painter().rect_filled(bar, 1.5, p.accent);
    }
    if response.clicked() {
        *page = value;
    }
}

/// A titled group of rows on a card; `note` is a short line under the title and a longer
/// one on hover, as Tortuga's.
fn group<R>(
    ui: &mut Ui,
    title: &str,
    note: Option<(&str, &str)>,
    add: impl FnOnce(&mut Rows<'_>) -> R,
) -> R {
    let p = theme::palette(ui);
    theme::section_label(ui, title);
    if let Some((short, long)) = note {
        ui.add(egui::Label::new(RichText::new(short).small().color(p.muted)).wrap())
            .on_hover_text(long);
    }
    ui.add_space(4.0);
    let inner = Frame::new()
        .fill(p.bg)
        .stroke(Stroke::new(1.0, p.border))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::symmetric(14, 0))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            let label_width = (ui.available_width() * 0.42).clamp(140.0, 230.0);
            add(&mut Rows {
                ui,
                first: true,
                label_width,
            })
        })
        .inner;
    ui.add_space(18.0);
    inner
}

/// The rows of a [`group`].
struct Rows<'a> {
    ui: &'a mut Ui,
    first: bool,
    label_width: f32,
}

impl Rows<'_> {
    /// A setting: its name and a short line about it on the left, `control` beside them.
    /// `control` gets the name's id, to name its widget with `labelled_by`.
    fn row<R>(
        &mut self,
        name: &str,
        about: Option<&str>,
        control: impl FnOnce(&mut Ui, Id) -> R,
    ) -> R {
        self.divider();
        self.ui.add_space(10.0);
        let width = self.label_width;
        let inner = self
            .ui
            .horizontal_top(|ui| {
                // The name lines up with the text of a button or field beside it.
                let text = ui.text_style_height(&egui::TextStyle::Body);
                let pad = ((ui.spacing().interact_size.y - text) / 2.0).max(0.0);
                let name_id = ui
                    .allocate_ui_with_layout(vec2(width, 0.0), Layout::top_down(Align::Min), |ui| {
                        ui.set_width(width);
                        ui.add_space(pad);
                        let id = ui.label(name).id;
                        if let Some(about) = about {
                            theme::hint(ui, about);
                        }
                        id
                    })
                    .inner;
                ui.add_space(12.0);
                ui.vertical(|ui| control(ui, name_id)).inner
            })
            .inner;
        self.ui.add_space(10.0);
        inner
    }

    /// A setting that is on or off: its name, and a switch named by it.
    fn switch(&mut self, name: &str, about: Option<&str>, on: &mut bool) {
        self.row(name, about, |ui, name_id| {
            theme::switch(ui, on).labelled_by(name_id);
        });
    }

    /// A fact, not a setting: its name and `value` as text side by side, on a lower row.
    fn fact(&mut self, name: &str, value: impl FnOnce(&mut Ui)) {
        self.divider();
        self.ui.add_space(7.0);
        let width = self.label_width;
        let muted = theme::palette(self.ui).muted;
        self.ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(vec2(width, 0.0), Layout::top_down(Align::Min), |ui| {
                ui.set_width(width);
                ui.label(RichText::new(name).color(muted));
            });
            ui.add_space(12.0);
            ui.vertical(value);
        });
        self.ui.add_space(7.0);
    }

    /// A row across the whole card, for what isn't a setting (a preview, a note).
    fn full_row<R>(&mut self, add: impl FnOnce(&mut Ui) -> R) -> R {
        self.divider();
        self.ui.add_space(10.0);
        let inner = add(self.ui);
        self.ui.add_space(10.0);
        inner
    }

    /// A line between rows; none above the first.
    fn divider(&mut self) {
        if std::mem::take(&mut self.first) {
            return;
        }
        let p = theme::palette(self.ui);
        let rect = self.ui.available_rect_before_wrap();
        let y = self.ui.cursor().top();
        self.ui
            .painter()
            .hline(rect.x_range(), y, Stroke::new(1.0, p.border));
    }
}

/// Theme, text size and density; with `sql`, where the SQL window's results go (R-SQL-49).
fn appearance_page(ui: &mut Ui, settings: &mut Settings, sql: bool) {
    group(ui, "Theme", None, |rows| {
        rows.row("Theme", None, |ui, _| {
            theme::segmented(
                ui,
                &mut settings.theme,
                &[
                    (Theme::System, "Like the system"),
                    (Theme::Light, "Light"),
                    (Theme::Dark, "Dark"),
                ],
            );
        });
    });
    group(ui, "Text", None, |rows| {
        rows.row(
            "Text size",
            Some("Everything, the screen included."),
            |ui, name| {
                percent_choice(ui, &mut settings.text_size).labelled_by(name);
            },
        );
        rows.row(
            "Density",
            Some("Room around buttons and fields."),
            |ui, _| {
                theme::segmented(
                    ui,
                    &mut settings.density,
                    &[
                        (Density::Comfortable, "Comfortable"),
                        (Density::Compact, "Compact"),
                    ],
                );
            },
        );
    });
    if sql {
        group(ui, "SQL window", None, |rows| {
            rows.row(
                "Results",
                Some("Beside the editor, or below it; Automatic: beside when the window is wide."),
                |ui, _| {
                    use crate::sql::layout::ResultsPlace;
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
}

/// The text sizes, with `value` in the list even when it isn't one of them.
fn percent_choice(ui: &mut Ui, value: &mut u16) -> egui::Response {
    let mut sizes = TEXT_SIZES.to_vec();
    if !sizes.contains(value) {
        sizes.push(*value);
        sizes.sort_unstable();
    }
    ComboBox::from_id_salt("text_size")
        .selected_text(format!("{value} %"))
        .show_ui(ui, |ui| {
            for size in sizes {
                ui.selectable_value(value, size, format!("{size} %"));
            }
        })
        .response
}

/// The bundled fonts, then the installed monospace ones (`None` while still looking).
fn font_choice(ui: &mut Ui, font: &mut String, installed: Option<&[String]>) -> egui::Response {
    ComboBox::from_id_salt("terminal_font")
        .selected_text(font.as_str())
        .width(240.0)
        .show_ui(ui, |ui| {
            theme::section_label(ui, "Included with Parley");
            for name in fonts::BUNDLED {
                ui.selectable_value(font, name.to_owned(), name);
            }
            ui.separator();
            theme::section_label(ui, "Installed");
            match installed {
                None => {
                    ui.spinner();
                }
                Some([]) => theme::hint(ui, "No other monospace fonts found."),
                Some(names) => {
                    for name in names {
                        ui.selectable_value(font, name.clone(), name);
                    }
                }
            }
        })
        .response
}

/// A few lines of a 5250 screen in the terminal font and colours.
fn preview(ui: &mut Ui) {
    let font = fonts::terminal_font(ui.ctx(), 15.0);
    Frame::new()
        .fill(terminal::BACKGROUND)
        .inner_margin(Margin::same(10))
        .corner_radius(CornerRadius::same(6))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 2.0;
            let line = |ui: &mut Ui, text: &str, color| {
                ui.label(RichText::new(text).font(font.clone()).color(color));
            };
            let white = terminal::color(parley_core::Color::White);
            let green = terminal::color(parley_core::Color::Green);
            line(ui, "            Sign On             ", white);
            line(ui, "System . . . . . :   PUB400     ", green);
            line(ui, "User . . . . . . . . QSECOFR    ", green);
            line(ui, "0Oo 1lI| {}[] æøå ÆØÅ €  ", green);
        });
    ui.add_space(4.0);
}

/// The app's version and platform, for bug reports.
fn version_info() -> String {
    format!(
        "Parley {} ({} {})",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
    )
}

/// "Copy download command", listing `packages` by file name; returns the command picked.
/// Nothing when there are none (R-UI-12).
fn download_command_menu(
    ui: &mut Ui,
    packages: &[&parley_core::update::Download],
) -> Option<String> {
    if packages.is_empty() {
        return None;
    }
    ui.menu_button("Copy download command", |ui| {
        let mut picked = None;
        for package in packages {
            let command = package.curl_command();
            if ui
                .button(&package.name)
                .on_hover_text(command.as_str())
                .clicked()
            {
                picked = Some(command);
            }
        }
        picked
    })
    .inner
    .flatten()
}
