//! Parley's user interface, built with egui.
//!
//! No platform code lives here: the launcher (`parley-app`) opens the window and hands the UI
//! an [`egui::Ui`] to draw in each frame.

mod addons;
mod connect;
mod dialog;
mod fonts;
mod icons;
mod keymap;
mod keys;
mod macros;
mod menu;
mod profile;
mod sessions;
mod settings;
mod shortcuts;
mod sql;
mod sql_app;
mod status;
mod terminal;
mod theme;
mod updates;

use egui::{CentralPanel, Frame, Margin, Panel, Ui, ViewportCommand};
use parley_core::net::{Connection, LinkState};

pub use addons::{AddOn, AddOnSettings};
use keymap::{Function, KeyInput, KeyMap};
use macros::Macros;
pub use macros::{MacroStore, NoMacroStore, StoredMacro, check_name as check_macro_name};
use menu::Action;
pub use profile::Profile;
use sessions::{Editor, Outcome};
pub use settings::{Settings, Theme};
pub use sql::SqlFiles;
pub use sql::assistant::ApiKeys;
pub use sql::editor::SavedScript;
pub use sql::snippets::SnippetStore;
pub use sql_app::{ParleySqlUi, SqlConnection};
pub use theme::Density;
pub use updates::Checker;

/// The whole application UI: menu bar, toolbar and status bar around the start screen or the
/// open session's screen.
#[derive(Debug)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "independent UI flags: panel shown, focus wanted, close confirmed"
)]
pub struct ParleyUi {
    profiles: Vec<Profile>,
    /// The session in use, or the last one used.
    current: Option<Profile>,
    connection: Option<Connection>,
    /// The connection got through: when it ends, the host ended it.
    online: bool,
    /// Why the last connection ended, shown on the start screen and in the status bar.
    closed: Option<String>,
    /// A problem to show next to the keyboard state, such as a macro that cannot be run.
    warning: Option<String>,
    editor: Option<Editor>,
    settings: Settings,
    /// The settings in effect; `None` until the first frame puts them into effect.
    applied: Option<Settings>,
    settings_dialog: Option<settings::Dialog>,
    /// The add-ons in effect: the settings' add-ons as last saved (R-ADD-1).
    addons: AddOnSettings,
    key_input: KeyInput,
    logo: menu::Logo,
    updates: updates::Updates,
    /// Give the terminal the keyboard focus on the next frame.
    focus_terminal: bool,
    /// The keyboard shortcut sheet, while it is open (R-UI-17).
    shortcut_sheet: Option<shortcuts::Sheet>,
    /// The response time and the recording's length in the status bar (R-UI-14).
    clock: status::Clock,
    /// The operator is being asked whether to close the window.
    asking_to_exit: bool,
    /// Closing is decided: the next close request goes through.
    exit_confirmed: bool,
    macros: Macros,
    /// Naming a finished recording (R-KEY-3).
    save_macro: Option<macros::SaveDialog>,
    /// Asking for the password a running macro needs (R-KEY-4).
    macro_password: Option<macros::PasswordDialog>,
    macro_manager: Option<macros::Manager>,
    /// The folder of the SQL helper's jars, if this installation has one (R-SQL-13).
    sql_helpers: Option<std::path::PathBuf>,
    /// The keychain the SQL assistant's API keys are kept in (R-SQL-48).
    api_keys: Option<std::sync::Arc<dyn ApiKeys>>,
    /// The SQL window, while open (R-SQL-6).
    sql_window: Option<sql::SqlWindow>,
    /// The user last typed in the SQL window, by session, for this run of Parley only.
    sql_users: std::collections::BTreeMap<String, String>,
    /// Why the SQL window cannot open (no Java, no helper), while that is shown (R-SQL-12).
    sql_notice: Option<String>,
    /// The file window for exports and scripts (R-SQL-8, R-SQL-21).
    file_saver: Option<Box<dyn sql::SqlFiles>>,
    /// The snippet folders (R-SQL-24); without them, the SQL window has only the starters.
    snippet_store: Option<Box<dyn sql::snippets::SnippetStore>>,
    /// The SQL window's scripts as last kept in the settings (R-SQL-20).
    sql_scripts_kept: u64,
}

impl Default for ParleyUi {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

impl ParleyUi {
    /// Starts on the start screen with the saved sessions.
    pub fn new(profiles: Vec<Profile>) -> Self {
        Self {
            profiles,
            current: None,
            connection: None,
            online: false,
            closed: None,
            warning: None,
            editor: None,
            settings: Settings::default(),
            applied: None,
            settings_dialog: None,
            addons: AddOnSettings::default(),
            key_input: KeyInput::default(),
            logo: menu::Logo::default(),
            updates: updates::Updates::default(),
            focus_terminal: false,
            shortcut_sheet: None,
            clock: status::Clock::default(),
            asking_to_exit: false,
            exit_confirmed: false,
            macros: Macros::new(Box::new(NoMacroStore)),
            save_macro: None,
            macro_password: None,
            macro_manager: None,
            sql_helpers: None,
            api_keys: None,
            sql_window: None,
            sql_users: std::collections::BTreeMap::new(),
            sql_notice: None,
            file_saver: None,
            snippet_store: None,
            sql_scripts_kept: 0,
        }
    }

    /// Where macros are kept (R-KEY-3). Without one, a recording cannot be saved. The folder
    /// is read only while the Macros add-on is on (R-ADD-2), so set the settings first.
    pub fn set_macro_store(&mut self, store: Box<dyn MacroStore>) {
        self.macros = if self.addon_on(AddOn::Macros) {
            Macros::new(store)
        } else {
            Macros::unloaded(store)
        };
    }

    /// The app icon to show before the name in the menu bar (the launcher has it as a PNG).
    pub fn set_logo(&mut self, image: egui::ColorImage) {
        self.logo = menu::Logo::new(image);
    }

    /// How the SQL window reaches files (R-SQL-8, R-SQL-21); without it, no Export or Open….
    pub fn set_file_saver(&mut self, saver: Box<dyn sql::SqlFiles>) {
        self.file_saver = Some(saver);
    }

    /// Where the SQL window keeps snippets (R-SQL-24).
    pub fn set_snippet_store(&mut self, store: Box<dyn sql::snippets::SnippetStore>) {
        self.snippet_store = Some(store);
    }

    /// Where the SQL helper's jars are (R-SQL-13); `None` when this installation has none.
    pub fn set_sql_helpers(&mut self, folder: Option<std::path::PathBuf>) {
        self.sql_helpers = folder;
    }

    /// The keychain for the SQL assistant's API keys (R-SQL-48); without one, keys come only
    /// from the environment.
    pub fn set_api_keys(&mut self, keys: std::sync::Arc<dyn ApiKeys>) {
        self.api_keys = Some(keys);
    }

    /// How to ask for the latest release (R-UI-11). Without one, Parley doesn't look for
    /// new versions (tests, and builds that shouldn't).
    pub fn set_update_checker(&mut self, checker: Checker) {
        self.updates.set_checker(checker);
    }

    /// The settings, for the launcher to keep.
    pub const fn settings(&self) -> &Settings {
        &self.settings
    }

    /// Replaces the settings; they take effect on the next frame, the add-ons at once.
    pub fn set_settings(&mut self, settings: Settings) {
        let addons = settings.addons.clone();
        self.settings = settings;
        self.set_addons(addons);
    }

    /// Whether `addon` is on (R-ADD-1): the one question the UI asks about an add-on.
    fn addon_on(&self, addon: AddOn) -> bool {
        self.addons.enabled(addon)
    }

    /// Puts `addons` into effect: an add-on turned on is loaded, one turned off is unloaded.
    fn set_addons(&mut self, addons: AddOnSettings) {
        let was = self.addon_on(AddOn::Macros);
        self.addons = addons;
        match (was, self.addon_on(AddOn::Macros)) {
            (false, true) => self.macros.reload(),
            (true, false) => self.macros_off(),
            (true, true) | (false, false) => {}
        }
    }

    /// The saved sessions, for the launcher to keep.
    pub fn profiles(&self) -> &[Profile] {
        &self.profiles
    }

    /// The saved session marked "connect on program start", if any.
    pub fn startup_profile(&self) -> Option<&Profile> {
        self.profiles.iter().find(|p| p.connect_on_start)
    }

    /// The saved session for `host`, if there is one.
    pub fn profile_for_host(&self, host: &str) -> Option<&Profile> {
        self.profiles
            .iter()
            .find(|p| p.config.host.eq_ignore_ascii_case(host))
    }

    /// The window title for the current state.
    pub fn title(&self) -> String {
        match (&self.connection, &self.current) {
            (Some(_), Some(profile)) => format!("{} – Parley", profile.name),
            _ => "Parley".to_owned(),
        }
    }

    /// Connects to `profile`. The operator signs on: Parley never does (R-NET-7).
    pub fn connect(&mut self, ctx: &egui::Context, profile: Profile) {
        self.warning = None;
        let ctx = ctx.clone();
        let connection = Connection::open(profile.config.clone(), move || ctx.request_repaint());
        if self.addon_on(AddOn::Macros) && !profile.run_macro.is_empty() {
            self.play(&mut connection.session(), &profile.run_macro);
        }
        self.connection = Some(connection);
        self.current = Some(profile);
        self.online = false;
        self.closed = None;
        self.focus_terminal = true;
    }

    fn connect_saved(&mut self, ctx: &egui::Context, index: usize) {
        if let Some(profile) = self.profiles.get(index).cloned() {
            self.connect(ctx, profile);
        }
    }

    /// The status bar (R-UI-14); what was clicked in it.
    fn status_panel(
        &self,
        ui: &mut Ui,
        frame: Frame,
        session: Option<&parley_core::Session>,
        link: Option<&LinkState>,
    ) -> Option<status::StatusAction> {
        let maps: Vec<String> = std::iter::once(KeyMap::built_in())
            .chain(&self.settings.keyboard_maps)
            .map(|m| m.name.clone())
            .collect();
        let status = status::Status {
            session,
            link,
            config: self.current.as_ref().map(|p| &p.config),
            note: self.closed.as_deref().or(self.warning.as_deref()),
            macros: self.addon_on(AddOn::Macros),
            can_connect_again: self.current_index().is_some(),
            map: &self.keyboard_map().name,
            keymap: self.keyboard_map(),
            mac: is_mac(ui.ctx()),
            maps: &maps,
            clock: &self.clock,
        };
        Panel::bottom("status_bar")
            .frame(frame)
            .show(ui, |ui| status::status_bar(ui, &status))
            .inner
    }

    /// The keys of this frame may have locked the keyboard after the status bar was drawn:
    /// note it now, and draw again at once so the hourglass shows while the host works
    /// (R-UI-14). Nothing else asks for a frame until the host answers.
    fn observe_keys_taken(&mut self, ctx: &egui::Context, session: Option<&parley_core::Session>) {
        let was_waiting = self.clock.waiting();
        self.clock.observe(session, std::time::Instant::now());
        if self.clock.waiting() != was_waiting {
            ctx.request_repaint();
        }
    }

    /// Carries out what was clicked in the status bar.
    fn status_action(&mut self, action: status::StatusAction, actions: &mut Vec<Action>) {
        match action {
            status::StatusAction::StopRecording => actions.push(Action::StopRecording),
            status::StatusAction::ConnectAgain => {
                if let Some(index) = self.current_index() {
                    actions.push(Action::Connect(index));
                }
            }
            status::StatusAction::UseMap(name) => self.use_keyboard_map(&name),
            status::StatusAction::Key(key) => actions.push(Action::Key(key)),
        }
    }

    /// The session in use, and its saved copy, use the keyboard map `name` (R-KEY-2).
    fn use_keyboard_map(&mut self, name: &str) {
        let index = self.current_index();
        if let Some(current) = self.current.as_mut() {
            name.clone_into(&mut current.keyboard_map);
        }
        if let Some(saved) = index.and_then(|i| self.profiles.get_mut(i)) {
            name.clone_into(&mut saved.keyboard_map);
        }
    }

    /// The keyboard map of the session in use (R-KEY-2).
    fn keyboard_map(&self) -> &KeyMap {
        let name = self
            .current
            .as_ref()
            .map_or("", |p| p.keyboard_map.as_str());
        KeyMap::find(&self.settings.keyboard_maps, name)
    }

    /// The settings dialog, showing the session's keyboard map and the macros it can bind.
    fn open_settings(&self, ctx: &egui::Context) -> settings::Dialog {
        let macros = self.addon_on(AddOn::Macros).then(|| self.macro_names());
        settings::Dialog::open(ctx, &self.settings)
            .with_keyboard(&self.keyboard_map().name, macros, self.addon_on(AddOn::Sql))
            .with_sql_helpers(self.sql_helpers.clone())
            .with_api_keys(self.api_keys.clone())
    }

    /// Sessions that used the map `old` use it under its new name.
    fn rename_keyboard_map(&mut self, old: &str, new: &str) {
        let profiles = self.profiles.iter_mut().chain(self.current.as_mut());
        for profile in profiles.filter(|p| p.keyboard_map == old) {
            new.clone_into(&mut profile.keyboard_map);
        }
    }

    fn open_editor(&mut self, index: Option<usize>) {
        self.editor = Some(Editor::open(&self.profiles, index));
    }

    /// The index of the session in use among the saved ones.
    fn current_index(&self) -> Option<usize> {
        let current = self.current.as_ref()?;
        self.profiles.iter().position(|p| p.name == current.name)
    }

    /// Draws one frame.
    pub fn ui(&mut self, ui: &mut Ui) {
        // Ctrl+Plus and Ctrl+Minus are Field+ and Field- here, not zoom.
        if ui.ctx().options(|o| o.zoom_with_keyboard) {
            ui.ctx().options_mut(|o| o.zoom_with_keyboard = false);
        }
        if self.applied.is_none() && self.settings.check_for_updates {
            self.updates.start(ui.ctx(), updates::Check::AtStart);
        }
        self.updates.collect();
        if self.applied.as_ref() != Some(&self.settings) {
            let note = settings::apply(ui.ctx(), self.applied.as_ref(), &self.settings);
            if note.is_some() {
                self.warning = note;
            }
            self.applied = Some(self.settings.clone());
        }
        self.follow_link(ui.ctx());
        self.guard_exit(ui.ctx());

        let connection = self.connection.take();
        let link = connection.as_ref().map(Connection::state);
        let mut session = connection.as_ref().map(Connection::session);
        self.clock
            .observe(session.as_deref(), std::time::Instant::now());
        let mut actions = Vec::new();

        // Made before the panels borrow `ui`.
        let menu_frame = theme::bar(ui, Margin::symmetric(8, 3));
        let status_frame = theme::bar(ui, Margin::symmetric(8, 5));
        actions.extend(self.menu_panel(ui, menu_frame, session.as_deref(), link.as_ref()));
        if let Some(chosen) = self.status_panel(ui, status_frame, session.as_deref(), link.as_ref())
        {
            self.status_action(chosen, &mut actions);
        }
        let mut asked = Vec::new();
        if let Some(session) = session.as_deref_mut() {
            CentralPanel::default()
                .frame(
                    Frame::new()
                        .fill(terminal::BACKGROUND)
                        .inner_margin(Margin::same(14)),
                )
                .show(ui, |ui| {
                    // The dialog has the keyboard while it is open.
                    let grab = self.editor.is_none() && std::mem::take(&mut self.focus_terminal);
                    let map = KeyMap::find(
                        &self.settings.keyboard_maps,
                        self.current
                            .as_ref()
                            .map_or("", |p| p.keyboard_map.as_str()),
                    );
                    asked = terminal::show(ui, session, &mut self.key_input, map, grab);
                });
        } else {
            CentralPanel::default().show(ui, |ui| {
                let entries: Vec<_> = self.profiles.iter().map(connect::Entry::session).collect();
                let words = connect::SESSIONS;
                match connect::show(ui, words, &entries, self.closed.as_deref()) {
                    Some(connect::Choice::Connect(i)) => actions.push(Action::Connect(i)),
                    Some(connect::Choice::Edit(i)) => self.open_editor(Some(i)),
                    Some(connect::Choice::New) => actions.push(Action::NewSession),
                    None => {}
                }
            });
        }

        let next = self.carry_out(ui.ctx(), actions, session.as_deref_mut());
        if let Some(session) = session.as_deref_mut() {
            self.carry_out_keys(asked, session);
        }
        self.observe_keys_taken(ui.ctx(), session.as_deref());
        if let Some(session) = session.as_deref_mut() {
            self.macro_needs_password(session);
        }
        drop(session);
        match connection {
            Some(connection) if next.disconnect => {
                self.end_recording(&mut connection.session());
                connection.close();
                self.closed = Some("Disconnected.".to_owned());
                // R-SQL-6: the SQL window goes with its session, asking first under R-SQL-5.
                if let Some(window) = &mut self.sql_window {
                    window.request_close();
                }
            }
            other => self.connection = other,
        }
        // Only now: the connection taken for this frame is back, so a new one is not overwritten.
        if let Some(index) = next.connect {
            self.connect_saved(ui.ctx(), index);
        }

        self.show_editor(ui.ctx());
        self.show_settings(ui.ctx());
        self.show_sql_window(ui.ctx());
        self.show_shortcut_sheet(ui.ctx());
        self.show_macro_dialogs(ui.ctx());
        self.show_exit_question(ui.ctx());
    }

    /// The header (R-UI-15); returns what was chosen from it.
    fn menu_panel(
        &mut self,
        ui: &mut Ui,
        frame: Frame,
        session: Option<&parley_core::Session>,
        link: Option<&LinkState>,
    ) -> Option<Action> {
        let macro_names = self.macro_names();
        let macro_menu = menu::MacroMenu {
            state: session.map(parley_core::Session::macro_state),
            names: &macro_names,
        };
        let logo = self.logo.texture(ui.ctx()).cloned();
        let header = menu::Header {
            link,
            profiles: &self.profiles,
            current: self.current.as_ref(),
            logo: logo.as_ref(),
            newer: self.updates.available(),
            macros: self.addon_on(AddOn::Macros).then_some(&macro_menu),
            sql: self.addon_on(AddOn::Sql),
            map: self.keyboard_map(),
            mac: is_mac(ui.ctx()),
        };
        Panel::top("menu_bar")
            .frame(frame)
            .show(ui, |ui| menu::menu_bar(ui, &header))
            .inner
    }

    /// Does what the header, key strip, status bar and start screen asked, except opening and closing the
    /// connection, which [`Self::ui`] does once the frame is done with the current one.
    fn carry_out(
        &mut self,
        ctx: &egui::Context,
        actions: Vec<Action>,
        mut session: Option<&mut parley_core::Session>,
    ) -> Next {
        let connected = session.is_some();
        let mut next = Next::default();
        for action in actions {
            match action {
                Action::NewSession => self.open_editor(None),
                Action::EditSessions => {
                    let index = self
                        .current_index()
                        .or((!self.profiles.is_empty()).then_some(0));
                    self.open_editor(index);
                }
                Action::Connect(i) => {
                    // From the session pill: the open session closes first.
                    next.disconnect = connected;
                    next.connect = Some(i);
                }
                Action::Disconnect => next.disconnect = true,
                Action::Exit => ctx.send_viewport_cmd(ViewportCommand::Close),
                Action::CopyScreen => {
                    if let Some(s) = session.as_deref() {
                        ctx.copy_text(s.screen().text(s.code_page()));
                    }
                }
                Action::Paste => {
                    // The pasted text arrives as an input event, which the terminal takes.
                    ctx.send_viewport_cmd(ViewportCommand::RequestPaste);
                    self.focus_terminal = true;
                }
                Action::Settings => self.settings_dialog = Some(self.open_settings(ctx)),
                Action::CheckForUpdates => {
                    let dialog = self.open_settings(ctx).on_about_page();
                    self.settings_dialog = Some(dialog);
                    self.updates.start(ctx, updates::Check::Now);
                }
                Action::Shortcuts => self.shortcut_sheet = Some(shortcuts::Sheet::default()),
                Action::Sql => self.open_sql_window(connected),
                Action::Key(key) => {
                    if let Some(s) = session.as_deref_mut() {
                        s.key(key);
                        self.focus_terminal = true;
                        ctx.request_repaint();
                    }
                }
                Action::RecordMacro => {
                    if let Some(s) = session.as_deref_mut() {
                        s.start_recording();
                        self.focus_terminal = true;
                    }
                }
                Action::StopRecording => {
                    if let Some(s) = session.as_deref_mut() {
                        self.end_recording(s);
                    }
                }
                Action::RunMacro(i) => {
                    let name = self.macro_names().get(i).cloned();
                    if let (Some(s), Some(name)) = (session.as_deref_mut(), name) {
                        self.play(s, &name);
                        self.focus_terminal = true;
                    }
                }
                // Reset stops a running macro, as from the keyboard.
                Action::StopMacro => {
                    if let Some(s) = session.as_deref_mut() {
                        s.key(parley_core::Key::Reset);
                    }
                }
                Action::Macros => {
                    self.macros.reload();
                    self.macro_manager = Some(macros::Manager::default());
                }
            }
        }
        next
    }

    /// Notices the connection coming up or ending; a host that ends the session closes the
    /// window if the session says so.
    fn follow_link(&mut self, ctx: &egui::Context) {
        match self.connection.as_ref().map(Connection::state) {
            Some(LinkState::Connected) => self.online = true,
            Some(LinkState::Closed(reason)) => {
                if let Some(connection) = self.connection.take() {
                    self.end_recording(&mut connection.session());
                }
                self.closed = Some(reason);
                let exit = self.current.as_ref().is_some_and(|p| p.exit_on_close);
                if std::mem::take(&mut self.online) && exit {
                    self.exit_confirmed = true;
                    ctx.send_viewport_cmd(ViewportCommand::Close);
                }
            }
            Some(LinkState::Connecting) | None => {}
        }
    }

    /// Holds a close request while connected to a session with "Confirm exit", to ask first.
    fn guard_exit(&mut self, ctx: &egui::Context) {
        let wants_close = ctx.input(|i| i.viewport().close_requested());
        let confirm = self.current.as_ref().is_some_and(|p| p.confirm_exit);
        if wants_close && self.connection.is_some() && confirm && !self.exit_confirmed {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            self.asking_to_exit = true;
        }
    }

    fn show_exit_question(&mut self, ctx: &egui::Context) {
        if !self.asking_to_exit {
            return;
        }
        let name = self.current.as_ref().map_or("", |p| p.name.as_str());
        let (close, (cancel, exit)) =
            dialog::dialog(ctx, "confirm_exit", "Exit Parley?", 380.0, |ui| {
                ui.label(format!("The session \"{name}\" is still connected."));
                dialog::actions(ui, theme::primary_button(ui, "Exit"), true)
            });
        if close || cancel {
            self.asking_to_exit = false;
        }
        if exit {
            self.asking_to_exit = false;
            self.exit_confirmed = true;
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }
    }

    /// Tools ▸ SQL… (R-SQL-6): the window for the session in use, or why it cannot open.
    /// `connected` comes from the caller: during a frame the connection is out of its field.
    fn open_sql_window(&mut self, connected: bool) {
        if !connected || !self.addon_on(AddOn::Sql) || self.sql_window.is_some() {
            return;
        }
        let Some(profile) = self.current.as_ref() else {
            return;
        };
        let user = self.sql_users.get(&profile.name).map_or("", String::as_str);
        match sql::hosting::open(
            &profile.name,
            &profile.config,
            self.sql_helpers.as_deref(),
            user,
            &self.settings,
        ) {
            Ok(window) => self.sql_window = Some(window),
            Err(e) => self.sql_notice = Some(e),
        }
    }

    /// The SQL window while open, its statements into the history (R-SQL-7), and the notice
    /// when it could not open.
    fn show_sql_window(&mut self, ctx: &egui::Context) {
        let assistant_on = self.addon_on(AddOn::Assistant);
        if let Some(window) = &mut self.sql_window {
            let session = self
                .current
                .as_ref()
                .map_or_else(String::new, |p| p.name.clone());
            let history = self
                .settings
                .sql_history
                .get(&session)
                .cloned()
                .unwrap_or_default();
            // R-SQL-43: the saved sessions "Run on…" offers.
            let systems: Vec<(String, parley_core::SessionConfig)> = self
                .profiles
                .iter()
                .map(|p| (p.name.clone(), p.config.clone()))
                .collect();
            let around = sql::Around {
                history: &history,
                files: self.file_saver.as_deref(),
                snippets: self.snippet_store.as_deref(),
                shared: &self.settings.sql_snippets_shared,
                systems: Some(&systems),
                results: self.settings.sql_results,
            };
            window.set_assistant(
                assistant_on,
                &self.settings.assistant,
                self.api_keys.as_ref(),
            );
            let open = window.show(ctx, around);
            if !window.user.trim().is_empty() {
                self.sql_users
                    .insert(session.clone(), window.user.trim().to_owned());
            }
            sql::hosting::keep(
                window,
                &session,
                &mut self.settings,
                &mut self.sql_scripts_kept,
            );
            if !open {
                self.sql_window = None;
            }
        }
        sql::hosting::notice_dialog(ctx, &mut self.sql_notice);
    }

    /// The keyboard shortcut sheet, while it is open (R-UI-17).
    fn show_shortcut_sheet(&mut self, ctx: &egui::Context) {
        if self.shortcut_sheet.is_none() {
            return;
        }
        let macros = self.addon_on(AddOn::Macros).then(|| self.macro_names());
        let map = self.keyboard_map();
        let groups = shortcuts::groups(
            map,
            macros.as_deref(),
            self.addon_on(AddOn::Sql),
            is_mac(ctx),
        );
        let close_keys = map.chords_for(&Function::Shortcuts);
        let Some(sheet) = &mut self.shortcut_sheet else {
            return;
        };
        match sheet.show(ctx, &groups, &close_keys) {
            shortcuts::Outcome::Open => {}
            shortcuts::Outcome::Close => self.shortcut_sheet = None,
            shortcuts::Outcome::ChangeKeys => {
                self.shortcut_sheet = None;
                self.settings_dialog = Some(self.open_settings(ctx).on_keyboard_page());
            }
        }
    }

    fn show_settings(&mut self, ctx: &egui::Context) {
        let Some(dialog) = &mut self.settings_dialog else {
            return;
        };
        let closed = dialog.show(ctx, &mut self.settings, self.updates.status());
        if dialog.take_check_request() {
            self.updates.start(ctx, updates::Check::Now);
        }
        match closed {
            Some(settings::Closed::Keep) => {
                for (old, new) in dialog.take_map_renames() {
                    self.rename_keyboard_map(&old, &new);
                }
                self.settings_dialog = None;
                self.set_addons(self.settings.addons.clone());
            }
            Some(settings::Closed::Revert) => {
                self.settings = dialog.original().clone();
                self.settings_dialog = None;
            }
            None => {}
        }
    }

    fn show_editor(&mut self, ctx: &egui::Context) {
        let macros_on = self.addon_on(AddOn::Macros);
        let Some(editor) = &mut self.editor else {
            return;
        };
        let connected = self.connection.is_some();
        let outcome = editor.show(
            ctx,
            &mut self.profiles,
            connected,
            macros_on.then_some(&self.macros),
            &self.settings.keyboard_maps,
        );
        if let Some((old, new)) = editor.take_renamed()
            && let Err(e) = self.macros.rename_session(&old, &new)
        {
            self.warning = Some(format!("Could not move the macros of \"{old}\": {e}"));
        }
        match outcome {
            Some(Outcome::Close) => self.editor = None,
            Some(Outcome::Connect(index)) => {
                self.editor = None;
                self.connect_saved(ctx, index);
            }
            None => {}
        }
    }
}

/// Macros (R-KEY-3, R-KEY-4).
impl ParleyUi {
    /// The keys of a map that do something with macros (R-KEY-2): Record macro starts or
    /// stops a recording, a macro's key runs it. Nothing while the add-on is off (R-ADD-2).
    fn carry_out_keys(&mut self, asked: Vec<Function>, session: &mut parley_core::Session) {
        if !self.addon_on(AddOn::Macros) {
            return;
        }
        for function in asked {
            match function {
                Function::RecordMacro => {
                    if session.macro_state() == parley_core::MacroState::Recording {
                        self.end_recording(session);
                    } else {
                        session.start_recording();
                    }
                }
                Function::RunMacro(name) => self.play(session, &name),
                Function::Shortcuts => self.shortcut_sheet = Some(shortcuts::Sheet::default()),
                // The terminal took the key, so a session is connected.
                Function::Sql => self.open_sql_window(true),
                Function::Key(_) | Function::CopyScreen | Function::Paste => {}
            }
        }
    }

    /// The macros the session in use can run, as the Tools menu lists them.
    fn macro_names(&self) -> Vec<String> {
        self.macros
            .names_for(self.current.as_ref().map(|p| p.name.as_str()))
    }

    /// Starts the saved macro `name` on `session`.
    fn play(&mut self, session: &mut parley_core::Session, name: &str) {
        match self.macros.find(name).map(|m| &m.contents) {
            Some(Ok(m)) => session.play(name, m.steps.clone()),
            Some(Err(e)) => self.warning = Some(format!("Macro \"{name}\" cannot be run: {e}")),
            None => self.warning = Some(format!("There is no macro called \"{name}\".")),
        }
    }

    /// The Macros add-on was turned off (R-ADD-2): stops a running macro, ends a recording
    /// (asking for its name, so nothing is lost), closes the Macros dialog and forgets the list.
    fn macros_off(&mut self) {
        self.macros.unload();
        self.macro_manager = None;
        self.macro_password = None;
        let connection = self.connection.take();
        if let Some(connection) = &connection {
            let mut session = connection.session();
            if session.playing().is_some() {
                // Reset stops a running macro without reaching the host, as Stop macro does.
                session.key(parley_core::Key::Reset);
            }
            self.end_recording(&mut session);
            drop(session);
        }
        self.connection = connection;
    }

    /// Stops recording, if the session was, and asks for a name for the recording.
    fn end_recording(&mut self, session: &mut parley_core::Session) {
        if let Some(recorded) = session.stop_recording() {
            let in_session = self
                .current
                .as_ref()
                .filter(|c| self.profiles.iter().any(|p| p.name == c.name))
                .map(|c| c.name.clone());
            self.save_macro = Some(macros::SaveDialog::new(recorded, in_session));
        }
    }

    /// A macro is at a `password` step: ask for it, as Parley keeps none (R-NET-7).
    fn macro_needs_password(&mut self, session: &parley_core::Session) {
        if session.macro_state() != parley_core::MacroState::NeedsPassword
            || self.macro_password.is_some()
        {
            return;
        }
        let name = session.playing().unwrap_or_default();
        self.macro_password = Some(macros::PasswordDialog::new(name));
    }

    fn show_macro_dialogs(&mut self, ctx: &egui::Context) {
        if let Some(dialog) = &mut self.save_macro
            && dialog.show(ctx, &mut self.macros)
        {
            self.save_macro = None;
            self.focus_terminal = true;
        }
        if let Some(dialog) = &mut self.macro_manager {
            let closed = dialog.show(ctx, &mut self.macros);
            if let Some((old, new)) = dialog.take_renamed() {
                for map in &mut self.settings.keyboard_maps {
                    map.rename_macro(&old, &new);
                }
            }
            if closed {
                self.macro_manager = None;
            }
        }
        if let Some(dialog) = &mut self.macro_password
            && let Some(answer) = dialog.show(ctx)
        {
            self.macro_password = None;
            self.focus_terminal = true;
            if let Some(connection) = &self.connection {
                let password = match answer {
                    macros::PasswordAnswer::Typed(p) => {
                        Some(parley_core::password::Password::new(p))
                    }
                    macros::PasswordAnswer::Cancelled => None,
                };
                connection.session().give_macro_password(password.as_ref());
            }
        }
    }
}

/// What to do with the connection once a frame is drawn.
#[derive(Debug, Default)]
struct Next {
    disconnect: bool,
    /// Connect to the saved session at this index.
    connect: Option<usize>,
}

/// Whether to name keys as macOS does (Cmd+C).
fn is_mac(ctx: &egui::Context) -> bool {
    ctx.os() == egui::os::OperatingSystem::Mac
}

/// Sessions for the UI tests.
#[cfg(test)]
pub(crate) mod test_support {
    use parley_core::record::{self, Opcode};
    use parley_core::telnet::Telnet;
    use parley_core::{CodePage, Session, SessionConfig};
    use std::cell::RefCell;
    use std::rc::Rc;

    use parley_core::macros::{Macro, Step};

    use crate::{MacroStore, Profile, StoredMacro};

    /// A keychain in memory, for tests.
    #[derive(Default)]
    pub struct FakeKeys(
        pub  std::sync::Mutex<
            std::collections::HashMap<parley_core::sql::assistant::Provider, String>,
        >,
    );

    impl std::fmt::Debug for FakeKeys {
        /// Like a real keychain's: the keys are not shown.
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("FakeKeys")
        }
    }

    impl crate::ApiKeys for FakeKeys {
        fn get(
            &self,
            provider: parley_core::sql::assistant::Provider,
        ) -> Result<Option<String>, String> {
            Ok(self
                .0
                .lock()
                .map_err(|e| e.to_string())?
                .get(&provider)
                .cloned())
        }
        fn set(
            &self,
            provider: parley_core::sql::assistant::Provider,
            key: &str,
        ) -> Result<(), String> {
            self.0
                .lock()
                .map_err(|e| e.to_string())?
                .insert(provider, key.to_owned());
            Ok(())
        }
        fn delete(&self, provider: parley_core::sql::assistant::Provider) -> Result<(), String> {
            self.0.lock().map_err(|e| e.to_string())?.remove(&provider);
            Ok(())
        }
    }

    /// Parley's fonts, once per context, for a widget tested on its own: the icons and
    /// `theme::semibold` need them.
    pub fn install_fonts(ui: &egui::Ui) {
        let id = egui::Id::unique("test_support::install_fonts");
        let done = ui
            .ctx()
            .data_mut(|d| std::mem::replace(d.get_temp_mut_or_default::<bool>(id), true));
        if !done {
            crate::theme::install(ui.ctx(), crate::fonts::DEFAULT_TERMINAL_FONT);
        }
    }

    /// A session showing "Hello" and one input field at row 2, column 2, waiting for input.
    pub fn waiting_session() -> Session {
        let mut session = Session::new(SessionConfig::default());
        session.receive(&waiting_screen());
        session
    }

    /// The host's record behind [`waiting_session`]: the screen, then a read that unlocks the
    /// keyboard.
    pub fn waiting_screen() -> Vec<u8> {
        let mut data = vec![0x04, 0x40, 0x04, 0x11, 0x00, 0x00, 0x11, 1, 1];
        data.extend("Hello".chars().filter_map(|c| CodePage::Ccsid37.to_byte(c)));
        data.extend([
            0x11, 2, 1, 0x1D, 0x40, 0x00, 0x24, 0x00, 0x0A, 0x04, 0x52, 0x00, 0x00,
        ]);
        Telnet::frame(&record::build(0, Opcode::PutGet, &data))
    }

    /// Macros in memory, shared with the test so it can look inside.
    #[derive(Debug, Clone, Default)]
    pub struct MemoryMacros(pub Rc<RefCell<std::collections::BTreeMap<String, Macro>>>);

    impl MacroStore for MemoryMacros {
        fn list(&self) -> Result<Vec<StoredMacro>, String> {
            Ok(self
                .0
                .borrow()
                .iter()
                .map(|(name, m)| StoredMacro {
                    name: name.clone(),
                    contents: Ok(m.clone()),
                })
                .collect())
        }

        fn read_text(&self, name: &str) -> Result<String, String> {
            let macros = self.0.borrow();
            let m = macros.get(name).ok_or("no such macro")?;
            Ok(m.to_text())
        }

        fn save_text(&self, name: &str, text: &str) -> Result<(), String> {
            let m = Macro::parse(text).map_err(|e| e.to_string())?;
            self.0.borrow_mut().insert(name.to_owned(), m);
            Ok(())
        }

        fn delete(&self, name: &str) -> Result<(), String> {
            self.0.borrow_mut().remove(name);
            Ok(())
        }

        fn rename(&self, old: &str, new: &str) -> Result<(), String> {
            let m = self.0.borrow_mut().remove(old).ok_or("no such macro")?;
            self.0.borrow_mut().insert(new.to_owned(), m);
            Ok(())
        }

        fn folder(&self) -> Option<String> {
            Some("/memory".to_owned())
        }

        /// As if the file `Imported.macro` was chosen.
        fn import(&self) -> Vec<Result<String, String>> {
            self.0
                .borrow_mut()
                .insert("Imported".to_owned(), Macro::default());
            vec![Ok("Imported".to_owned())]
        }

        /// As if the copy was saved in `/elsewhere`.
        fn export(&self, name: &str) -> Result<Option<String>, String> {
            Ok(Some(format!("/elsewhere/{name}.macro")))
        }
    }

    /// A store holding one macro for every session, called `name`.
    pub fn macros_with(name: &str) -> MemoryMacros {
        let store = MemoryMacros::default();
        store.0.borrow_mut().insert(
            name.to_owned(),
            Macro {
                session: None,
                steps: vec![Step::Text("wrkactjob".into()), Step::Wait],
            },
        );
        store
    }

    pub fn profile(name: &str, host: &str) -> Profile {
        Profile {
            name: name.to_owned(),
            config: SessionConfig {
                host: host.to_owned(),
                ..SessionConfig::default()
            },
            ..Profile::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::Harness;
    use egui_kittest::kittest::{By, Queryable};

    use test_support::profile;

    /// Frames `Harness::run` may take to settle: the connection thread and the search for
    /// installed fonts ask for repaints of their own, so a busy machine needs a few more than
    /// kittest's default of 4.
    const SETTLE_STEPS: u64 = 16;

    fn app_with(profiles: Vec<Profile>) -> Harness<'static, ParleyUi> {
        let app = ParleyUi::new(profiles);
        Harness::builder()
            .with_size(egui::vec2(1000.0, 800.0))
            .with_max_steps(SETTLE_STEPS)
            .build_ui_state(|ui, app: &mut ParleyUi| app.ui(ui), app)
    }

    fn type_into(harness: &mut Harness<'static, ParleyUi>, label: &str, text: &str) {
        harness.get_by_label(label).focus();
        harness.run();
        harness.get_by_label(label).type_text(text);
        harness.run();
    }

    fn choose_terminal_font(harness: &mut Harness<'static, ParleyUi>, font: &str) {
        harness.get_by_label("Tools").click();
        harness.run();
        harness.get_by_label("Settings…").click();
        harness.run();
        choose_terminal_font_on_open_dialog(harness, font);
    }

    fn choose_terminal_font_on_open_dialog(harness: &mut Harness<'static, ParleyUi>, font: &str) {
        harness.get_by_label("Terminal").click();
        harness.run();
        harness.get_by_label("Font").click();
        // The list may still show a spinner while installed fonts are found.
        harness.run_steps(2);
        harness.get_by_label(font).click();
        harness.run();
    }

    #[test]
    fn the_terminal_font_is_chosen_in_the_settings_and_save_keeps_it() {
        // R-UI-6
        let mut harness = app_with(Vec::new());
        choose_terminal_font(&mut harness, "JetBrains Mono");
        assert_eq!(harness.state().settings().terminal_font, "JetBrains Mono");
        harness.get_by_label("Save").click();
        harness.run();
        assert!(harness.state().settings_dialog.is_none());
        assert_eq!(harness.state().settings().terminal_font, "JetBrains Mono");
    }

    #[test]
    fn the_close_button_of_the_settings_works_like_cancel() {
        // R-UI-7
        let mut harness = app_with(Vec::new());
        choose_terminal_font(&mut harness, "Hack");
        harness.get_by_label("Close").click();
        harness.run();
        assert!(harness.state().settings_dialog.is_none());
        assert_eq!(harness.state().settings(), &Settings::default());
    }

    #[test]
    fn the_close_button_of_the_session_dialog_closes_it_unsaved() {
        // R-UI-7
        let mut harness = app_with(Vec::new());
        harness.get_by_label("New session…").click();
        harness.run();
        type_into(&mut harness, "Name", "Unsaved");
        harness.get_by_label("Close").click();
        harness.run();
        assert!(harness.state().editor.is_none());
        assert_eq!(harness.state().profiles(), []);
    }

    #[test]
    fn the_shortcut_sheet_closes_with_its_close_buttons_r_ui_17() {
        // R-UI-7: the 🗙 in the title row and Close at the bottom.
        for which in 0..2 {
            let mut harness = app_with(Vec::new());
            harness.state_mut().shortcut_sheet = Some(shortcuts::Sheet::default());
            harness.run();
            let close = harness.get_all_by_label("Close").nth(which).expect("two");
            close.click();
            harness.run();
            assert!(harness.state().shortcut_sheet.is_none(), "button {which}");
        }
    }

    #[test]
    fn the_shortcut_sheet_closes_with_esc_but_not_on_the_frame_it_opens_r_ui_17() {
        let mut harness = app_with(Vec::new());
        harness.state_mut().shortcut_sheet = Some(shortcuts::Sheet::default());
        harness.key_press(egui::Key::Escape);
        harness.run();
        assert!(
            harness.state().shortcut_sheet.is_some(),
            "the opening frame"
        );
        harness.key_press(egui::Key::Escape);
        harness.run();
        assert!(harness.state().shortcut_sheet.is_none());
    }

    #[test]
    fn change_keys_in_the_sheet_opens_settings_on_the_keyboard_page_r_ui_17() {
        let mut harness = app_with(Vec::new());
        harness.state_mut().shortcut_sheet = Some(shortcuts::Sheet::default());
        harness.run();
        harness.get_by_label("Change keys…").click();
        harness.run();
        assert!(harness.state().shortcut_sheet.is_none());
        assert!(harness.state().settings_dialog.is_some());
    }

    #[test]
    fn restore_defaults_resets_only_the_page_shown() {
        // R-UI-8: the theme (Appearance) survives restoring the Terminal page.
        let mut harness = app_with(Vec::new());
        harness.get_by_label("Tools").click();
        harness.run();
        harness.get_by_label("Settings…").click();
        harness.run();
        harness.get_by_label("Dark").click();
        harness.run();
        choose_terminal_font_on_open_dialog(&mut harness, "Hack");
        harness.get_by_label("Restore defaults").click();
        harness.run();
        let settings = harness.state().settings();
        assert_eq!(settings.terminal_font, Settings::default().terminal_font);
        assert_eq!(settings.theme, Theme::Dark);
    }

    #[test]
    fn the_settings_dialog_keeps_its_place_from_frame_to_frame() {
        // R-UI-8: a dialog that grows each frame moves its buttons away from the pointer.
        let mut harness = app_with(Vec::new());
        harness.get_by_label("Tools").click();
        harness.run();
        harness.get_by_label("Settings…").click();
        harness.run();
        let before = harness.get_by_label("Save").rect();
        harness.run_steps(5);
        assert_eq!(harness.get_by_label("Save").rect(), before);
    }

    #[test]
    fn the_session_dialog_keeps_its_place_from_frame_to_frame() {
        // D12: a dialog that grows over frames moves its buttons away from the pointer.
        let mut harness = app_with(Vec::new());
        harness.get_by_label("New session…").click();
        harness.run();
        let before = harness.get_by_label("Apply").rect();
        harness.run_steps(5);
        assert_eq!(harness.get_by_label("Apply").rect(), before);
    }

    #[test]
    fn cancel_puts_the_old_settings_back() {
        // R-UI-6
        let mut harness = app_with(Vec::new());
        choose_terminal_font(&mut harness, "Hack");
        harness.get_by_label("Appearance").click();
        harness.run();
        harness.get_by_label("Dark").click();
        harness.run();
        harness.get_by_label("Cancel").click();
        harness.run();
        assert_eq!(harness.state().settings(), &Settings::default());
    }

    /// egui panics on a character its fonts lack: every symbol the UI draws is listed here
    /// (D12). Add a new one here before using it.
    #[test]
    fn bundled_fonts_have_every_symbol_the_ui_uses() {
        let mut harness = Harness::new_ui(|ui| {
            theme::install(ui.ctx(), fonts::DEFAULT_TERMINAL_FONT);
            for symbol in ["⏵", "⏷", "🗙", "·", "×", "…", "–", "©"] {
                // A missing glyph would draw as an empty box.
                let found = ui
                    .ctx()
                    .fonts_mut(|f| f.has_glyphs(&egui::FontId::proportional(13.0), symbol));
                assert!(found, "the bundled fonts have no glyph for {symbol}");
                ui.label(symbol);
                ui.label(theme::semibold(symbol));
            }
            // R-UI-13: the icons from the icon font itself, the only font in their family.
            let family = egui::FontFamily::Name(icons::FAMILY.into());
            for icon in icons::ALL {
                let font = egui::FontId::new(13.0, family.clone());
                let found = ui.ctx().fonts_mut(|f| f.has_glyphs(&font, icon));
                assert!(
                    found,
                    "the icon font has no glyph for U+{:X}",
                    icon.chars().next().map_or(0, u32::from)
                );
                ui.label(icons::icon(icon));
            }
        });
        harness.run();
    }

    #[test]
    fn the_menu_bar_starts_with_the_app_and_ends_with_help() {
        // R-UI-10
        let mut app = ParleyUi::default();
        app.set_logo(egui::ColorImage::filled([4, 4], egui::Color32::WHITE));
        let harness = Harness::builder()
            .with_size(egui::vec2(1000.0, 800.0))
            .build_ui_state(|ui, app: &mut ParleyUi| app.ui(ui), app);
        assert!(
            harness.query_all_by_label("Parley").count() >= 1,
            "the name"
        );
        let file = harness.get_by_label("File").rect();
        let tools = harness.get_by_label("Tools").rect();
        let help = harness
            .get_all_by_label("Help")
            .map(|n| n.rect())
            .find(|r| (r.center().y - file.center().y).abs() < 1.0)
            .expect("Help in the menu bar");
        assert!(
            help.left() > tools.right() + 400.0,
            "Help on the right: {help:?}"
        );
        assert!(help.right() > 950.0, "at the edge: {help:?}");
    }

    /// An update checker that answers `found` and counts its calls; never GitHub.
    fn fake_checker(
        found: Result<parley_core::update::Found, String>,
    ) -> (Checker, std::sync::Arc<std::sync::atomic::AtomicUsize>) {
        let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counter = std::sync::Arc::clone(&calls);
        let checker: Checker = std::sync::Arc::new(move || {
            counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            found.clone()
        });
        (checker, calls)
    }

    fn newer_release() -> parley_core::update::Found {
        let file = |name: &str| parley_core::update::Download {
            name: name.to_owned(),
            url: format!("https://github.com/sorensenNiels/parley/releases/download/v9.9.9/{name}"),
        };
        parley_core::update::Found::Newer(parley_core::update::Release {
            version: "9.9.9".into(),
            page: "https://github.com/sorensenNiels/parley/releases/tag/v9.9.9".into(),
            downloads: vec![
                file("parley_9.9.9_amd64.deb"),
                file("parley_9.9.9_x86_64.AppImage"),
            ],
        })
    }

    /// Help ▸ Check for updates… (the Help menu, not the toolbar's Help key).
    fn check_for_updates(harness: &mut Harness<'static, ParleyUi>) {
        harness
            .get_all_by_label("Help")
            .min_by(|a, b| a.rect().top().total_cmp(&b.rect().top()))
            .expect("the Help menu")
            .click();
        harness.run();
        harness.get_by_label("Check for updates…").click();
    }

    /// Frames until `label` shows: the check runs on a thread.
    fn wait_for(harness: &mut Harness<'static, ParleyUi>, label: &str) {
        for _ in 0..200 {
            harness.step();
            if harness.query_by_label(label).is_some() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        panic!("{label} never showed");
    }

    fn app_checking(checker: Checker, settings: Settings) -> Harness<'static, ParleyUi> {
        let mut app = ParleyUi::default();
        app.set_settings(settings);
        app.set_update_checker(checker);
        Harness::builder()
            .with_size(egui::vec2(1000.0, 800.0))
            .build_ui_state(|ui, app: &mut ParleyUi| app.ui(ui), app)
    }

    #[test]
    fn a_newer_version_found_at_start_up_shows_as_a_link_in_the_menu_bar() {
        // R-UI-11
        let (checker, calls) = fake_checker(Ok(newer_release()));
        let mut harness = app_checking(checker, Settings::default());
        wait_for(&mut harness, "Parley 9.9.9 is available");
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[test]
    fn no_check_at_start_up_when_the_setting_is_off() {
        // R-UI-11
        let (checker, calls) = fake_checker(Ok(newer_release()));
        let settings = Settings {
            check_for_updates: false,
            ..Settings::default()
        };
        let mut harness = app_checking(checker, settings);
        harness.run_steps(5);
        std::thread::sleep(std::time::Duration::from_millis(20));
        harness.run_steps(2);
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 0);
        assert!(
            harness
                .query_by_label("Parley 9.9.9 is available")
                .is_none()
        );
    }

    #[test]
    fn check_for_updates_opens_about_and_says_what_it_found() {
        // R-UI-11
        let (checker, _) = fake_checker(Ok(parley_core::update::Found::UpToDate));
        let settings = Settings {
            check_for_updates: false,
            ..Settings::default()
        };
        let mut harness = app_checking(checker, settings);
        harness.run();
        check_for_updates(&mut harness);
        wait_for(&mut harness, "Parley is up to date.");
        harness.get_by_label(&format!("Version {}", env!("CARGO_PKG_VERSION")));
    }

    #[test]
    fn a_failed_check_now_says_why_but_one_at_start_up_is_silent() {
        // R-UI-11
        let (checker, calls) =
            fake_checker(Err("could not check for a new version: offline".into()));
        let mut harness = app_checking(checker, Settings::default());
        for _ in 0..50 {
            harness.step();
            if calls.load(std::sync::atomic::Ordering::SeqCst) == 1 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        harness.run_steps(3);
        assert!(
            harness.query_by_label_contains("offline").is_none(),
            "silent at start-up"
        );
        check_for_updates(&mut harness);
        for _ in 0..200 {
            harness.step();
            if harness.query_by_label_contains("offline").is_some() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        panic!("the failure never showed");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn on_linux_the_about_page_offers_the_download_commands() {
        // R-UI-12
        let (checker, _) = fake_checker(Ok(newer_release()));
        let mut harness = app_checking(checker, Settings::default());
        check_for_updates(&mut harness);
        wait_for(&mut harness, "Copy download command");
        harness.get_by_label("Copy download command").click();
        harness.run_steps(2);
        harness.get_by_label("parley_9.9.9_amd64.deb").click();
        harness.run_steps(2);
        harness.get_by_label_contains("curl -fLO 'https://github.com/sorensenNiels/parley/releases/download/v9.9.9/parley_9.9.9_amd64.deb'");
    }

    #[test]
    fn without_a_checker_the_about_page_says_so() {
        // R-UI-11: tests and builds without a checker never contact GitHub.
        let mut harness = app_with(Vec::new());
        check_for_updates(&mut harness);
        harness.run_steps(3);
        harness.get_by_label("This build doesn't look for new versions.");
    }

    #[test]
    fn the_start_screen_lists_the_saved_sessions() {
        // R-UI-5
        let harness = app_with(vec![
            profile("Production", "prod.example.com"),
            profile("Test", "test.example.com"),
        ]);
        harness.get_by_label("Connect to IBM i");
        harness.get_by_label("Production");
        harness.get_by_label("Test");
        assert_eq!(harness.get_all_by_label("Connect").count(), 2);
        harness.get_by_label("New session…");
    }

    #[test]
    fn connect_on_a_session_card_opens_that_session() {
        // R-UI-5: the start screen's Connect button.
        let mut harness = app_with(vec![
            profile("Production", "127.0.0.1"),
            profile("Test", "127.0.0.1"),
        ]);
        harness
            .get_all_by_label("Connect")
            .nth(1)
            .expect("two cards")
            .click();
        harness.run_steps(2);
        let state = harness.state();
        assert_eq!(
            state.current.as_ref().map(|p| p.name.as_str()),
            Some("Test")
        );
        // Open, or (refused at once) closed with a reason: not silently dropped.
        assert!(
            state.connection.is_some() || state.closed.is_some(),
            "the connection was dropped without a word"
        );
    }

    #[test]
    fn connect_in_the_session_dialog_opens_that_session() {
        // R-UI-5
        let mut harness = app_with(vec![profile("Test", "127.0.0.1")]);
        harness.get_by_label("Edit…").click();
        harness.run();
        harness
            .get_all_by_label("Connect")
            .last()
            .expect("the dialog's Connect")
            .click();
        harness.run_steps(2);
        assert!(harness.state().editor.is_none(), "the dialog closes");
        let state = harness.state();
        assert_eq!(
            state.current.as_ref().map(|p| p.name.as_str()),
            Some("Test")
        );
        // Open, or (refused at once) closed with a reason: not silently dropped.
        assert!(
            state.connection.is_some() || state.closed.is_some(),
            "the connection was dropped without a word"
        );
    }

    #[test]
    fn a_new_session_is_saved_without_asking_for_a_user_or_password() {
        // R-UI-5, R-P-3, R-NET-7
        let mut harness = app_with(Vec::new());
        harness.get_by_label("New session…").click();
        harness.run();
        for gone in ["User", "Password", "Enable auto sign-on"] {
            assert!(harness.query_by_label(gone).is_none(), "{gone} is asked");
        }
        type_into(&mut harness, "Name", "Production");
        type_into(&mut harness, "Host", "prod.example.com");
        harness.get_by_label("Apply").click();
        harness.run();

        let saved = harness.state().profiles();
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].name, "Production");
        assert_eq!(saved[0].config.host, "prod.example.com");
        assert!(saved[0].config.tls, "new sessions use TLS (R-P-3)");
        assert_eq!(saved[0].config.port, 992);
    }

    #[test]
    fn a_session_needs_a_name_and_a_host() {
        // R-UI-5
        let mut harness = app_with(Vec::new());
        harness.get_by_label("New session…").click();
        harness.run();
        harness.get_by_label("Apply").click();
        harness.run();
        harness.get_by_label("Give the session a name.");
        assert_eq!(harness.state().profiles(), []);
    }

    #[test]
    fn delete_removes_the_session() {
        // R-UI-5
        let mut harness = app_with(vec![
            profile("Production", "prod.example.com"),
            profile("Test", "test.example.com"),
        ]);
        harness
            .get_all_by_label("Edit…")
            .nth(1)
            .expect("two sessions")
            .click();
        harness.run();
        harness.get_by_label("Delete").click();
        harness.run();
        let names: Vec<_> = harness
            .state()
            .profiles()
            .iter()
            .map(|p| p.name.as_str())
            .collect();
        assert_eq!(names, ["Production"]);
    }

    #[test]
    fn only_one_session_connects_on_program_start() {
        // R-UI-5
        let mut first = profile("Production", "prod.example.com");
        first.connect_on_start = true;
        let mut harness = app_with(vec![first, profile("Test", "test.example.com")]);
        harness
            .get_all_by_label("Edit…")
            .nth(1)
            .expect("two sessions")
            .click();
        harness.run();
        harness
            .get_by_label("Connect to this session on program start")
            .click();
        harness.run();
        harness.get_by_label("Apply").click();
        harness.run();
        let marked: Vec<_> = harness
            .state()
            .profiles()
            .iter()
            .map(|p| p.connect_on_start)
            .collect();
        assert_eq!(marked, [false, true]);
        assert_eq!(
            harness.state().startup_profile().map(|p| p.name.as_str()),
            Some("Test")
        );
    }

    #[test]
    fn the_header_and_status_bar_are_there_before_connecting_without_the_keys_menu() {
        // R-UI-15, R-UI-16: one header row; the Keys menu only while connected.
        let harness = app_with(Vec::new());
        for menu in ["File", "Edit", "View", "Tools", "Help", "Offline"] {
            harness.get_by_label(menu);
        }
        assert!(harness.query_by_label("Keys").is_none());
    }

    #[test]
    fn the_shortcut_sheet_opens_from_the_view_menu_r_ui_17() {
        let mut harness = app_with(Vec::new());
        harness.get_by_label("View").click();
        harness.run();
        harness.get_by_label("Keyboard shortcuts").click();
        harness.run();
        assert!(harness.state().shortcut_sheet.is_some());
        harness.get_by_label("Field Exit");
    }

    /// The status bar for `session` on TLS, with the Default map and a clock.
    fn status_harness(
        session: parley_core::Session,
        clock: status::Clock,
        width: f32,
    ) -> Harness<'static> {
        let config = parley_core::SessionConfig {
            tls: true,
            ..parley_core::SessionConfig::default()
        };
        Harness::builder()
            .with_size(egui::vec2(width, 40.0))
            .build_ui(move |ui| {
                test_support::install_fonts(ui);
                let maps = ["Default".to_owned()];
                status::status_bar(
                    ui,
                    &status::Status {
                        session: Some(&session),
                        link: Some(&LinkState::Connected),
                        config: Some(&config),
                        note: None,
                        macros: true,
                        can_connect_again: false,
                        map: "Default",
                        keymap: KeyMap::built_in(),
                        mac: false,
                        maps: &maps,
                        clock: &clock,
                    },
                );
            })
    }

    #[test]
    fn the_status_bar_shows_the_state_tls_map_code_page_size_and_cursor_r_ui_14() {
        let harness = status_harness(
            test_support::waiting_session(),
            status::Clock::default(),
            1000.0,
        );
        for text in ["Ready", "CCSID 37", "24×80", "2, 2"] {
            harness.get_by_label(text);
        }
        harness.get(By::new().label_contains("TLS"));
        assert!(
            harness
                .query_all(By::new().label_contains("Default"))
                .next()
                .is_some()
        );
        assert!(
            harness.query_by_label("INS").is_none(),
            "only in insert mode"
        );
    }

    #[test]
    fn the_hourglass_has_its_own_slot_beside_the_ready_icon_r_ui_14() {
        // The check stays while the host works; the hourglass comes beside it and nothing moves.
        use parley_core::{Aid, Key};
        let ready = status_harness(
            test_support::waiting_session(),
            status::Clock::default(),
            1000.0,
        );
        assert!(ready.query_by_label("X SYSTEM").is_none());
        let check = ready.get_by_label("Ready").rect();
        let keys = ready.get_by_label("Keys").rect();
        let mut session = test_support::waiting_session();
        session.key(Key::Aid(Aid::ENTER));
        let waiting = status_harness(session, status::Clock::default(), 1000.0);
        let hourglass = waiting.get_by_label("X SYSTEM").rect();
        assert_eq!(
            waiting.get_by_label("Connected").rect(),
            check,
            "the check stays"
        );
        assert!(hourglass.left() >= check.right(), "beside the check");
        assert_eq!(waiting.get_by_label("Keys").rect(), keys);
    }

    #[test]
    fn a_narrow_status_bar_drops_size_and_code_page_but_keeps_tls_and_the_cursor_r_ui_14() {
        let harness = status_harness(
            test_support::waiting_session(),
            status::Clock::default(),
            380.0,
        );
        assert!(harness.query_by_label("24×80").is_none());
        assert!(harness.query_by_label("CCSID 37").is_none());
        harness.get_by_label("2, 2");
        harness.get(By::new().label_contains("TLS"));
    }

    #[test]
    fn turning_tls_off_moves_the_port_back_to_23() {
        let mut harness = app_with(Vec::new());
        harness.get_by_label("New session…").click();
        harness.run();
        harness.get_by_label("TLS").click();
        harness.run();
        type_into(&mut harness, "Name", "Plain");
        type_into(&mut harness, "Host", "h");
        harness.get_by_label("Apply").click();
        harness.run();
        let config = &harness.state().profiles()[0].config;
        assert!(!config.tls);
        assert_eq!(config.port, 23);
    }

    #[test]
    fn the_session_dialog_saves_the_macro_to_run_on_connect_r_key_4() {
        let mut harness = app_with(Vec::new());
        harness
            .state_mut()
            .set_macro_store(Box::new(test_support::macros_with("Jobs")));
        harness.get_by_label("New session…").click();
        harness.run();
        type_into(&mut harness, "Name", "Test");
        type_into(&mut harness, "Host", "ibmi.example.com");
        harness
            .get_all_by_label("Run macro")
            .next()
            .expect("the switch")
            .click();
        harness.run();
        harness.get_by_label("Apply").click();
        harness.run();
        assert_eq!(harness.state().profiles()[0].run_macro, "Jobs");
    }

    #[test]
    fn renaming_a_session_moves_its_macros_along_r_key_3() {
        let mut harness = app_with(vec![profile("Old", "h")]);
        let store = test_support::MemoryMacros::default();
        let mine = parley_core::macros::Macro {
            session: Some("Old".into()),
            steps: Vec::new(),
        };
        store.0.borrow_mut().insert("m".into(), mine);
        harness.state_mut().set_macro_store(Box::new(store.clone()));
        harness.get_by_label("Edit…").click();
        harness.run();
        type_into(&mut harness, "Name", " 2");
        harness.get_by_label("Apply").click();
        harness.run();
        assert_eq!(store.0.borrow()["m"].session.as_deref(), Some("Old 2"));
    }

    /// A session connected to a local port that never answers: enough for a live session.
    /// Keep the listener until the test ends.
    fn connect_locally(
        harness: &mut Harness<'static, ParleyUi>,
        run_macro: &str,
    ) -> std::net::TcpListener {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a local port");
        let mut local = profile("Local", "127.0.0.1");
        local.config.port = listener.local_addr().expect("its address").port();
        local.config.tls = false;
        local.run_macro = run_macro.to_owned();
        let ctx = harness.ctx.clone();
        harness.state_mut().connect(&ctx, local);
        harness.run();
        listener
    }

    fn macros_off() -> Settings {
        let mut settings = Settings::default();
        settings.addons.set_enabled(AddOn::Macros, false);
        settings
    }

    /// Settings ▸ Add-ons, then the Macros switch.
    fn switch_macros_in_settings(harness: &mut Harness<'static, ParleyUi>) {
        switch_add_on_in_settings(harness, "Macros");
    }

    /// Settings ▸ Add-ons, then the switch named `name`.
    fn switch_add_on_in_settings(harness: &mut Harness<'static, ParleyUi>, name: &str) {
        harness.get_by_label("Tools").click();
        harness.run();
        harness.get_by_label("Settings…").click();
        harness.run();
        harness.get_by_label("Add-ons").click();
        harness.run();
        harness
            .get_by_role_and_label(egui::accesskit::Role::CheckBox, name)
            .click();
        harness.run();
    }

    #[test]
    fn the_assistant_is_turned_on_on_its_own_page_not_in_add_ons_r_sql_48() {
        let mut harness = app_with(Vec::new());
        switch_add_on_in_settings(&mut harness, "SQL");
        assert!(
            harness
                .query_by_role_and_label(egui::accesskit::Role::CheckBox, "SQL assistant")
                .is_none(),
            "not on the Add-ons page"
        );
        harness.get_by_label("SQL assistant").click();
        harness.run();
        harness
            .get_by_role_and_label(egui::accesskit::Role::CheckBox, "SQL assistant")
            .click();
        harness.get_by_label("Save").click();
        harness.run();
        assert!(harness.state().addon_on(AddOn::Assistant));
    }

    #[test]
    fn turning_sql_on_shows_its_java_and_helper_and_off_hides_them_r_sql_12() {
        let mut harness = app_with(Vec::new());
        assert!(
            !harness.state().addon_on(AddOn::Sql),
            "off by default (R-SQL-10)"
        );
        harness.state_mut().set_sql_helpers(None);
        switch_add_on_in_settings(&mut harness, "SQL");
        harness.get_by_label("Found");
        harness.get_by_label_contains("Not installed with this Parley");
        // A Java that is not there is named, not looked for again every frame.
        type_into(&mut harness, "Java", "/no/such/java");
        harness.run_steps(3);
        harness.get_by_label_contains("needs Java 11 or later");
        harness
            .get_by_role_and_label(egui::accesskit::Role::CheckBox, "SQL")
            .click();
        harness.run();
        assert!(harness.query_by_label("Found").is_none());
    }

    #[test]
    fn the_settings_dialog_keeps_its_place_with_the_sql_rows_r_sql_12() {
        let mut harness = app_with(Vec::new());
        switch_add_on_in_settings(&mut harness, "SQL");
        let before = harness.get_by_label("Save").rect();
        harness.run_steps(5);
        assert_eq!(harness.get_by_label("Save").rect(), before);
    }

    #[test]
    fn the_add_ons_page_turns_macros_off_when_saved_r_add_1() {
        let mut harness = app_with(Vec::new());
        assert!(harness.state().addon_on(AddOn::Macros), "on by default");
        switch_macros_in_settings(&mut harness);
        harness.get_by_label(AddOn::Macros.about());
        assert!(
            harness.state().addon_on(AddOn::Macros),
            "nothing changes before Save"
        );
        harness.get_by_label("Save").click();
        harness.run();
        assert!(!harness.state().addon_on(AddOn::Macros));
        assert!(
            harness
                .state()
                .settings()
                .addons
                .disabled
                .contains("macros")
        );
    }

    #[test]
    fn cancel_leaves_the_add_ons_as_they_were_r_add_1() {
        let mut harness = app_with(Vec::new());
        switch_macros_in_settings(&mut harness);
        harness.get_by_label("Cancel").click();
        harness.run();
        assert!(harness.state().addon_on(AddOn::Macros));
        assert_eq!(harness.state().settings(), &Settings::default());
    }

    #[test]
    fn the_add_ons_page_keeps_the_dialog_in_place_r_add_1() {
        let mut harness = app_with(Vec::new());
        switch_macros_in_settings(&mut harness);
        let before = harness.get_by_label("Save").rect();
        harness.run_steps(5);
        assert_eq!(harness.get_by_label("Save").rect(), before);
    }

    #[test]
    fn while_macros_are_off_nothing_of_them_is_in_the_app_r_add_2() {
        let mut harness = app_with(Vec::new());
        harness.state_mut().set_settings(macros_off());
        harness
            .state_mut()
            .set_macro_store(Box::new(test_support::macros_with("Jobs")));
        assert!(harness.state().macros.list().is_empty(), "folder not read");
        harness.run();
        harness.get_by_label("Tools").click();
        harness.run();
        harness.get_by_label("Settings…");
        for item in ["Record macro", "Run macro", "Macros…"] {
            assert!(harness.query_by_label(item).is_none(), "{item} in Tools");
        }
        harness.get_by_label("Tools").click();
        harness.run();
        harness.get_by_label("New session…").click();
        harness.run();
        assert!(harness.query_by_label("Run macro").is_none());

        // Turned on again, the folder is read.
        harness.state_mut().set_settings(Settings::default());
        assert_eq!(harness.state().macro_names(), ["Jobs"]);
    }

    #[test]
    fn while_macros_are_off_a_session_s_run_macro_is_kept_but_not_run_r_add_2() {
        let mut harness = app_with(Vec::new());
        harness.state_mut().set_settings(macros_off());
        harness
            .state_mut()
            .set_macro_store(Box::new(test_support::macros_with("Jobs")));
        let _host = connect_locally(&mut harness, "Jobs");
        let app = harness.state();
        let connection = app.connection.as_ref().expect("connected");
        assert_eq!(connection.session().playing(), None);
        assert_eq!(
            app.current.as_ref().map(|p| p.run_macro.as_str()),
            Some("Jobs")
        );
    }

    #[test]
    fn turning_macros_off_asks_for_the_name_of_a_recording_r_add_2() {
        let mut harness = app_with(Vec::new());
        let _host = connect_locally(&mut harness, "");
        let app = harness.state();
        app.connection
            .as_ref()
            .expect("connected")
            .session()
            .start_recording();
        switch_macros_in_settings(&mut harness);
        harness.get_by_label("Save").click();
        harness.run();
        let app = harness.state();
        let session = app.connection.as_ref().expect("connected").session();
        assert_eq!(session.macro_state(), parley_core::MacroState::Off);
        assert!(app.save_macro.is_some(), "the recording is not lost");
    }

    #[test]
    fn turning_macros_off_stops_a_running_macro_r_add_2() {
        let mut harness = app_with(Vec::new());
        harness
            .state_mut()
            .set_macro_store(Box::new(test_support::macros_with("Jobs")));
        harness.state_mut().macro_manager = Some(macros::Manager::default());
        let _host = connect_locally(&mut harness, "Jobs");
        let playing = |app: &ParleyUi| {
            let connection = app.connection.as_ref().expect("connected");
            connection.session().playing().map(str::to_owned)
        };
        assert_eq!(playing(harness.state()).as_deref(), Some("Jobs"));
        harness.state_mut().set_settings(macros_off());
        assert_eq!(playing(harness.state()), None);
        assert!(harness.state().macro_manager.is_none());
    }

    /// Tools ▸ Settings…, then the Keyboard page.
    fn open_keyboard_settings(harness: &mut Harness<'static, ParleyUi>) {
        harness.get_by_label("Tools").click();
        harness.run();
        harness.get_by_label("Settings…").click();
        harness.run();
        harness.get_by_label("Keyboard").click();
        harness.run();
    }

    fn with_map(name: &str) -> Settings {
        Settings {
            keyboard_maps: vec![KeyMap {
                name: name.to_owned(),
                ..KeyMap::default()
            }],
            ..Settings::default()
        }
    }

    fn enter() -> Function {
        Function::Key(parley_core::Key::Aid(parley_core::Aid::ENTER))
    }

    #[test]
    fn the_default_map_cannot_be_changed_r_key_2() {
        let mut harness = app_with(Vec::new());
        open_keyboard_settings(&mut harness);
        assert!(harness.query_by_label("+").is_none(), "no key to add");
        assert!(harness.query_by_label("Remove this key").is_none());
        harness.get_by_label("Ctrl+R");
    }

    #[test]
    fn a_copy_of_the_default_map_gets_a_new_key_r_key_2() {
        let mut harness = app_with(Vec::new());
        open_keyboard_settings(&mut harness);
        harness.get_by_label("Copy…").click();
        harness.run();
        // The name offered is "Default copy".
        harness.get_by_label("Create").click();
        harness.run();
        // The first + is Enter's.
        harness
            .get_all_by_label("+")
            .next()
            .expect("a + button")
            .click();
        harness.run();
        harness.get_by_label("Press a key…");
        harness.key_press_modifiers(egui::Modifiers::CTRL, egui::Key::J);
        harness.run();
        harness.get_by_label("Save").click();
        harness.run();
        let maps = &harness.state().settings().keyboard_maps;
        assert_eq!(maps.len(), 1);
        assert_eq!(maps[0].name, "Default copy");
        let ctrl_j = keymap::Chord::parse("Ctrl+J").expect("a key");
        assert_eq!(maps[0].function_for(ctrl_j), Some(enter()));
    }

    #[test]
    fn a_key_another_function_has_moves_only_when_confirmed_r_key_2() {
        let mut harness = app_with(Vec::new());
        harness.state_mut().set_settings(with_map("Mine"));
        harness.state_mut().current = Some(Profile {
            keyboard_map: "Mine".to_owned(),
            ..profile("Prod", "prod")
        });
        open_keyboard_settings(&mut harness);
        harness
            .get_all_by_label("+")
            .next()
            .expect("a + button")
            .click();
        harness.run();
        harness.key_press(egui::Key::F3);
        harness.run();
        harness.get_by_label("F3 is F3 now.");
        let f3 = keymap::Chord::parse("F3").expect("a key");
        let map = |h: &Harness<'static, ParleyUi>| h.state().settings().keyboard_maps[0].clone();
        assert_ne!(map(&harness).function_for(f3), Some(enter()), "not yet");
        harness.get_by_label("Use it for Enter").click();
        harness.run();
        assert_eq!(map(&harness).function_for(f3), Some(enter()));
    }

    #[test]
    fn renaming_a_map_keeps_the_sessions_that_use_it_r_key_2() {
        let prod = Profile {
            keyboard_map: "Mine".to_owned(),
            ..profile("Prod", "prod")
        };
        let mut harness = app_with(vec![prod.clone()]);
        harness.state_mut().set_settings(with_map("Mine"));
        harness.state_mut().current = Some(prod);
        open_keyboard_settings(&mut harness);
        harness.get_by_label("Rename…").click();
        harness.run();
        type_into(&mut harness, "Name", " 2");
        harness.get_by_label("Rename").click();
        harness.run();
        harness.get_by_label("Save").click();
        harness.run();
        assert_eq!(harness.state().settings().keyboard_maps[0].name, "Mine 2");
        assert_eq!(harness.state().profiles()[0].keyboard_map, "Mine 2");
        assert_eq!(harness.state().keyboard_map().name, "Mine 2");
    }

    #[test]
    fn a_session_whose_map_was_deleted_uses_default_r_key_2() {
        let mut harness = app_with(Vec::new());
        harness.state_mut().set_settings(with_map("Mine"));
        harness.state_mut().current = Some(Profile {
            keyboard_map: "Mine".to_owned(),
            ..profile("Prod", "prod")
        });
        assert_eq!(harness.state().keyboard_map().name, "Mine");
        open_keyboard_settings(&mut harness);
        harness.get_by_label("Delete map").click();
        harness.run();
        harness.get_by_label("Save").click();
        harness.run();
        assert_eq!(harness.state().settings().keyboard_maps, []);
        assert!(harness.state().keyboard_map().is_built_in());
    }

    #[test]
    fn the_keyboard_page_keeps_the_dialog_in_place_r_key_2() {
        let mut harness = app_with(Vec::new());
        harness.state_mut().set_settings(with_map("Mine"));
        harness.state_mut().current = Some(Profile {
            keyboard_map: "Mine".to_owned(),
            ..profile("Prod", "prod")
        });
        open_keyboard_settings(&mut harness);
        let before = harness.get_by_label("Save").rect();
        harness.run_steps(5);
        assert_eq!(harness.get_by_label("Save").rect(), before);
    }

    #[test]
    fn the_session_dialog_chooses_the_keyboard_map_r_key_2() {
        let mut harness = app_with(Vec::new());
        harness.state_mut().set_settings(with_map("Mine"));
        harness.get_by_label("New session…").click();
        harness.run();
        type_into(&mut harness, "Name", "Prod");
        type_into(&mut harness, "Host", "prod");
        harness.get_by_label("Keyboard map").click();
        harness.run();
        harness.get_by_label("Mine").click();
        harness.run();
        harness.get_by_label("Apply").click();
        harness.run();
        assert_eq!(harness.state().profiles()[0].keyboard_map, "Mine");
    }

    #[test]
    fn stop_beside_rec_in_the_status_bar_ends_the_recording_r_key_6() {
        let mut harness = app_with(Vec::new());
        let _host = connect_locally(&mut harness, "");
        assert!(
            harness.query_by_label("Stop").is_none(),
            "only while recording"
        );
        // The connection comes up on its own thread.
        for _ in 0..200 {
            let state = harness.state().connection.as_ref().map(Connection::state);
            if matches!(state, Some(LinkState::Connected)) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
            harness.run();
        }
        harness.run();
        // No screen yet: the keyboard waits for the host.
        let pill = harness.get_by_label("X SYSTEM").rect();
        harness
            .state()
            .connection
            .as_ref()
            .expect("connected")
            .session()
            .start_recording();
        harness.run();
        // A taller box would make the bar grow and move the others.
        assert_eq!(harness.get_by_label("X SYSTEM").rect(), pill);
        harness.get_by_label("Stop").click();
        harness.run();
        let app = harness.state();
        let session = app.connection.as_ref().expect("connected").session();
        assert_eq!(session.macro_state(), parley_core::MacroState::Off);
        assert!(app.save_macro.is_some(), "it asks for the macro's name");
    }

    /// Connected to a local listener as "Local", with "Other" saved too; waits for the link.
    fn connected_app() -> (Harness<'static, ParleyUi>, std::net::TcpListener) {
        let mut harness = app_with(vec![
            profile("Local", "127.0.0.1"),
            profile("Other", "other"),
        ]);
        let host = connect_locally(&mut harness, "");
        for _ in 0..200 {
            let state = harness.state().connection.as_ref().map(Connection::state);
            if matches!(state, Some(LinkState::Connected)) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
            harness.run();
        }
        harness.run();
        (harness, host)
    }

    #[test]
    fn the_wait_for_the_host_is_noticed_in_the_frame_enter_goes_down_r_ui_14() {
        // The status bar is drawn before the terminal takes Enter: without a look at the
        // keyboard after the keys, the hourglass would wait for the next input or answer.
        let (mut harness, _host) = connected_app();
        let connection = harness.state().connection.as_ref().expect("connected");
        connection
            .session()
            .receive(&test_support::waiting_screen());
        harness.run();
        assert!(!harness.state().clock.waiting());
        // Only the key going down, in one pass: nothing after it may be needed to notice.
        harness
            .ctx
            .options_mut(|o| o.max_passes = std::num::NonZeroUsize::MIN);
        harness.event(egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });
        harness.step();
        assert!(harness.state().clock.waiting(), "the clock saw the lock");
    }

    #[test]
    fn the_keys_menu_in_the_status_bar_lists_the_5250_keys_while_connected_r_ui_16() {
        let (mut harness, _host) = connected_app();
        harness.get_by_label("Keys").click();
        harness.run();
        for key in ["Attention", "System Request", "F24"] {
            harness.get_by_label(key);
        }
    }

    #[test]
    fn the_session_pill_shows_the_session_and_disconnects_r_ui_15() {
        let (mut harness, _host) = connected_app();
        harness.get(By::new().label_contains("127.0.0.1:")).click();
        harness.run();
        harness.get_by_label("Edit session…");
        harness.get_by_label("Disconnect").click();
        harness.run();
        assert!(harness.state().connection.is_none());
        harness.get_by_label("Offline");
    }

    #[test]
    fn connect_to_another_session_while_connected_closes_the_open_one_first_r_ui_15() {
        let (mut harness, _host) = connected_app();
        let ctx = harness.ctx.clone();
        let app = harness.state_mut();
        let next = app.carry_out(&ctx, vec![Action::Connect(1)], None);
        assert!(!next.disconnect, "nothing open: only connect");
        let connection = app.connection.take().expect("connected");
        let next = app.carry_out(
            &ctx,
            vec![Action::Connect(1)],
            Some(&mut connection.session()),
        );
        assert!(next.disconnect, "the open session closes first");
        assert_eq!(next.connect, Some(1));
    }

    #[test]
    fn choosing_a_keyboard_map_in_the_status_bar_saves_it_to_the_session_r_ui_14() {
        let (mut harness, _host) = connected_app();
        harness.state_mut().set_settings(with_map("Mine"));
        harness.run();
        harness.get(By::new().label_contains("Default")).click();
        harness.run();
        harness.get_by_label("Mine").click();
        harness.run();
        assert_eq!(harness.state().keyboard_map().name, "Mine");
        assert_eq!(harness.state().profiles()[0].keyboard_map, "Mine");
    }

    #[test]
    fn ctrl_slash_on_the_screen_opens_the_shortcut_sheet_r_ui_17() {
        let (mut harness, _host) = connected_app();
        harness.key_press_modifiers(egui::Modifiers::CTRL, egui::Key::Slash);
        harness.run();
        assert!(harness.state().shortcut_sheet.is_some());
        harness.run();
        assert!(
            harness.state().shortcut_sheet.is_some(),
            "still open a frame later"
        );
    }

    #[test]
    fn the_shortcut_sheet_keeps_its_place_from_frame_to_frame_r_ui_17() {
        let mut harness = app_with(Vec::new());
        harness.state_mut().shortcut_sheet = Some(shortcuts::Sheet::default());
        harness.run();
        let before = harness.get_by_label("Change keys…").rect();
        harness.run_steps(5);
        assert_eq!(harness.get_by_label("Change keys…").rect(), before);
    }

    #[test]
    fn tools_sql_is_there_only_while_the_add_on_is_on_r_sql_10() {
        let (mut harness, _host) = connected_app();
        harness.get_by_label("Tools").click();
        harness.run();
        assert!(harness.query_by_label("SQL…").is_none());
        harness.get_by_label("Tools").click(); // closes the menu again
        harness.run();
        let mut settings = Settings::default();
        settings.addons.set_enabled(AddOn::Sql, true);
        harness.state_mut().set_settings(settings);
        harness.run();
        harness.get_by_label("Tools").click();
        harness.run();
        harness.get_by_label("SQL…");
    }

    #[test]
    fn without_the_helper_or_a_working_java_the_sql_window_explains_r_sql_12() {
        let (mut harness, _host) = connected_app();
        let mut settings = Settings::default();
        settings.addons.set_enabled(AddOn::Sql, true);
        harness.state_mut().set_settings(settings.clone());
        harness.state_mut().open_sql_window(true);
        harness.run();
        harness.get_by_label_contains("installed without the SQL helper");
        assert!(harness.state().sql_window.is_none());
        harness.get_by_label("OK").click();
        harness.run();

        settings.java_path = "/no/such/java".to_owned();
        harness.state_mut().set_settings(settings);
        harness
            .state_mut()
            .set_sql_helpers(Some(std::path::PathBuf::from("/some/helpers")));
        harness.state_mut().open_sql_window(true);
        harness.run();
        harness.get_by_label_contains("needs Java 11 or later");
        harness.get_by_label_contains("Eclipse Temurin");
        assert!(harness.state().sql_window.is_none());
    }

    #[test]
    fn tools_sql_opens_from_the_menu_while_connected_r_sql_6() {
        // The bug: the menu's action ran while the frame held the connection, so the window
        // found no connection and did nothing.
        let (mut harness, _host) = connected_app();
        let mut settings = Settings::default();
        settings.addons.set_enabled(AddOn::Sql, true);
        settings.java_path = "/no/such/java".to_owned();
        harness.state_mut().set_settings(settings);
        harness
            .state_mut()
            .set_sql_helpers(Some(std::path::PathBuf::from("/some/helpers")));
        harness.run();
        harness.get_by_label("Tools").click();
        harness.run();
        harness.get_by_label("SQL…").click();
        harness.run();
        // Something answers: here, why the window cannot open with this Java.
        harness.get_by_label_contains("needs Java 11 or later");
    }

    #[test]
    #[ignore = "needs Java and the jars from make helper"]
    fn with_java_and_the_helper_tools_sql_shows_the_sign_on_r_sql_6() {
        let (mut harness, _host) = connected_app();
        let mut settings = Settings::default();
        settings.addons.set_enabled(AddOn::Sql, true);
        harness.state_mut().set_settings(settings);
        let helpers = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/helpers");
        harness.state_mut().set_sql_helpers(Some(helpers));
        harness.run();
        harness.get_by_label("Tools").click();
        harness.run();
        harness.get_by_label("SQL…").click();
        harness.run();
        assert!(harness.state().sql_window.is_some(), "the window is open");
        harness.get_by_label("Password");
        harness.get_by_label("Connect");
    }
}
