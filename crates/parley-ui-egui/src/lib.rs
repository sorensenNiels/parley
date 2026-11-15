//! Parley's user interface, built with egui.
//!
//! No platform code lives here: the launcher (`parley-app`) opens the window and hands the UI
//! an [`egui::Ui`] to draw in each frame.

pub mod addons;
pub mod connect;
pub mod dialog;
pub mod fonts;
pub mod icons;
pub mod keymap;
mod keys;
mod macros;
pub mod menu;
mod profile;
mod reconnect;
mod sessions;
pub mod settings;
mod shortcuts;
pub mod status;
mod terminal;
pub mod theme;
pub mod updates;

use egui::{CentralPanel, Frame, Margin, Panel, Ui, ViewportCommand};
use parley_core::net::{Connection, LinkState};

pub use addons::{AddOn, AddOnSettings, Toggle};
use keymap::{Function, KeyInput, KeyMap};
use macros::Macros;
pub use macros::{MacroStore, NoMacroStore, StoredMacro, check_name as check_macro_name};
use menu::Action;
pub use profile::Profile;
use sessions::{Editor, Outcome};
pub use settings::{Settings, Theme};
pub use theme::Density;
pub use updates::Checker;

/// How the launcher starts Parley SQL on the session named so, on its config's host
/// (R-SQL-70); `Err` says why it could not. Parley knows no user and passes no password (D16).
pub type StartSql = Box<dyn Fn(&str, &parley_core::SessionConfig) -> Result<(), String>>;

/// Plays the alarm's beep (R-UI-25, D37): given by the launcher, since the UI has no platform
/// code. Never fails: without a sound output it does nothing.
pub type Beep = Box<dyn Fn()>;

struct AlarmSound(Beep);

impl std::fmt::Debug for AlarmSound {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AlarmSound")
    }
}

/// Starts Parley SQL on a host (R-SQL-70): given by the launcher, which knows where it is.
struct SqlStarter(StartSql);

impl std::fmt::Debug for SqlStarter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SqlStarter")
    }
}

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
    /// Connecting again by itself after the network dropped the connection (R-NET-10).
    reconnect: Option<reconnect::Reconnect>,
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
    /// The find bar and the selection on the screen (R-UI-21, R-UI-22).
    view: terminal::View,
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
    /// Why Parley SQL could not be started, while that is shown (R-SQL-70).
    sql_notice: Option<String>,
    /// Starts Parley SQL on a host (R-SQL-70); without one (tests), Tools ▸ SQL… says so.
    sql_starter: Option<SqlStarter>,
    /// Plays the alarm's beep (R-UI-25); without one (tests), the alarm only flashes.
    beep: Option<AlarmSound>,
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
            reconnect: None,
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
            view: terminal::View::default(),
            shortcut_sheet: None,
            clock: status::Clock::default(),
            asking_to_exit: false,
            exit_confirmed: false,
            macros: Macros::new(Box::new(NoMacroStore)),
            save_macro: None,
            macro_password: None,
            macro_manager: None,
            sql_notice: None,
            sql_starter: None,
            beep: None,
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

    /// How to start Parley SQL on a host, the program `parley-sql` beside Parley (R-SQL-70).
    pub fn set_sql_starter(&mut self, start: StartSql) {
        self.sql_starter = Some(SqlStarter(start));
    }

    /// How to play the alarm's beep (R-UI-25, D37).
    pub fn set_beep(&mut self, beep: Beep) {
        self.beep = Some(AlarmSound(beep));
    }

    /// The host sounded the alarm: flash, and beep, as the settings say (R-UI-25).
    fn sound_alarm(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        match self.settings.alarm {
            settings::Alarm::Off => {}
            settings::Alarm::FlashOnly => self.view.flash(now),
            settings::Alarm::SoundAndFlash => {
                self.view.flash(now);
                if let Some(beep) = &self.beep {
                    (beep.0)();
                }
            }
        }
        ctx.request_repaint();
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
        self.reconnect = None;
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
            reconnect_in: self
                .reconnect
                .and_then(|r| r.left(std::time::Instant::now())),
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
            status::StatusAction::CancelReconnect => self.reconnect = None,
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
        settings::Dialog::open(ctx, &self.settings).with_keyboard(
            &self.keyboard_map().name,
            macros,
            self.addon_on(AddOn::Sql),
        )
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
        self.apply_settings(ui.ctx());
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
        if let Some(session) = session.as_deref() {
            self.find_panel(ui, session);
        }
        self.view.palette = self.palette();
        self.view.blinking = self.settings.blinking_text;
        self.view.frames = self.settings.window_frames;
        if let Some(session) = session.as_deref_mut() {
            session.set_type_ahead(self.settings.type_ahead);
        }
        if session
            .as_deref_mut()
            .is_some_and(parley_core::Session::take_alarm)
        {
            self.sound_alarm(ui.ctx());
        }
        let asked = self.central_panel(ui, session.as_deref_mut(), &mut actions);

        let next = self.carry_out(ui.ctx(), actions, session.as_deref_mut());
        if let Some(session) = session.as_deref_mut() {
            self.carry_out_keys(asked, session);
        }
        self.observe_keys_taken(ui.ctx(), session.as_deref());
        if let Some(session) = session.as_deref_mut() {
            self.macro_needs_password(session);
        }
        drop(session);
        self.finish_connection(ui.ctx(), connection, &next);
        self.show_dialogs(ui.ctx());
    }

    /// Turns off egui's keyboard zoom, starts the check for updates at start-up, and puts
    /// changed settings into effect.
    fn apply_settings(&mut self, ctx: &egui::Context) {
        // Ctrl+Plus and Ctrl+Minus are Field+ and Field- here, not zoom.
        if ctx.options(|o| o.zoom_with_keyboard) {
            ctx.options_mut(|o| o.zoom_with_keyboard = false);
        }
        if self.applied.is_none() && self.settings.check_for_updates {
            self.updates.start(ctx, updates::Check::AtStart);
        }
        self.updates.collect();
        if self.applied.as_ref() != Some(&self.settings) {
            let old = self.applied.as_ref().map(Settings::look);
            let note = settings::apply(ctx, old.as_ref(), &self.settings.look());
            if note.is_some() {
                self.warning = note;
            }
            self.applied = Some(self.settings.clone());
        }
    }

    /// The 5250 screen's colours: the session's scheme, or the settings' (R-UI-23).
    fn palette(&self) -> terminal::Palette {
        let scheme = self
            .current
            .as_ref()
            .and_then(|p| p.screen_colors)
            .unwrap_or(self.settings.screen_colors);
        terminal::Palette::of(scheme, &self.settings.custom_colors)
    }

    /// The find bar above the screen while it is open (R-UI-21); closing it gives the screen
    /// the keyboard back.
    fn find_panel(&mut self, ui: &mut Ui, session: &parley_core::Session) {
        let Some(bar) = &mut self.view.find else {
            return;
        };
        let count = bar.matches(session).len();
        let frame = theme::bar(ui, Margin::symmetric(8, 3));
        let closed = Panel::top("find-bar")
            .frame(frame)
            .show(ui, |ui| terminal::find_bar(ui, bar, count))
            .inner;
        if closed == Some(terminal::FindAction::Close) {
            self.view.find = None;
            self.focus_terminal = true;
        }
    }

    /// The session's screen, or the start screen without one; returns the functions the
    /// keys typed on the screen asked for. What the start screen asks goes to `actions`.
    fn central_panel(
        &mut self,
        ui: &mut Ui,
        session: Option<&mut parley_core::Session>,
        actions: &mut Vec<Action>,
    ) -> Vec<Function> {
        let mut asked = Vec::new();
        if let Some(session) = session {
            CentralPanel::default()
                .frame(
                    Frame::new()
                        .fill(self.view.palette.background)
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
                    asked =
                        terminal::show(ui, session, &mut self.key_input, map, grab, &mut self.view);
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
        asked
    }

    /// Puts the frame's connection back, or closes it, then connects as `next` says: only
    /// now, so a new connection is not overwritten by the one taken for the frame.
    fn finish_connection(
        &mut self,
        ctx: &egui::Context,
        connection: Option<Connection>,
        next: &Next,
    ) {
        match connection {
            Some(connection) if next.disconnect => {
                self.end_recording(&mut connection.session());
                connection.close();
                self.closed = Some("Disconnected.".to_owned());
                self.reconnect = None;
            }
            other => self.connection = other,
        }
        if let Some(index) = next.connect {
            self.connect_saved(ctx, index);
        }
    }

    /// The dialogs that are open, in the order they stack.
    fn show_dialogs(&mut self, ctx: &egui::Context) {
        self.show_editor(ctx);
        self.show_settings(ctx);
        dialog::notice(
            ctx,
            "sql-notice",
            "Parley SQL cannot start",
            &mut self.sql_notice,
            |_| {},
        );
        self.show_shortcut_sheet(ctx);
        self.show_macro_dialogs(ctx);
        self.show_exit_question(ctx);
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
                        ctx.copy_text(self.view.copy_text(s));
                    }
                }
                Action::CopyTable => {
                    if let Some(s) = session.as_deref() {
                        ctx.copy_text(self.view.table_text(s));
                    }
                }
                Action::Find => self.view.open_find(),
                Action::Paste => {
                    // The pasted text arrives as an input event, which the terminal takes.
                    ctx.send_viewport_cmd(ViewportCommand::RequestPaste);
                    self.focus_terminal = true;
                }
                Action::Settings => self.settings_dialog = Some(self.open_settings(ctx)),
                Action::Shortcuts => self.shortcut_sheet = Some(shortcuts::Sheet::default()),
                Action::Sql => self.start_parley_sql(connected),
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
    /// window if the session says so. A connection the network dropped is connected again
    /// when the session says so (R-NET-10), and so is a try at it that failed.
    fn follow_link(&mut self, ctx: &egui::Context) {
        let now = std::time::Instant::now();
        match self.connection.as_ref().map(Connection::state) {
            Some(LinkState::Connected) => {
                self.online = true;
                self.reconnect = None;
            }
            Some(LinkState::Closed(reason)) => {
                self.connection_ended(reason);
                if let Some(trying) = self.reconnect {
                    self.reconnect = Some(trying.after_failure(now));
                } else {
                    self.exit_when_host_ends(ctx);
                }
            }
            Some(LinkState::Dropped(reason)) => {
                self.connection_ended(format!("The connection dropped: {reason}"));
                if self.current.as_ref().is_some_and(|p| p.reconnect) {
                    self.online = false;
                    self.reconnect = Some(reconnect::Reconnect::after_drop(now));
                } else {
                    self.exit_when_host_ends(ctx);
                }
            }
            Some(LinkState::Connecting) => {}
            None => self.reconnect_when_due(ctx, now),
        }
    }

    fn connection_ended(&mut self, reason: String) {
        if let Some(connection) = self.connection.take() {
            self.end_recording(&mut connection.session());
        }
        self.closed = Some(reason);
    }

    fn exit_when_host_ends(&mut self, ctx: &egui::Context) {
        let exit = self.current.as_ref().is_some_and(|p| p.exit_on_close);
        if std::mem::take(&mut self.online) && exit {
            self.exit_confirmed = true;
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }
    }

    /// Connects again once the wait is over, and draws the countdown each second until then.
    fn reconnect_when_due(&mut self, ctx: &egui::Context, now: std::time::Instant) {
        let Some(reconnect) = self.reconnect else {
            return;
        };
        match (reconnect.due(now), self.current.clone()) {
            (Some(trying), Some(profile)) => {
                self.connect(ctx, profile);
                self.reconnect = Some(trying);
            }
            (None, Some(_)) => ctx.request_repaint_after(std::time::Duration::from_secs(1)),
            (_, None) => self.reconnect = None,
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

    /// Tools ▸ SQL… (R-SQL-70): starts Parley SQL on the session's host, or says why not.
    /// `connected` comes from the caller: during a frame the connection is out of its field.
    fn start_parley_sql(&mut self, connected: bool) {
        if !connected || !self.addon_on(AddOn::Sql) {
            return;
        }
        let Some(profile) = self.current.as_ref() else {
            return;
        };
        let started = match &self.sql_starter {
            Some(starter) => (starter.0)(&profile.name, &profile.config),
            None => Err("This Parley cannot start Parley SQL.".to_owned()),
        };
        if let Err(e) = started {
            self.sql_notice = Some(e);
        }
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

    /// The saved session once called `old` was changed at `index` (R-UI-5): its macros
    /// follow a new name, and the session in use takes the change. While connected it keeps
    /// the connection's settings: they take effect on the next connect.
    fn session_saved(&mut self, old: &str, index: usize) {
        let Some(saved) = self.profiles.get(index).cloned() else {
            return;
        };
        if saved.name != old
            && let Err(e) = self.macros.rename_session(old, &saved.name)
        {
            self.warning = Some(format!("Could not move the macros of \"{old}\": {e}"));
        }
        let connected = self.connection.is_some();
        if let Some(current) = self.current.as_mut().filter(|c| c.name == old) {
            let config = if connected {
                current.config.clone()
            } else {
                saved.config.clone()
            };
            *current = Profile { config, ..saved };
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
        let saved = editor.take_saved();
        if let Some(saved) = saved {
            self.session_saved(&saved.old_name, saved.index);
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
    /// The keys of a map that do more than type (R-KEY-2): Record macro starts or stops a
    /// recording and a macro's key runs it, only while the Macros add-on is on (R-ADD-2);
    /// the shortcut sheet's and Parley SQL's keys work either way.
    fn carry_out_keys(&mut self, asked: Vec<Function>, session: &mut parley_core::Session) {
        let macros = self.addon_on(AddOn::Macros);
        for function in asked {
            match function {
                Function::RecordMacro | Function::RunMacro(_) if !macros => {}
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
                Function::Sql => self.start_parley_sql(true),
                // The terminal does these itself.
                Function::Key(_)
                | Function::CopyScreen
                | Function::CopyTable
                | Function::Find
                | Function::Paste => {}
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

    /// Keys and sessions that ran the macro `old` run it under its new name (R-KEY-2, R-KEY-4).
    fn rename_macro(&mut self, old: &str, new: &str) {
        for map in &mut self.settings.keyboard_maps {
            map.rename_macro(old, new);
        }
        let profiles = self.profiles.iter_mut().chain(self.current.as_mut());
        for profile in profiles.filter(|p| p.run_macro == old) {
            new.clone_into(&mut profile.run_macro);
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
                self.rename_macro(&old, &new);
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
                    macros::PasswordAnswer::Typed(p) => Some(p),
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

#[cfg(any(test, feature = "testing"))]
pub mod test_support;

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
        harness.get_by_label("Settings").click();
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
        harness.get_by_label("Settings").click();
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
        harness.get_by_label("Settings").click();
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
    fn the_terminal_font_has_a_glyph_for_every_character_of_every_code_page_r_ui_6() {
        for font in fonts::BUNDLED {
            let mut harness = Harness::new_ui(move |ui| {
                theme::install(ui.ctx(), font);
                let terminal = fonts::terminal_font(ui.ctx(), 14.0);
                let mut missing = Vec::new();
                for code_page in parley_core::CodePage::ALL {
                    for byte in 0..=u8::MAX {
                        let c = code_page.to_char(byte);
                        let text = c.to_string();
                        if !c.is_control()
                            && !ui.ctx().fonts_mut(|f| f.has_glyphs(&terminal, &text))
                        {
                            missing.push(format!("U+{:04X} ({code_page:?})", u32::from(c)));
                        }
                    }
                }
                assert!(missing.is_empty(), "{font} lacks {missing:?}");
            });
            harness.run();
        }
    }

    /// Closing the window while connected to the session "Local", which confirms exit.
    fn ask_to_close(harness: &mut Harness<'static, ParleyUi>) {
        harness
            .input_mut()
            .viewports
            .entry(egui::ViewportId::ROOT)
            .or_default()
            .events
            .push(egui::ViewportEvent::Close);
        harness.run();
    }

    #[test]
    fn closing_while_connected_asks_first_and_cancel_keeps_the_window_r_ui_5() {
        let (mut harness, _host) = connected_app();
        assert!(
            harness
                .state()
                .current
                .as_ref()
                .is_some_and(|p| p.confirm_exit)
        );
        ask_to_close(&mut harness);
        assert!(harness.state().asking_to_exit);
        assert!(!harness.state().exit_confirmed);
        let before = harness.get_by_label("Exit").rect();
        harness.run_steps(5);
        assert_eq!(
            harness.get_by_label("Exit").rect(),
            before,
            "the dialog keeps its place"
        );
        harness.get_by_label("Cancel").click();
        harness.run();
        assert!(!harness.state().asking_to_exit);
        assert!(!harness.state().exit_confirmed, "the window stays");
        assert!(harness.state().connection.is_some());
    }

    #[test]
    fn exit_in_the_question_lets_the_window_close_r_ui_5() {
        let (mut harness, _host) = connected_app();
        ask_to_close(&mut harness);
        harness.get_by_label("Exit").click();
        harness.run();
        assert!(!harness.state().asking_to_exit);
        assert!(harness.state().exit_confirmed);
    }

    #[test]
    fn closing_without_confirm_exit_does_not_ask_r_ui_5() {
        let (mut harness, _host) = connected_app();
        if let Some(current) = harness.state_mut().current.as_mut() {
            current.confirm_exit = false;
        }
        ask_to_close(&mut harness);
        assert!(!harness.state().asking_to_exit);
    }

    #[test]
    fn a_second_session_with_a_name_in_use_is_refused_r_ui_5() {
        let mut harness = app_with(vec![profile("Prod æøå", "prod")]);
        harness.get_by_label("New session…").click();
        harness.run();
        type_into(&mut harness, "Name", "Prod æøå");
        type_into(&mut harness, "Host", "other");
        harness.get_by_label("Apply").click();
        harness.run();
        harness.get_by_label("There is already a session called \"Prod æøå\".");
        assert_eq!(harness.state().profiles().len(), 1);
    }

    #[test]
    fn connect_in_the_session_dialog_is_off_while_connected_r_ui_5() {
        use egui_kittest::kittest::NodeT;
        let (mut harness, _host) = connected_app();
        harness.state_mut().open_editor(Some(1));
        harness.run();
        let connect = harness.get_by_label("Connect");
        assert!(connect.accesskit_node().is_disabled());
    }

    #[test]
    fn the_menu_bar_starts_with_the_app_and_ends_with_the_settings_gear() {
        // R-UI-10, R-UI-18
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
        let gear = harness.get_by_label("Settings").rect();
        assert!(
            (gear.center().y - file.center().y).abs() < 1.0,
            "in the menu bar: {gear:?}"
        );
        assert!(
            gear.left() > tools.right() + 400.0,
            "the gear on the right: {gear:?}"
        );
        assert!(gear.right() > 950.0, "at the edge: {gear:?}");
        // R-UI-19: the gear is drawn larger than the menus' text (at their size it was 17.5 wide).
        assert!(gear.width() > 21.0, "a larger gear: {gear:?}");
        assert!(
            harness
                .query_all_by_label("Help")
                .all(|n| (n.rect().center().y - file.center().y).abs() >= 1.0),
            "no Help menu (the Keys menu's Help key is another thing)"
        );
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

    /// The gear's Settings, About, Check now (R-UI-18).
    fn check_for_updates(harness: &mut Harness<'static, ParleyUi>) {
        harness.get_by_label("Settings").click();
        harness.run();
        harness.get_by_label("About").click();
        harness.run();
        if let Some(check) = harness.query_by_label("Check now") {
            check.click();
        }
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
        // The check at start-up finds it; a second one from Check now would race with it.
        wait_for(&mut harness, "Parley 9.9.9 is available");
        harness.get_by_label("Settings").click();
        harness.run();
        harness.get_by_label("About").click();
        harness.run();
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
    fn settings_open_from_the_gear_not_from_tools_r_ui_18() {
        let mut harness = app_with(Vec::new());
        harness.get_by_label("Tools").click();
        harness.run();
        assert!(
            harness.query_by_label("Settings…").is_none(),
            "not in Tools"
        );
        harness.key_press(egui::Key::Escape);
        harness.run();
        harness.get_by_label("Settings").click();
        harness.run();
        harness.get_by_label("Appearance");
        assert!(harness.state().settings_dialog.is_some());
    }

    #[test]
    fn the_header_and_status_bar_are_there_before_connecting_without_the_keys_menu() {
        // R-UI-15, R-UI-16: one header row; the Keys menu only while connected.
        let harness = app_with(Vec::new());
        for menu in ["File", "Edit", "View", "Tools", "Settings", "Offline"] {
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
                        reconnect_in: None,
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
    fn the_error_pill_shows_the_5250_code_before_the_message_r_key_9() {
        let mut session = test_support::waiting_session();
        for c in "abcdefghij".chars() {
            session.key(parley_core::Key::Char(c));
        }
        session.key(parley_core::Key::Home);
        session.key(parley_core::Key::ToggleInsert);
        session.key(parley_core::Key::Char('x'));
        let harness = status_harness(session, status::Clock::default(), 900.0);
        harness.get_by_label("X 0012 No room to insert: the field is full.");
    }

    #[test]
    fn the_status_bar_counts_the_keys_typed_ahead_r_key_11() {
        let mut session = test_support::waiting_session();
        session.set_type_ahead(true);
        session.key(parley_core::Key::Aid(parley_core::Aid::ENTER));
        session.key(parley_core::Key::Char('a'));
        session.key(parley_core::Key::Char('b'));
        let harness = status_harness(session, status::Clock::default(), 900.0);
        harness.get_by_label("2 keys typed ahead");
    }

    #[test]
    fn window_frames_are_on_by_default_and_the_terminal_page_turns_them_off_r_ui_26() {
        assert!(Settings::default().window_frames);
        let mut harness = app_with(Vec::new());
        harness.get_by_label("Settings").click();
        harness.run();
        harness.get_by_label("Terminal").click();
        harness.run();
        harness.get_by_label("Window frames").scroll_to_me();
        harness.run();
        harness.get_by_label("Window frames").click();
        harness.run();
        harness.get_by_label("Save").click();
        harness.run();
        assert!(!harness.state().settings().window_frames);
    }

    #[test]
    fn the_terminal_page_turns_type_ahead_on_r_key_11() {
        let mut harness = app_with(Vec::new());
        harness.get_by_label("Settings").click();
        harness.run();
        harness.get_by_label("Terminal").click();
        harness.run();
        harness.get_by_label("Type-ahead").scroll_to_me();
        harness.run();
        harness.get_by_label("Type-ahead").click();
        harness.run();
        harness.get_by_label("Save").click();
        harness.run();
        assert!(harness.state().settings().type_ahead);
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

    #[test]
    fn edit_find_opens_the_bar_and_esc_closes_it_r_ui_21() {
        let mut harness = app_with(Vec::new());
        let _host = connect_locally(&mut harness, "");
        harness.get_by_label("Edit").click();
        harness.run();
        harness.get_by_label("Find…").click();
        harness.run();
        assert!(harness.state().view.find.is_some());
        assert!(harness.get_by_label("Find on screen").is_focused());
        harness.key_press(egui::Key::Escape);
        harness.run();
        assert!(harness.state().view.find.is_none());
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
        harness.get_by_label("Settings").click();
        harness.run();
        harness.get_by_label("Add-ons").click();
        harness.run();
        harness
            .get_by_role_and_label(egui::accesskit::Role::CheckBox, name)
            .click();
        harness.run();
    }

    #[test]
    fn parleys_settings_have_none_of_parley_sqls_only_the_add_on_r_sql_69() {
        let mut harness = app_with(Vec::new());
        switch_add_on_in_settings(&mut harness, "SQL");
        harness.get_by_label(settings::SQL_SETTINGS_ELSEWHERE);
        for gone in [
            "SQL assistant",
            "Java",
            "Rows at a time",
            "Found",
            "Shortcuts",
        ] {
            assert!(
                harness.query_by_label(gone).is_none(),
                "{gone} is Parley SQL's"
            );
        }
        let before = harness.get_by_label("Save").rect();
        harness.run_steps(5);
        assert_eq!(harness.get_by_label("Save").rect(), before, "D12");
        harness.get_by_label("Appearance").click();
        harness.run();
        assert!(harness.query_by_label("SQL window").is_none());
        assert!(harness.query_by_label("Results").is_none());
        harness.get_by_label("Save").click();
        harness.run();
        assert!(harness.state().addon_on(AddOn::Sql));
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
        // R-UI-18: with neither macros nor SQL, Tools would be empty, so it is not there.
        assert!(harness.query_by_label("Tools").is_none());
        for item in ["Record macro", "Run macro", "Macros…"] {
            assert!(harness.query_by_label(item).is_none(), "{item} shown");
        }
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

    /// The gear's Settings, then the Keyboard page.
    fn open_keyboard_settings(harness: &mut Harness<'static, ParleyUi>) {
        harness.get_by_label("Settings").click();
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

    fn choose_screen_colors(harness: &mut Harness<'static, ParleyUi>, scheme: &str) {
        harness.get_by_label("Settings").click();
        harness.run();
        harness.get_by_label("Terminal").click();
        harness.run();
        harness.get_by_label(scheme).click();
        harness.run();
    }

    #[test]
    fn the_screen_colours_change_at_once_and_cancel_puts_them_back_r_ui_23() {
        let mut harness = app_with(Vec::new());
        choose_screen_colors(&mut harness, "Light");
        assert_eq!(harness.state().view.palette, terminal::Palette::light());
        harness.get_by_label("Cancel").click();
        harness.run();
        assert_eq!(
            harness.state().settings().screen_colors,
            terminal::Scheme::Classic
        );
        assert_eq!(harness.state().view.palette, terminal::Palette::classic());
    }

    #[test]
    fn custom_colours_offer_a_button_per_colour_and_keep_the_dialog_in_place_r_ui_23() {
        let mut harness = app_with(Vec::new());
        choose_screen_colors(&mut harness, "Custom");
        harness.get_by_label("Background");
        harness.get_by_role_and_label(egui::accesskit::Role::Button, "Turquoise");
        let before = harness.get_by_label("Save").rect();
        harness.run_steps(5);
        assert_eq!(harness.get_by_label("Save").rect(), before);
        harness.get_by_label("Save").click();
        harness.run();
        assert_eq!(
            harness.state().settings().screen_colors,
            terminal::Scheme::Custom
        );
    }

    #[test]
    fn a_session_s_own_colours_win_over_the_settings_r_ui_23() {
        let mut harness = app_with(Vec::new());
        let mut prod = profile("Prod", "127.0.0.1");
        prod.screen_colors = Some(terminal::Scheme::Light);
        harness.state_mut().current = Some(prod);
        harness.run();
        assert_eq!(harness.state().view.palette, terminal::Palette::light());
        harness
            .state_mut()
            .current
            .as_mut()
            .expect("set")
            .screen_colors = None;
        harness.run();
        assert_eq!(harness.state().view.palette, terminal::Palette::classic());
    }

    #[test]
    fn the_session_dialog_chooses_the_screen_colours_r_ui_23() {
        let mut harness = app_with(Vec::new());
        harness.get_by_label("New session…").click();
        harness.run();
        type_into(&mut harness, "Name", "Prod");
        type_into(&mut harness, "Host", "prod");
        harness.get_by_label("5250 colours").click();
        harness.run();
        harness.get_by_label("Light").click();
        harness.run();
        harness.get_by_label("Apply").click();
        harness.run();
        assert_eq!(
            harness.state().profiles()[0].screen_colors,
            Some(terminal::Scheme::Light)
        );
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

    /// Frames until `done` says the app is where the test wants it (the network has its own
    /// thread), at most two seconds.
    fn run_until(harness: &mut Harness<'static, ParleyUi>, done: impl Fn(&ParleyUi) -> bool) {
        for _ in 0..200 {
            if done(harness.state()) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
            harness.run();
        }
        panic!("the app did not get there");
    }

    /// Connects "Local" to a host on a local port that accepts once and then resets the
    /// connection (DO TERMINAL-TYPE, the answer left unread), as a network failure does.
    fn drop_locally(
        harness: &mut Harness<'static, ParleyUi>,
        reconnect: bool,
    ) -> std::net::TcpListener {
        use std::io::Write as _;
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a local port");
        let mut local = profile("Local", "127.0.0.1");
        local.config.port = listener.local_addr().expect("its address").port();
        local.config.tls = false;
        local.reconnect = reconnect;
        let ctx = harness.ctx.clone();
        harness.state_mut().connect(&ctx, local);
        let (mut socket, _) = listener.accept().expect("Parley connects");
        socket.write_all(&[255, 253, 24]).expect("send");
        std::thread::sleep(std::time::Duration::from_millis(300));
        drop(socket);
        run_until(harness, |app| app.connection.is_none());
        listener
    }

    /// Connects "Local" to a host on a local port that sends one screen with the alarm bit set
    /// (Write To Display, CC2 X'04'); returns the beeps counted.
    fn alarm_from_host(
        alarm: settings::Alarm,
    ) -> (
        Harness<'static, ParleyUi>,
        std::rc::Rc<std::cell::Cell<u32>>,
    ) {
        use std::io::Write as _;
        let mut harness = app_with(Vec::new());
        let beeps = std::rc::Rc::new(std::cell::Cell::new(0));
        let counted = std::rc::Rc::clone(&beeps);
        harness
            .state_mut()
            .set_beep(Box::new(move || counted.set(counted.get() + 1)));
        harness.state_mut().set_settings(Settings {
            alarm,
            ..Settings::default()
        });
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a local port");
        let mut local = profile("Local", "127.0.0.1");
        local.config.port = listener.local_addr().expect("its address").port();
        local.config.tls = false;
        let ctx = harness.ctx.clone();
        harness.state_mut().connect(&ctx, local);
        let (mut socket, _) = listener.accept().expect("Parley connects");
        let record = parley_core::record::build(
            0,
            parley_core::record::Opcode::PutGet,
            &[0x04, 0x11, 0x00, 0x04, 0x11, 1, 1, 0xC1], // ... SBA 1,1 "A"
        );
        socket
            .write_all(&parley_core::telnet::Telnet::frame(&record))
            .expect("send");
        run_until(&mut harness, |app| {
            app.connection.as_ref().is_some_and(|c| {
                c.session()
                    .screen()
                    .text(parley_core::CodePage::Ccsid37)
                    .starts_with('A')
            })
        });
        harness.run();
        drop(socket);
        (harness, beeps)
    }

    #[test]
    fn the_host_s_alarm_beeps_and_flashes_once_r_ui_25() {
        let (harness, beeps) = alarm_from_host(settings::Alarm::SoundAndFlash);
        assert_eq!(beeps.get(), 1);
        assert!(harness.state().view.flashing_or(false));
    }

    #[test]
    fn flash_only_does_not_beep_r_ui_25() {
        let (_harness, beeps) = alarm_from_host(settings::Alarm::FlashOnly);
        assert_eq!(beeps.get(), 0);
    }

    #[test]
    fn the_alarm_set_off_neither_beeps_nor_flashes_r_ui_25() {
        let (harness, beeps) = alarm_from_host(settings::Alarm::Off);
        assert_eq!(beeps.get(), 0);
        assert!(!harness.state().view.flashing_or(false));
    }

    #[test]
    fn the_terminal_page_chooses_the_alarm_and_turns_blinking_off_r_ui_24_r_ui_25() {
        let mut harness = app_with(Vec::new());
        harness.get_by_label("Settings").click();
        harness.run();
        harness.get_by_label("Terminal").click();
        harness.run();
        harness.get_by_label("Flash").click();
        harness.get_by_label("Blinking text").click();
        harness.run();
        harness.get_by_label("Save").click();
        harness.run();
        assert_eq!(harness.state().settings().alarm, settings::Alarm::FlashOnly);
        assert!(!harness.state().settings().blinking_text);
    }

    #[test]
    fn a_dropped_connection_counts_down_to_connecting_again_r_net_10() {
        let mut harness = app_with(Vec::new());
        let host = drop_locally(&mut harness, true);
        harness.run();
        harness.get_by_label_contains("Reconnecting in 2 s");
        let note = harness.state().closed.clone().unwrap_or_default();
        assert!(note.starts_with("The connection dropped"), "{note}");

        // The wait is over: Parley connects again, and the countdown goes.
        let soon = std::time::Instant::now();
        harness.state_mut().reconnect = Some(reconnect::Reconnect::Waiting {
            attempt: 0,
            at: soon,
        });
        harness.run();
        let (_again, _) = host.accept().expect("Parley connects again");
        run_until(&mut harness, |app| app.reconnect.is_none() && app.online);
        assert!(harness.query_by_label_contains("Reconnecting").is_none());
    }

    #[test]
    fn a_failed_try_waits_longer_and_cancel_stops_reconnecting_r_net_10() {
        let mut harness = app_with(Vec::new());
        let host = drop_locally(&mut harness, true);
        drop(host); // nothing listens any more: the next try fails
        let soon = std::time::Instant::now();
        harness.state_mut().reconnect = Some(reconnect::Reconnect::Waiting {
            attempt: 0,
            at: soon,
        });
        run_until(&mut harness, |app| {
            matches!(
                app.reconnect,
                Some(reconnect::Reconnect::Waiting { attempt: 1, .. })
            )
        });
        harness.run();
        harness.get_by_label_contains("Reconnecting in 4 s");
        harness.get_by_label("Cancel").click();
        harness.run();
        assert_eq!(harness.state().reconnect, None);
        assert!(harness.query_by_label_contains("Reconnecting").is_none());
    }

    #[test]
    fn without_the_setting_a_dropped_connection_stays_offline_r_net_10() {
        let mut harness = app_with(Vec::new());
        let _host = drop_locally(&mut harness, false);
        harness.run();
        assert_eq!(harness.state().reconnect, None);
        let note = harness.state().closed.clone().unwrap_or_default();
        assert!(note.starts_with("The connection dropped"), "{note}");
    }

    #[test]
    fn the_session_dialog_turns_reconnecting_on_r_net_10() {
        let mut harness = app_with(Vec::new());
        harness.get_by_label("New session…").click();
        harness.run();
        type_into(&mut harness, "Name", "Prod");
        type_into(&mut harness, "Host", "prod");
        harness
            .get_by_label("Reconnect when the connection drops")
            .click();
        harness.run();
        harness.get_by_label("Apply").click();
        harness.run();
        assert!(harness.state().profiles()[0].reconnect);
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
    fn ctrl_slash_opens_the_shortcut_sheet_while_macros_are_off_r_ui_17() {
        let (mut harness, _host) = connected_app();
        harness.state_mut().set_settings(macros_off());
        harness.run();
        harness.key_press_modifiers(egui::Modifiers::CTRL, egui::Key::Slash);
        harness.run();
        assert!(harness.state().shortcut_sheet.is_some());
    }

    #[test]
    fn the_sql_key_starts_parley_sql_while_macros_are_off_r_sql_70() {
        let (mut harness, _host) = connected_app();
        let started = with_sql_starter(&mut harness);
        let mut settings = macros_off();
        settings.addons.set_enabled(AddOn::Sql, true);
        harness.state_mut().set_settings(settings);
        harness.run();
        let app = harness.state_mut();
        let connection = app.connection.take().expect("connected");
        app.carry_out_keys(vec![Function::Sql], &mut connection.session());
        app.connection = Some(connection);
        assert_eq!(started.lock().expect("not poisoned").len(), 1);
    }

    #[test]
    fn applying_the_connected_session_updates_the_session_in_use_r_ui_5() {
        let (mut harness, _host) = connected_app();
        let port = harness
            .state()
            .current
            .as_ref()
            .expect("in use")
            .config
            .port;
        harness.get(By::new().label_contains("127.0.0.1:")).click();
        harness.run();
        harness.get_by_label("Edit session…").click();
        harness.run();
        type_into(&mut harness, "Name", " æøå");
        harness.get_by_label("Apply").click();
        harness.run();
        let app = harness.state();
        let current = app.current.as_ref().expect("in use");
        assert_eq!(current.name, "Local æøå");
        assert_eq!(current.config.port, port, "the connection's settings stay");
        assert_eq!(app.current_index(), Some(0), "Connect again finds it");
        assert_eq!(app.title(), "Local æøå – Parley");
    }

    #[test]
    fn renaming_a_macro_keeps_the_sessions_that_run_it_r_key_4() {
        let prod = Profile {
            run_macro: "Jobs".to_owned(),
            ..profile("Prod", "prod")
        };
        let mut harness = app_with(vec![prod.clone()]);
        harness
            .state_mut()
            .set_macro_store(Box::new(test_support::macros_with("Jobs")));
        harness.state_mut().current = Some(prod);
        harness.state_mut().macro_manager = Some(macros::Manager::default());
        harness.run();
        harness.get_by_label("Rename").click();
        harness.run();
        type_into(&mut harness, "New name", " 2");
        harness.get_by_label("Save name").click();
        harness.run();
        let app = harness.state();
        assert_eq!(app.profiles()[0].run_macro, "Jobs 2");
        assert_eq!(
            app.current.as_ref().map(|p| p.run_macro.as_str()),
            Some("Jobs 2")
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

    /// Settings with the SQL add-on on, and a starter that keeps what it was asked to open.
    fn with_sql_starter(
        harness: &mut Harness<'static, ParleyUi>,
    ) -> std::sync::Arc<std::sync::Mutex<Vec<(String, parley_core::SessionConfig)>>> {
        let mut settings = Settings::default();
        settings.addons.set_enabled(AddOn::Sql, true);
        harness.state_mut().set_settings(settings);
        let started = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let kept = std::sync::Arc::clone(&started);
        harness
            .state_mut()
            .set_sql_starter(Box::new(move |name, config| {
                kept.lock()
                    .expect("not poisoned")
                    .push((name.to_owned(), config.clone()));
                Ok(())
            }));
        started
    }

    #[test]
    fn tools_sql_starts_parley_sql_on_the_sessions_host_r_sql_70() {
        // R-SQL-6's bug stays fixed: the menu's action ran while the frame held the
        // connection, so it found no connection and did nothing.
        let (mut harness, _host) = connected_app();
        let started = with_sql_starter(&mut harness);
        harness.run();
        harness.get_by_label("Tools").click();
        harness.run();
        harness.get_by_label("SQL…").click();
        harness.run();
        let profile = harness.state().current.clone().expect("a session in use");
        let started = started.lock().expect("not poisoned").clone();
        assert_eq!(started.len(), 1, "one Parley SQL");
        assert_eq!(started[0].0, profile.name);
        assert_eq!(started[0].1, profile.config);
    }

    #[test]
    fn when_parley_sql_cannot_start_parley_says_why_r_sql_70() {
        let (mut harness, _host) = connected_app();
        with_sql_starter(&mut harness);
        harness.state_mut().set_sql_starter(Box::new(|_, _| {
            Err("Parley SQL is not installed beside Parley.".into())
        }));
        harness.state_mut().start_parley_sql(true);
        harness.run();
        harness.get_by_label_contains("not installed beside Parley");
        harness.get_by_label("OK").click();
        harness.run();
        assert!(harness.query_by_label_contains("not installed").is_none());
    }

    #[test]
    fn without_a_connection_tools_sql_starts_nothing_r_sql_70() {
        let mut harness = app_with(Vec::new());
        let started = with_sql_starter(&mut harness);
        harness.state_mut().start_parley_sql(false);
        harness.run();
        assert!(started.lock().expect("not poisoned").is_empty());
    }
}
