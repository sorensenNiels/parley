//! Parley SSH's UI (D41): the start screen with the saved sessions (R-SSH-11), the SSH session in the main window
//! (one per window, D45), under a header and over a status bar, in the look Parley 5250 and Parley SQL share (D12).
//! The engine is `parley-ssh-core`; the launcher (`parley-ssh` in `parley-app`) keeps the settings, reads the
//! command line (R-SSH-14) and opens other windows (R-SSH-15).

mod bars;
mod dialogs;
pub mod screen;
mod session;
pub mod sessions;
pub mod settings;
mod start;

use std::sync::Arc;

use egui::{CentralPanel, Margin, Panel, Ui, ViewportCommand};
use parley_ssh_core::known_hosts::KnownHosts;
use parley_ssh_core::net::Target;
use parley_ui_egui::menu::{Logo, Program};
use parley_ui_egui::{theme, updates};

use bars::Action;
use session::Session;
use sessions::{Edited, Editor, SavedSession};
pub use settings::Settings;

/// The smallest and largest text size View offers, in percent, and its step.
const TEXT_SIZES: (u16, u16, u16) = (60, 250, 10);

/// Starts another Parley SSH window with these arguments (R-SSH-15); the launcher's, as the UI keeps no platform
/// code.
pub type Spawner = Arc<dyn Fn(&[String]) -> Result<(), String> + Send + Sync>;

/// What the command line asked to open (R-SSH-14): a saved session by name, or a host, with what goes with them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StartRequest {
    /// `-load NAME`.
    pub load: Option<String>,
    /// `[user@]host[:port]`.
    pub host: Option<String>,
    /// `-p PORT`.
    pub port: Option<u16>,
    /// `-l USER`.
    pub user: Option<String>,
    /// `-i FILE`, each one.
    pub key_files: Vec<String>,
    /// `-A` (true) or `-a` (false).
    pub agent_forwarding: Option<bool>,
}

/// Parley SSH's whole UI.
pub struct ParleySshUi {
    settings: Settings,
    applied: Option<Settings>,
    settings_dialog: Option<settings::Dialog>,
    logo: Logo,
    updates: updates::Updates,
    known_hosts: KnownHosts,
    form: start::Form,
    session: Option<Session>,
    editor: Option<Editor>,
    /// The event log is open, with its scroll area's last height (R-SSH-16).
    event_log: Option<f32>,
    spawner: Option<Spawner>,
    /// Why the last session closed, or what the command line could not open, for the status bar.
    note: Option<String>,
    shortcuts_open: bool,
    /// The window's close button was pressed while connected: ask (R-SSH-10).
    asking_close: bool,
    close_confirmed: bool,
}

impl std::fmt::Debug for ParleySshUi {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ParleySshUi")
            .field("session", &self.session.as_ref().map(|s| s.target.label()))
            .field("sessions", &self.settings.sessions.len())
            .finish_non_exhaustive()
    }
}

impl ParleySshUi {
    /// The UI with `settings`, keeping host keys in `known_hosts` (R-SSH-3).
    pub fn new(settings: Settings, known_hosts: KnownHosts) -> Self {
        Self {
            form: start::Form::from_settings(&settings),
            settings,
            applied: None,
            settings_dialog: None,
            logo: Logo::default(),
            updates: updates::Updates::default(),
            known_hosts,
            session: None,
            editor: None,
            event_log: None,
            spawner: None,
            note: None,
            shortcuts_open: false,
            asking_close: false,
            close_confirmed: false,
        }
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    pub fn set_logo(&mut self, image: egui::ColorImage) {
        self.logo = Logo::new(image);
    }

    pub fn set_update_checker(&mut self, checker: updates::Checker) {
        self.updates.set_checker(checker);
    }

    pub fn set_spawner(&mut self, spawner: Spawner) {
        self.spawner = Some(spawner);
    }

    /// The title bar: what the host set, or the session, then the program.
    pub fn title(&self) -> String {
        let name = Program::ParleySsh.name();
        let Some(session) = &self.session else {
            return name.to_owned();
        };
        let host_title = session.host_title();
        let shown = if host_title.is_empty() {
            session.target.label()
        } else {
            host_title
        };
        format!("{shown} – {name}")
    }

    /// Opens what the command line asked for (R-SSH-14); what cannot be opened is said in the status bar.
    pub fn start(&mut self, ctx: &egui::Context, request: StartRequest) {
        match self.source_for(request) {
            Ok(Some((source, saved))) => self.open(ctx, source, saved),
            Ok(None) => {}
            Err(e) => self.note = Some(e),
        }
    }

    /// The session a [`StartRequest`] names, and whether it is saved; `None` when it names none.
    fn source_for(&self, request: StartRequest) -> Result<Option<(SavedSession, bool)>, String> {
        let (mut source, saved) = if let Some(name) = &request.load {
            let found = self.settings.sessions.iter().find(|s| &s.name == name);
            let found =
                found.ok_or_else(|| format!("There is no saved session called \"{name}\"."))?;
            (found.clone(), true)
        } else if let Some(host) = &request.host {
            let user = request
                .user
                .clone()
                .unwrap_or_else(|| self.settings.defaults.user.clone());
            let port = request.port.map_or_else(String::new, |p| p.to_string());
            let target = start::parse(host, &port, &user)?;
            (
                SavedSession::from_defaults(&self.settings.defaults, "", &target),
                false,
            )
        } else {
            return Ok(None);
        };
        if request.load.is_some() {
            if let Some(port) = request.port {
                source.port = port;
            }
            if let Some(user) = request.user {
                source.user = user;
            }
        }
        source.key_files.extend(request.key_files);
        if let Some(forwarding) = request.agent_forwarding {
            source.agent_forwarding = forwarding;
        }
        Ok(Some((source, saved)))
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        self.apply_settings(ui.ctx());
        self.guard_close(ui.ctx());
        let menu_frame = theme::bar(ui, Margin::symmetric(8, 3));
        let status_frame = theme::bar(ui, Margin::symmetric(8, 5));
        let logo = self.logo.texture(ui.ctx()).cloned();
        let newer = self
            .updates
            .available()
            .map(|r| (r.version.clone(), r.page.clone()));
        let header = bars::Header {
            logo: logo.as_ref(),
            session: self.session.as_ref(),
            newer: newer.as_ref().map(|(v, p)| (v.as_str(), p.as_str())),
        };
        let action = Panel::top("ssh-menu")
            .frame(menu_frame)
            .show(ui, |ui| bars::menu_bar(ui, &header))
            .inner;
        Panel::bottom("ssh-status")
            .frame(status_frame)
            .show(ui, |ui| {
                bars::status_bar(ui, self.session.as_ref(), self.note.as_deref());
            });
        for shortcut in self.central(ui) {
            let action = match shortcut {
                screen::Shortcut::NewSession => Action::NewSession,
                screen::Shortcut::Duplicate => Action::Duplicate,
                screen::Shortcut::Restart => Action::Restart,
            };
            self.carry_out(ui.ctx(), action);
        }
        if let Some(action) = action {
            self.carry_out(ui.ctx(), action);
            // A menu's click took the keyboard from the screen: give it back.
            if let Some(session) = &mut self.session {
                session.focus_screen();
            }
        }
        self.show_dialogs(ui.ctx());
    }

    fn apply_settings(&mut self, ctx: &egui::Context) {
        if self.applied.is_none() && self.settings.check_for_updates {
            self.updates.start(ctx, updates::Check::AtStart);
        }
        self.updates.collect();
        if self.applied.as_ref() != Some(&self.settings) {
            let old = self.applied.as_ref().map(Settings::look);
            // Parley SSH uses the bundled default terminal font, which always loads: there is no note.
            let _ = settings::apply(ctx, old.as_ref(), &self.settings.look());
            self.applied = Some(self.settings.clone());
        }
    }

    /// The start screen, or the session's screen.
    fn central(&mut self, ui: &mut Ui) -> Vec<screen::Shortcut> {
        let palette = screen::Palette::for_theme(ui.visuals().dark_mode);
        let confirm_paste = self.settings.confirm_paste;
        let zoom = self.settings.text_size;
        let frame = egui::Frame::NONE.fill(palette.background);
        let mut shortcuts = Vec::new();
        let mut choice = None;
        if let Some(session) = &mut self.session {
            session.view.zoom = zoom;
            CentralPanel::default().frame(frame).show(ui, |ui| {
                shortcuts = session.show(ui, &palette, confirm_paste);
            });
        } else {
            let sessions = &self.settings.sessions;
            CentralPanel::default().show(ui, |ui| choice = self.form.show(ui, sessions));
        }
        if let Some(choice) = choice {
            self.chosen(ui.ctx(), choice);
        }
        shortcuts
    }

    /// What was chosen on the start screen.
    fn chosen(&mut self, ctx: &egui::Context, choice: start::Choice) {
        match choice {
            start::Choice::Connect(i) => {
                if let Some(source) = self.settings.sessions.get(i).cloned() {
                    self.open(ctx, source, true);
                }
            }
            start::Choice::Edit(i) => {
                if let Some(session) = self.settings.sessions.get(i).cloned() {
                    self.editor = Some(Editor::new(session, Some(i)));
                }
            }
            start::Choice::New => {
                let blank = Target {
                    host: String::new(),
                    port: self.settings.defaults.port,
                    user: String::new(),
                };
                let new = SavedSession {
                    user: self.settings.defaults.user.clone(),
                    ..SavedSession::from_defaults(&self.settings.defaults, "", &blank)
                };
                self.editor = Some(Editor::new(new, None));
            }
            start::Choice::Defaults => {
                self.editor = Some(Editor::defaults(&self.settings.defaults));
            }
            start::Choice::Quick(target) => {
                self.settings.last_host.clone_from(&target.host);
                self.settings.last_port = target.port;
                self.settings.last_user.clone_from(&target.user);
                let source = SavedSession::from_defaults(&self.settings.defaults, "", &target);
                self.open(ctx, source, false);
            }
        }
    }

    fn open(&mut self, ctx: &egui::Context, source: SavedSession, saved: bool) {
        let ctx = ctx.clone();
        self.session = Some(Session::open(
            source,
            saved,
            self.known_hosts.clone(),
            move || ctx.request_repaint(),
        ));
        self.note = None;
    }

    /// Starts another window with `args` (R-SSH-15).
    fn spawn(&mut self, args: &[String]) {
        let result = match &self.spawner {
            Some(spawner) => spawner(args),
            None => Err("This window cannot start another one.".to_owned()),
        };
        if let Err(e) = result {
            self.note = Some(e);
        }
    }

    fn carry_out(&mut self, ctx: &egui::Context, action: Action) {
        match action {
            Action::NewSession => self.spawn(&[]),
            Action::Duplicate => {
                if let Some(args) = self
                    .session
                    .as_ref()
                    .map(|s| s.source.command_line(s.saved))
                {
                    self.spawn(&args);
                }
            }
            Action::Restart => {
                if let Some(session) = &mut self.session {
                    session.disconnect();
                    let ctx = ctx.clone();
                    session.restart(move || ctx.request_repaint());
                }
            }
            Action::Disconnect => {
                if let Some(session) = &self.session {
                    session.disconnect();
                }
            }
            Action::CloseSession => {
                if let Some(session) = self.session.take() {
                    session.disconnect();
                    self.note = Some(format!(
                        "Closed the session with {}.",
                        session.target.label()
                    ));
                }
                self.event_log = None;
                self.form = start::Form::from_settings(&self.settings);
            }
            Action::EventLog => self.event_log = Some(0.0),
            Action::Exit => ctx.send_viewport_cmd(ViewportCommand::Close),
            Action::Copy => {
                if let Some(text) = self.session.as_ref().and_then(Session::copy_selection) {
                    ctx.copy_text(text);
                }
            }
            Action::Paste => {
                if let Some(session) = &mut self.session {
                    session.view.request_paste(ctx);
                }
            }
            Action::CopyAll => {
                if let Some(session) = &self.session {
                    ctx.copy_text(session.copy_all());
                }
            }
            Action::ClearScrollback => {
                if let Some(session) = &self.session {
                    session.clear_scrollback();
                }
            }
            Action::ResetTerminal => {
                if let Some(session) = &mut self.session {
                    session.reset_terminal();
                }
            }
            Action::TextLarger => {
                self.settings.text_size =
                    (self.settings.text_size + TEXT_SIZES.2).min(TEXT_SIZES.1);
            }
            Action::TextSmaller => {
                self.settings.text_size = self
                    .settings
                    .text_size
                    .saturating_sub(TEXT_SIZES.2)
                    .max(TEXT_SIZES.0);
            }
            Action::TextActualSize => self.settings.text_size = 100,
            Action::Shortcuts => self.shortcuts_open = true,
            Action::Settings => self.settings_dialog = Some(settings::Dialog::open(&self.settings)),
        }
    }

    /// Closing the window while connected asks first, when the settings say so (R-SSH-10).
    fn guard_close(&mut self, ctx: &egui::Context) {
        if !ctx.input(|i| i.viewport().close_requested()) || self.close_confirmed {
            return;
        }
        let connected = self.session.as_ref().is_some_and(Session::is_connected);
        if connected && self.settings.confirm_close {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            self.asking_close = true;
        }
    }

    fn show_dialogs(&mut self, ctx: &egui::Context) {
        if self.shortcuts_open
            && dialogs::shortcut_sheet(ctx, ctx.os() == egui::os::OperatingSystem::Mac)
        {
            self.shortcuts_open = false;
        }
        if self.asking_close
            && let Some(session) = &self.session
            && let Some(close) = dialogs::confirm_close(ctx, &session.target)
        {
            self.asking_close = false;
            if close {
                self.close_confirmed = true;
                session.disconnect();
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }
        }
        self.show_event_log(ctx);
        self.show_editor(ctx);
        self.show_settings(ctx);
    }

    fn show_event_log(&mut self, ctx: &egui::Context) {
        let (Some(height), Some(session)) = (&mut self.event_log, &self.session) else {
            return;
        };
        match dialogs::event_log(ctx, &session.target, &session.log, height) {
            Some(dialogs::LogAction::Copy) => ctx.copy_text(session.log_text()),
            Some(dialogs::LogAction::Close) => self.event_log = None,
            None => {}
        }
    }

    /// The session editor; what it saved goes into the settings at once (R-SSH-11).
    fn show_editor(&mut self, ctx: &egui::Context) {
        let Some(editor) = &mut self.editor else {
            return;
        };
        let taken: Vec<&str> = self
            .settings
            .sessions
            .iter()
            .enumerate()
            .filter(|(i, _)| Some(*i) != editor.index)
            .map(|(_, s)| s.name.as_str())
            .collect();
        let Some(edited) = editor.show(ctx, &taken) else {
            return;
        };
        let sessions = &mut self.settings.sessions;
        match (edited, editor.index) {
            (Edited::Save(session), _) if editor.is_defaults() => {
                self.settings.defaults = SavedSession {
                    name: String::new(),
                    host: String::new(),
                    ..session
                };
            }
            (Edited::Save(session), Some(i)) if i < sessions.len() => sessions[i] = session,
            (Edited::Save(session), _) => sessions.push(session),
            (Edited::Delete, Some(i)) if i < sessions.len() => {
                sessions.remove(i);
            }
            (Edited::Delete | Edited::Cancel, _) => {}
        }
        self.editor = None;
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
            Some(settings::Closed::Keep) => self.settings_dialog = None,
            Some(settings::Closed::Revert) => {
                self.settings = dialog.original().clone();
                self.settings_dialog = None;
            }
            None => {}
        }
    }
}

#[cfg(test)]
mod tests;
