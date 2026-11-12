//! Parley's user interface, built with egui.
//!
//! No platform code lives here: the launcher (`parley-app`) opens the window and hands the UI
//! an [`egui::Ui`] to draw in each frame.

mod connect;
mod dialog;
mod fonts;
mod keymap;
mod menu;
mod profile;
mod sessions;
mod settings;
mod status;
mod terminal;
mod theme;

use egui::{CentralPanel, Frame, Margin, Panel, Ui, ViewportCommand};
use parley_core::net::{Connection, LinkState};

use keymap::Keymap;
use menu::Action;
pub use profile::{Keychain, NoKeychain, Profile};
use sessions::{Editor, Outcome};
pub use settings::{Settings, Theme};
pub use theme::Density;

/// The whole application UI: menu bar, toolbar and status bar around the start screen or the
/// open session's screen.
#[derive(Debug)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "independent UI flags: panel shown, focus wanted, close confirmed"
)]
pub struct ParleyUi {
    profiles: Vec<Profile>,
    keychain: Box<dyn Keychain>,
    /// The session in use, or the last one used.
    current: Option<Profile>,
    connection: Option<Connection>,
    /// The connection got through: when it ends, the host ended it.
    online: bool,
    /// Why the last connection ended, shown on the start screen and in the status bar.
    closed: Option<String>,
    /// A problem to show next to the keyboard state, such as an unreadable keychain.
    warning: Option<String>,
    editor: Option<Editor>,
    settings: Settings,
    /// The settings in effect; `None` until the first frame puts them into effect.
    applied: Option<Settings>,
    settings_dialog: Option<settings::Dialog>,
    keymap: Keymap,
    /// Give the terminal the keyboard focus on the next frame.
    focus_terminal: bool,
    show_shortcuts: bool,
    /// The operator is being asked whether to close the window.
    asking_to_exit: bool,
    /// Closing is decided: the next close request goes through.
    exit_confirmed: bool,
}

impl Default for ParleyUi {
    fn default() -> Self {
        Self::new(Vec::new(), Box::new(NoKeychain))
    }
}

impl ParleyUi {
    /// Starts on the start screen with the saved sessions; passwords come from `keychain`.
    pub fn new(profiles: Vec<Profile>, keychain: Box<dyn Keychain>) -> Self {
        Self {
            profiles,
            keychain,
            current: None,
            connection: None,
            online: false,
            closed: None,
            warning: None,
            editor: None,
            settings: Settings::default(),
            applied: None,
            settings_dialog: None,
            keymap: Keymap::default(),
            focus_terminal: false,
            show_shortcuts: false,
            asking_to_exit: false,
            exit_confirmed: false,
        }
    }

    /// The settings, for the launcher to keep.
    pub const fn settings(&self) -> &Settings {
        &self.settings
    }

    /// Replaces the settings; they take effect on the next frame.
    pub fn set_settings(&mut self, settings: Settings) {
        self.settings = settings;
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

    /// Connects to `profile`, with its password from the keychain if it signs on by itself.
    pub fn connect(&mut self, ctx: &egui::Context, profile: Profile) {
        let mut config = profile.config.clone();
        self.warning = None;
        if config.sign_on != parley_core::signon::SignOn::Off {
            match self.keychain.password(&profile.name) {
                Ok(Some(password)) => {
                    config.password = parley_core::signon::Password::new(password);
                }
                Ok(None) => self.warning = Some("Auto sign-on: no password saved.".to_owned()),
                Err(e) => self.warning = Some(format!("Auto sign-on: {e}")),
            }
        }
        let ctx = ctx.clone();
        self.connection = Some(Connection::open(config, move || ctx.request_repaint()));
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

    fn open_editor(&mut self, index: Option<usize>) {
        self.editor = Some(Editor::open(&self.profiles, index, self.keychain.as_ref()));
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
        let connected = session.is_some();
        let mut actions = Vec::new();

        // Made before the panels borrow `ui`.
        let menu_frame = theme::bar(ui, Margin::symmetric(8, 3));
        let tool_frame = theme::bar(ui, Margin::symmetric(10, 6));
        let status_frame = theme::bar(ui, Margin::symmetric(8, 5));
        Panel::top("menu_bar").frame(menu_frame).show(ui, |ui| {
            actions.extend(menu::menu_bar(
                ui,
                connected,
                self.show_shortcuts,
                &self.profiles,
            ));
        });
        Panel::top("toolbar").frame(tool_frame).show(ui, |ui| {
            let config = self
                .current
                .as_ref()
                .filter(|_| connected)
                .map(|p| &p.config);
            actions.extend(menu::toolbar(ui, config));
        });
        Panel::bottom("status_bar")
            .frame(status_frame)
            .show(ui, |ui| {
                status::status_bar(
                    ui,
                    session.as_deref(),
                    link.as_ref(),
                    self.current.as_ref().is_some_and(|p| p.config.tls),
                    self.closed.as_deref().or(self.warning.as_deref()),
                );
            });
        if self.show_shortcuts {
            Panel::right("shortcuts").show(ui, |ui| {
                if shortcuts(ui) {
                    actions.push(Action::ToggleShortcuts);
                }
            });
        }
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
                    terminal::show(ui, session, &mut self.keymap, grab);
                });
        } else {
            CentralPanel::default().show(ui, |ui| {
                match connect::show(ui, &self.profiles, self.closed.as_deref()) {
                    Some(connect::Choice::Connect(i)) => actions.push(Action::Connect(i)),
                    Some(connect::Choice::Edit(i)) => self.open_editor(Some(i)),
                    Some(connect::Choice::New) => actions.push(Action::NewSession),
                    None => {}
                }
            });
        }

        let next = self.carry_out(ui.ctx(), actions, session.as_deref_mut());
        drop(session);
        match connection {
            Some(connection) if next.disconnect => {
                connection.close();
                self.closed = Some("Disconnected.".to_owned());
            }
            other => self.connection = other,
        }
        // Only now: the connection taken for this frame is back, so a new one is not overwritten.
        if let Some(index) = next.connect {
            self.connect_saved(ui.ctx(), index);
        }

        self.show_editor(ui.ctx());
        self.show_settings(ui.ctx());
        self.show_exit_question(ui.ctx());
    }

    /// Does what the menus, toolbar and start screen asked, except opening and closing the
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
                Action::Connect(i) if !connected => next.connect = Some(i),
                Action::Connect(_) => {}
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
                Action::Settings => {
                    self.settings_dialog = Some(settings::Dialog::open(ctx, &self.settings));
                }
                Action::ToggleShortcuts => self.show_shortcuts = !self.show_shortcuts,
                Action::Key(key) => {
                    if let Some(s) = session.as_deref_mut() {
                        s.key(key);
                        self.focus_terminal = true;
                        ctx.request_repaint();
                    }
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
                self.connection = None;
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

    fn show_settings(&mut self, ctx: &egui::Context) {
        let Some(dialog) = &mut self.settings_dialog else {
            return;
        };
        match dialog.show(ctx, &mut self.settings) {
            Some(settings::Closed::Keep) => self.settings_dialog = None,
            Some(settings::Closed::Revert) => {
                self.settings = dialog.original().clone();
                self.settings_dialog = None;
            }
            None => {}
        }
    }

    fn show_editor(&mut self, ctx: &egui::Context) {
        let Some(editor) = &mut self.editor else {
            return;
        };
        let connected = self.connection.is_some();
        let outcome = editor.show(ctx, &mut self.profiles, self.keychain.as_ref(), connected);
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

/// What to do with the connection once a frame is drawn.
#[derive(Debug, Default)]
struct Next {
    disconnect: bool,
    /// Connect to the saved session at this index.
    connect: Option<usize>,
}

/// The keyboard shortcuts, in a side panel; returns true when its close button was clicked.
fn shortcuts(ui: &mut Ui) -> bool {
    let close = dialog::title_bar(ui, "Keyboard shortcuts");
    ui.add_space(4.0);
    egui::Grid::new("shortcuts").striped(true).show(ui, |ui| {
        for (pc, function) in keymap::SHORTCUTS {
            ui.label(*pc);
            ui.label(*function);
            ui.end_row();
        }
    });
    close
}

/// Sessions for the UI tests.
#[cfg(test)]
pub(crate) mod test_support {
    use parley_core::record::{self, Opcode};
    use parley_core::telnet::Telnet;
    use parley_core::{CodePage, Session, SessionConfig};
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::rc::Rc;

    use crate::{Keychain, Profile};

    /// A session showing "Hello" and one input field at row 2, column 2, waiting for input.
    pub fn waiting_session() -> Session {
        let mut data = vec![0x04, 0x40, 0x04, 0x11, 0x00, 0x00, 0x11, 1, 1];
        data.extend("Hello".chars().filter_map(|c| CodePage::Ccsid37.to_byte(c)));
        data.extend([
            0x11, 2, 1, 0x1D, 0x40, 0x00, 0x24, 0x00, 0x0A, 0x04, 0x52, 0x00, 0x00,
        ]);
        let mut session = Session::new(SessionConfig::default());
        session.receive(&Telnet::frame(&record::build(0, Opcode::PutGet, &data)));
        session
    }

    /// A keychain in memory, shared with the test so it can look inside.
    #[derive(Debug, Clone, Default)]
    pub struct MemoryKeychain(pub Rc<RefCell<HashMap<String, String>>>);

    impl Keychain for MemoryKeychain {
        fn password(&self, session: &str) -> Result<Option<String>, String> {
            Ok(self.0.borrow().get(session).cloned())
        }

        fn set_password(&self, session: &str, password: &str) -> Result<(), String> {
            self.0
                .borrow_mut()
                .insert(session.to_owned(), password.to_owned());
            Ok(())
        }

        fn delete_password(&self, session: &str) -> Result<(), String> {
            self.0.borrow_mut().remove(session);
            Ok(())
        }
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
    use egui_kittest::kittest::Queryable;

    use test_support::{MemoryKeychain, profile};

    fn app_with(profiles: Vec<Profile>, keychain: &MemoryKeychain) -> Harness<'static, ParleyUi> {
        let app = ParleyUi::new(profiles, Box::new(keychain.clone()));
        Harness::builder()
            .with_size(egui::vec2(1000.0, 800.0))
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
        let mut harness = app_with(Vec::new(), &MemoryKeychain::default());
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
        let mut harness = app_with(Vec::new(), &MemoryKeychain::default());
        choose_terminal_font(&mut harness, "Hack");
        harness.get_by_label("Close").click();
        harness.run();
        assert!(harness.state().settings_dialog.is_none());
        assert_eq!(harness.state().settings(), &Settings::default());
    }

    #[test]
    fn the_close_button_of_the_session_dialog_closes_it_unsaved() {
        // R-UI-7
        let mut harness = app_with(Vec::new(), &MemoryKeychain::default());
        harness.get_by_label("New session…").click();
        harness.run();
        type_into(&mut harness, "Name", "Unsaved");
        harness.get_by_label("Close").click();
        harness.run();
        assert!(harness.state().editor.is_none());
        assert_eq!(harness.state().profiles(), []);
    }

    #[test]
    fn the_shortcuts_panel_closes_with_its_close_button() {
        // R-UI-7
        let mut harness = app_with(Vec::new(), &MemoryKeychain::default());
        harness.state_mut().show_shortcuts = true;
        harness.run();
        harness.get_by_label("Close").click();
        harness.run();
        assert!(!harness.state().show_shortcuts);
    }

    #[test]
    fn restore_defaults_resets_only_the_page_shown() {
        // R-UI-8: the theme (Appearance) survives restoring the Terminal page.
        let mut harness = app_with(Vec::new(), &MemoryKeychain::default());
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
        let mut harness = app_with(Vec::new(), &MemoryKeychain::default());
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
        let mut harness = app_with(Vec::new(), &MemoryKeychain::default());
        harness.get_by_label("New session…").click();
        harness.run();
        let before = harness.get_by_label("Apply").rect();
        harness.run_steps(5);
        assert_eq!(harness.get_by_label("Apply").rect(), before);
    }

    #[test]
    fn cancel_puts_the_old_settings_back() {
        // R-UI-6
        let mut harness = app_with(Vec::new(), &MemoryKeychain::default());
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
            for symbol in ["⏵", "⏷", "🗙", "·", "×", "…", "–"] {
                // A missing glyph would draw as an empty box.
                let found = ui
                    .ctx()
                    .fonts_mut(|f| f.has_glyphs(&egui::FontId::proportional(13.0), symbol));
                assert!(found, "the bundled fonts have no glyph for {symbol}");
                ui.label(symbol);
                ui.label(theme::semibold(symbol));
            }
        });
        harness.run();
    }

    #[test]
    fn the_start_screen_lists_the_saved_sessions() {
        // R-UI-5
        let harness = app_with(
            vec![
                profile("Production", "prod.example.com"),
                profile("Test", "test.example.com"),
            ],
            &MemoryKeychain::default(),
        );
        harness.get_by_label("Connect to IBM i");
        harness.get_by_label("Production");
        harness.get_by_label("Test");
        assert_eq!(harness.get_all_by_label("Connect").count(), 2);
        harness.get_by_label("New session…");
    }

    #[test]
    fn connect_on_a_session_card_opens_that_session() {
        // R-UI-5: the start screen's Connect button.
        let mut harness = app_with(
            vec![
                profile("Production", "127.0.0.1"),
                profile("Test", "127.0.0.1"),
            ],
            &MemoryKeychain::default(),
        );
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
        let mut harness = app_with(
            vec![profile("Test", "127.0.0.1")],
            &MemoryKeychain::default(),
        );
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
    fn a_new_session_is_saved_with_its_password_in_the_keychain_only() {
        // R-UI-5, R-P-3
        let keychain = MemoryKeychain::default();
        let mut harness = app_with(Vec::new(), &keychain);
        harness.get_by_label("New session…").click();
        harness.run();
        type_into(&mut harness, "Name", "Production");
        type_into(&mut harness, "Host", "prod.example.com");
        type_into(&mut harness, "User", "qsecofr");
        type_into(&mut harness, "Password", "secret");
        harness.get_by_label("Enable auto sign-on").click();
        harness.run();
        harness.get_by_label("Apply").click();
        harness.run();

        let saved = harness.state().profiles();
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].name, "Production");
        assert_eq!(saved[0].config.host, "prod.example.com");
        assert_eq!(saved[0].config.sign_on, parley_core::signon::SignOn::Screen);
        assert!(saved[0].config.tls, "new sessions use TLS (R-P-3)");
        assert_eq!(saved[0].config.port, 992);
        assert!(saved[0].config.password.is_empty());
        assert_eq!(
            keychain.0.borrow().get("Production").map(String::as_str),
            Some("secret")
        );
    }

    #[test]
    fn a_session_needs_a_name_and_a_host() {
        // R-UI-5
        let mut harness = app_with(Vec::new(), &MemoryKeychain::default());
        harness.get_by_label("New session…").click();
        harness.run();
        harness.get_by_label("Apply").click();
        harness.run();
        harness.get_by_label("Give the session a name.");
        assert_eq!(harness.state().profiles(), []);
    }

    #[test]
    fn deleting_a_session_removes_its_password() {
        // R-UI-5
        let keychain = MemoryKeychain::default();
        keychain
            .set_password("Test", "pw")
            .expect("memory keychain");
        let mut harness = app_with(
            vec![
                profile("Production", "prod.example.com"),
                profile("Test", "test.example.com"),
            ],
            &keychain,
        );
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
        assert!(keychain.0.borrow().is_empty());
    }

    #[test]
    fn renaming_a_session_moves_its_password() {
        // R-UI-5
        let keychain = MemoryKeychain::default();
        keychain
            .set_password("Test", "pw")
            .expect("memory keychain");
        let mut harness = app_with(vec![profile("Test", "test.example.com")], &keychain);
        harness.get_by_label("Edit…").click();
        harness.run();
        type_into(&mut harness, "Name", " 2");
        harness.get_by_label("Apply").click();
        harness.run();
        assert_eq!(harness.state().profiles()[0].name, "Test 2");
        let stored: Vec<_> = keychain.0.borrow().keys().cloned().collect();
        assert_eq!(stored, ["Test 2"]);
    }

    #[test]
    fn only_one_session_connects_on_program_start() {
        // R-UI-5
        let mut first = profile("Production", "prod.example.com");
        first.connect_on_start = true;
        let mut harness = app_with(
            vec![first, profile("Test", "test.example.com")],
            &MemoryKeychain::default(),
        );
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
    fn the_menu_bar_toolbar_and_status_bar_are_there_before_connecting() {
        // R-UI-4
        let harness =
            Harness::new_ui_state(|ui, app: &mut ParleyUi| app.ui(ui), ParleyUi::default());
        for menu in ["File", "Edit", "View", "Tools"] {
            harness.get_by_label(menu);
        }
        assert_eq!(
            harness.get_all_by_label("Help").count(),
            2,
            "the menu and the toolbar key"
        );
        harness.get_by_label("Attn");
        harness.get_by_label("Offline");
    }

    #[test]
    fn the_shortcuts_panel_opens_from_the_view_menu() {
        let mut harness =
            Harness::new_ui_state(|ui, app: &mut ParleyUi| app.ui(ui), ParleyUi::default());
        harness.get_by_label("View").click();
        harness.run();
        harness.get_by_label("Keyboard shortcuts").click();
        harness.run();
        assert!(harness.state().show_shortcuts);
        harness.get_by_label("Field Exit");
    }

    #[test]
    fn the_status_bar_shows_the_keyboard_connection_and_cursor() {
        // R-UI-2
        let session = test_support::waiting_session();
        let harness = Harness::new_ui(move |ui| {
            status::status_bar(ui, Some(&session), Some(&LinkState::Connected), true, None);
        });
        for text in ["Ready", "Online", "TLS", "2,2"] {
            harness.get_by_label(text);
        }
    }

    #[test]
    fn turning_tls_off_moves_the_port_back_to_23() {
        let mut harness = app_with(Vec::new(), &MemoryKeychain::default());
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
}
