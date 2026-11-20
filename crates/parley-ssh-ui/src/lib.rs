//! Parley SSH's UI (D41): tabs of panes (R-SSH-39, R-SSH-40, D47), each a start screen with the saved sessions
//! (R-SSH-11) or an SSH session, under a header and over a status bar, in the look Parley 5250 and Parley SQL share
//! (D12). The engine is `parley-ssh-core`; the launcher (`parley-ssh` in `parley-app`) keeps the settings, reads
//! the command line (R-SSH-14) and opens other windows (R-SSH-41).

mod bars;
mod broadcast;
mod dialogs;
mod find;
mod fonts;
mod log;
mod openssh;
mod palette;
mod panes;
mod reconnect;
pub mod restore;
pub mod screen;
mod session;
pub mod sessions;
pub mod settings;
pub mod snippets;
mod start;
mod tabs;
mod window;
mod window_keys;

use std::sync::Arc;

use egui::{Margin, Panel, Ui, ViewportCommand};
use parley_ssh_core::known_hosts::KnownHosts;
use parley_ui_egui::menu::{Logo, Program};
use parley_ui_egui::{theme, updates};

use bars::Action;
use panes::PaneId;
use session::Session;
use sessions::{Edited, Editor, SavedSession};
pub use settings::Settings;
use tabs::{Pane, Tab};

/// Starts another Parley SSH window with these arguments (R-SSH-41); the launcher's, as the UI keeps no platform
/// code.
pub type Spawner = Arc<dyn Fn(&[String]) -> Result<(), String> + Send + Sync>;

/// Shows a desktop notification, title and text (R-SSH-46); only the launcher can.
pub type Notifier = Arc<dyn Fn(&str, &str) + Send + Sync>;

/// How long a pane's border flashes for the bell, in seconds (R-SSH-19).
const BELL_FLASH: f64 = 0.2;

/// Sounds a short beep (R-SSH-19, D37); only the launcher can.
pub type Beeper = Arc<dyn Fn() + Send + Sync>;

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
    /// `-J [user@]host[:port],…` (R-SSH-43); `none` for no jump host.
    pub proxy_jump: Option<String>,
    /// What follows the host: run instead of the shell, as `ssh host command` (R-SSH-28).
    pub command: Option<String>,
}

/// What is about to close, waiting for the user's yes (R-SSH-10).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Closing {
    Window,
    Tab(usize),
    Pane(usize, PaneId),
}

/// Parley SSH's whole UI.
pub struct ParleySshUi {
    settings: Settings,
    applied: Option<Settings>,
    /// Said on the Appearance page when the chosen terminal font is not installed (R-SSH-20).
    font_note: Option<String>,
    settings_dialog: Option<settings::Dialog>,
    logo: Logo,
    updates: updates::Updates,
    known_hosts: KnownHosts,
    /// The hosts of `~/.ssh/config` (R-SSH-42).
    openssh: openssh::Hosts,
    /// Always at least one.
    tabs: Vec<Tab>,
    /// The tab in front.
    current: usize,
    next_pane: PaneId,
    /// Where the panes were drawn last frame, for Alt+arrows.
    area: egui::Rect,
    editor: Option<Editor>,
    /// The event log is open, with its scroll area's last height (R-SSH-16).
    event_log: Option<f32>,
    spawner: Option<Spawner>,
    /// What could not be done, or why a session closed, for the status bar.
    note: Option<String>,
    /// The shortcut sheet is open, with its rows' last height.
    shortcuts: Option<f32>,
    closing: Option<Closing>,
    close_confirmed: bool,
    /// Export and import of snippets through the system's file window (R-SSH-49).
    snippet_files: Option<Box<dyn snippets::SnippetFiles>>,
    picker: Option<snippets::Picker>,
    /// A snippet's placeholders being asked for.
    fill: Option<snippets::Fill>,
    /// Broadcast to every tab (R-SSH-50); a tab's own switch is `Tab::broadcast`.
    broadcast_all: bool,
    /// When Escape was last pressed while broadcasting, for Escape twice.
    last_escape: Option<f64>,
    notifier: Option<Notifier>,
    /// The command palette, while open (R-SSH-48).
    palette: Option<palette::Palette>,
    beeper: Option<Beeper>,
}

impl std::fmt::Debug for ParleySshUi {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ParleySshUi")
            .field("tabs", &self.tabs)
            .field("current", &self.current)
            .field("sessions", &self.settings.sessions.len())
            .finish_non_exhaustive()
    }
}

impl ParleySshUi {
    /// The UI with `settings`, keeping host keys in `known_hosts` (R-SSH-3).
    pub fn new(settings: Settings, known_hosts: KnownHosts) -> Self {
        let first = Tab::new(0, Pane::Start(start::Form::from_settings(&settings)));
        Self {
            settings,
            applied: None,
            font_note: None,
            settings_dialog: None,
            logo: Logo::default(),
            updates: updates::Updates::default(),
            known_hosts,
            openssh: openssh::Hosts::default(),
            tabs: vec![first],
            current: 0,
            next_pane: 1,
            area: egui::Rect::NOTHING,
            editor: None,
            event_log: None,
            spawner: None,
            note: None,
            shortcuts: None,
            closing: None,
            close_confirmed: false,
            snippet_files: None,
            picker: None,
            fill: None,
            broadcast_all: false,
            last_escape: None,
            notifier: None,
            palette: None,
            beeper: None,
        }
    }

    /// The bell's sound (R-SSH-19).
    pub fn set_beeper(&mut self, beeper: Beeper) {
        self.beeper = Some(beeper);
    }

    /// Desktop notifications (R-SSH-46).
    pub fn set_notifier(&mut self, notifier: Notifier) {
        self.notifier = Some(notifier);
    }

    /// Every session's shell events, also in tabs not in front; notifications while the window is not focused.
    fn shell_events(&mut self, ctx: &egui::Context) {
        let (now, focused) = ctx.input(|i| (i.time, i.viewport().focused.unwrap_or(true)));
        let allow = self.settings.clipboard_from_server;
        let bell = self.settings.bell;
        let mut notes = Vec::new();
        let mut rang = false;
        for tab in &mut self.tabs {
            for (_, pane) in &mut tab.panes {
                if let Some(session) = pane.session_mut() {
                    notes.extend(session.shell_events(ctx, now, allow));
                    if std::mem::take(&mut session.rang) {
                        rang = true;
                        if bell == settings::BellStyle::Flash {
                            session.flash_until = now + BELL_FLASH;
                        }
                    }
                }
            }
        }
        if rang
            && bell == settings::BellStyle::Sound
            && let Some(beeper) = &self.beeper
        {
            beeper();
        }
        if focused || (notes.is_empty() && !rang) {
            return;
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(
            egui::UserAttentionType::Informational,
        ));
        if let Some(notifier) = &self.notifier {
            for note in notes {
                notifier(&note.title, &note.body);
            }
        }
    }

    /// Export and import of snippets (R-SSH-49).
    pub fn set_snippet_files(&mut self, files: Box<dyn snippets::SnippetFiles>) {
        self.snippet_files = Some(files);
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

    /// OpenSSH's config, for the start screen and the command line (R-SSH-42).
    pub fn set_openssh_config(&mut self, config: parley_ssh_core::openssh_config::Config) {
        self.openssh = openssh::Hosts::new(config);
    }

    /// The title bar: the tab in front's title, then the program.
    pub fn title(&self) -> String {
        let name = Program::ParleySsh.name();
        match self.active_session() {
            Some(_) => format!("{} – {name}", self.tabs[self.current].title()),
            None => name.to_owned(),
        }
    }

    /// The session in the pane in front, if that pane has one.
    fn active_session(&self) -> Option<&Session> {
        self.tabs.get(self.current)?.active_pane()?.session()
    }

    fn active_session_mut(&mut self) -> Option<&mut Session> {
        self.tabs
            .get_mut(self.current)?
            .active_pane_mut()?
            .session_mut()
    }

    /// Opens what the command line asked for (R-SSH-14) in the first tab; what cannot be opened is said in the
    /// status bar. A host that cannot be opened as asked, such as one without a user when the defaults have none
    /// (Parley 5250's Tools ▸ SSH…, R-SSH-59), is put in the start screen's quick connect with what is missing.
    pub fn start(&mut self, ctx: &egui::Context, request: StartRequest) {
        let typed = match (&request.load, &request.host) {
            (None, Some(host)) => Some((host.clone(), request.port)),
            _ => None,
        };
        match self.source_for(request) {
            Ok(Some((source, saved))) => {
                let active = self.tabs[self.current].active;
                self.connect_pane(ctx, active, source, saved);
            }
            Ok(None) => self.restore_last_tabs(),
            Err(e) => match (typed, self.tabs[self.current].active_pane_mut()) {
                (Some((host, port)), Some(Pane::Start(form))) => {
                    form.host = host;
                    if let Some(port) = port {
                        form.port = port.to_string();
                    }
                    form.error = Some(e);
                }
                _ => self.note = Some(e),
            },
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
            (self.host_source(host, &request)?, false)
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
        if let Some(command) = request.command {
            source.remote_command = command;
        }
        if let Some(jumps) = request.proxy_jump {
            source.proxy_jump = if jumps.trim().eq_ignore_ascii_case("none") {
                String::new()
            } else {
                jumps
            };
        }
        Ok(Some((source, saved)))
    }

    /// The session for `[user@]host[:port]` from the command line: the defaults, then what `~/.ssh/config` says
    /// for the host as `ssh host` reads it, then what was typed (R-SSH-14, R-SSH-42).
    fn host_source(&self, host: &str, request: &StartRequest) -> Result<SavedSession, String> {
        let (typed_user, name, typed_port) = start::split(host)?;
        let mut source = SavedSession {
            name: String::new(),
            host: name.clone(),
            ..self.settings.defaults.clone()
        };
        if let Some(found) = self.openssh.lookup(&name) {
            openssh::apply(&mut source, &found);
        }
        if let Some(user) = typed_user.or_else(|| request.user.clone()) {
            source.user = user;
        }
        // With no user anywhere, quick connect asks for one (R-SSH-59).
        let port = typed_port
            .or_else(|| request.port.map(|p| p.to_string()))
            .unwrap_or_else(|| source.port.to_string());
        let target = start::parse(&source.host, &port, &source.user)?;
        source.host = target.host;
        source.port = target.port;
        source.user = target.user;
        Ok(source)
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        self.apply_settings(ui.ctx());
        self.shell_events(ui.ctx());
        self.guard_close(ui.ctx());
        if !self.dialog_open() {
            let mac = ui.ctx().os() == egui::os::OperatingSystem::Mac;
            let split = self.tabs[self.current].is_split();
            let keys = ui
                .ctx()
                .input_mut(|i| window_keys::take(&mut i.events, mac, split));
            for key in keys {
                self.window_key(ui.ctx(), key);
            }
            if let Some(i) = self.snippet_key(ui.ctx()) {
                self.send_snippet(i);
            }
            self.watch_double_escape(ui);
        }
        let menu_frame = theme::bar(ui, Margin::symmetric(8, 3));
        let status_frame = theme::bar(ui, Margin::symmetric(8, 5));
        let logo = self.logo.texture(ui.ctx()).cloned();
        let newer = self
            .updates
            .available()
            .map(|r| (r.version.clone(), r.page.clone()));
        let header = bars::Header {
            logo: logo.as_ref(),
            session: self.active_session(),
            newer: newer.as_ref().map(|(v, p)| (v.as_str(), p.as_str())),
            snippets: &self.settings.snippets,
            broadcast: self.broadcast_scope(),
        };
        let action = Panel::top("ssh-menu")
            .frame(menu_frame)
            .show(ui, |ui| bars::menu_bar(ui, &header))
            .inner;
        // R-SSH-61: the session's colour under the header with one tab; with more, their stripes say it.
        let color = self
            .active_session()
            .filter(|_| self.tabs.len() < 2)
            .and_then(|s| theme::session_color(ui, s.source.color));
        parley_ui_egui::session_color::header_line(ui, "ssh-session-line", color);
        let status_action = Panel::bottom("ssh-status")
            .frame(status_frame)
            .show(ui, |ui| {
                let broadcast = match self.receivers() {
                    0 => None,
                    n => Some(format!("Broadcasting to {n} panes (Escape twice to stop)")),
                };
                let note = broadcast.as_deref().or(self.note.as_deref());
                bars::status_bar(ui, self.active_session(), note)
            })
            .inner;
        let action = action.or(status_action);
        self.drop_failed_panes();
        self.tab_bar(ui);
        self.central(ui);
        if let Some(action) = action {
            self.carry_out(ui.ctx(), action);
            // A menu's click took the keyboard from the screen: give it back.
            if let Some(session) = self.active_session_mut() {
                session.focus_screen();
            }
        }
        self.show_dialogs(ui.ctx());
        self.snippet_dialogs(ui.ctx());
        self.show_palette(ui.ctx());
    }

    /// The command palette, when open (R-SSH-48).
    fn show_palette(&mut self, ctx: &egui::Context) {
        if self.palette.is_none() {
            return;
        }
        let mac = ctx.os() == egui::os::OperatingSystem::Mac;
        let items = self.palette_items(mac);
        let recent = self.settings.palette_recent.clone();
        if let Some(palette) = &mut self.palette
            && let Some(closed) = palette.show(ctx, &items, &recent)
        {
            self.palette = None;
            self.palette_chosen(ctx, closed);
        }
    }

    /// A dialog of the window's own is open: the window's shortcuts wait.
    fn dialog_open(&self) -> bool {
        self.editor.is_some()
            || self.settings_dialog.is_some()
            || self.closing.is_some()
            || self.shortcuts.is_some()
            || self.picker.is_some()
            || self.fill.is_some()
            || self.palette.is_some()
    }

    fn apply_settings(&mut self, ctx: &egui::Context) {
        if self.applied.is_none() {
            // Ctrl+= and Ctrl+- size the screen's text instead (R-SSH-20).
            ctx.options_mut(|o| o.zoom_with_keyboard = false);
        }
        if self.applied.is_none() && self.settings.check_for_updates {
            self.updates.start(ctx, updates::Check::AtStart);
        }
        self.updates.collect();
        if self.applied.as_ref() != Some(&self.settings) {
            let old = self.applied.as_ref().map(Settings::look);
            // The fonts are set up below with the screen's faces, which replaces what this sets and its note.
            let _ = settings::apply(ctx, old.as_ref(), &self.settings.look());
            let font = &self.settings.terminal_font;
            if old.as_ref().is_none_or(|o| &o.terminal_font != font) {
                self.font_note = fonts::install(ctx, font);
            }
            if let Some(dialog) = &mut self.settings_dialog {
                dialog.font_note.clone_from(&self.font_note);
            }
            self.applied = Some(self.settings.clone());
        }
    }

    /// What was chosen on the start screen of pane `pane`.
    fn chosen(&mut self, ctx: &egui::Context, pane: PaneId, choice: start::Choice) {
        match choice {
            start::Choice::Connect(i) => {
                if let Some(source) = self.settings.sessions.get(i).cloned() {
                    self.connect_pane(ctx, pane, source, true);
                }
            }
            start::Choice::Edit(i) => {
                if let Some(session) = self.settings.sessions.get(i).cloned() {
                    self.editor = Some(Editor::new(session, Some(i)));
                }
            }
            start::Choice::New => {
                let blank = parley_ssh_core::net::Target {
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
                self.connect_pane(ctx, pane, source, false);
            }
            start::Choice::ConfigConnect(i) => {
                if let Some(host) = self.openssh.hosts().get(i) {
                    let source = openssh::session(&self.settings.defaults, host);
                    self.connect_pane(ctx, pane, source, false);
                }
            }
            start::Choice::Favourite(i) => {
                if let Some(session) = self.settings.sessions.get_mut(i) {
                    session.favourite = !session.favourite;
                }
            }
            start::Choice::Recent(r) => {
                if let Some((source, saved)) =
                    palette::recent_source(&self.settings.sessions, &self.settings.recent, r)
                {
                    self.connect_pane(ctx, pane, source, saved);
                }
            }
            start::Choice::RecentSave(r) => {
                if let Some((session, false)) =
                    palette::recent_source(&self.settings.sessions, &self.settings.recent, r)
                {
                    self.editor = Some(Editor::new(session, None));
                }
            }
            start::Choice::AllSessionsOpen(open) => self.settings.all_sessions_open = open,
            start::Choice::ConfigSave(i) => {
                if let Some(host) = self.openssh.hosts().get(i) {
                    let session = openssh::session(&self.settings.defaults, host);
                    self.editor = Some(Editor::new(session, None));
                }
            }
        }
    }

    /// Starts another window with `args` (R-SSH-41).
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
            Action::NewTab => self.new_tab(ctx, None),
            Action::NewWindow => self.spawn(&[]),
            Action::Duplicate => self.duplicate(ctx),
            Action::SplitRight => self.split(ctx, panes::Axis::Row),
            Action::SplitDown => self.split(ctx, panes::Axis::Column),
            Action::Close => self.close_active(),
            Action::Restart => {
                if let Some(session) = self.active_session_mut() {
                    session.disconnect();
                    let ctx = ctx.clone();
                    session.restart(move || ctx.request_repaint());
                }
            }
            Action::Disconnect => {
                if let Some(session) = self.active_session() {
                    session.disconnect();
                }
            }
            Action::EventLog => self.event_log = Some(0.0),
            Action::Exit => ctx.send_viewport_cmd(ViewportCommand::Close),
            Action::Copy => {
                if let Some(text) = self.active_session().and_then(Session::copy_selection) {
                    ctx.copy_text(text);
                }
            }
            Action::Paste => {
                if let Some(session) = self.active_session_mut() {
                    session.view.request_paste(ctx);
                }
            }
            Action::CopyAll => {
                if let Some(session) = self.active_session() {
                    ctx.copy_text(session.copy_all());
                }
            }
            Action::ClearScrollback => {
                if let Some(session) = self.active_session() {
                    session.clear_scrollback();
                }
            }
            Action::ResetTerminal => {
                if let Some(session) = self.active_session_mut() {
                    session.reset_terminal();
                }
            }
            // The screen's text, as Ctrl+= / Ctrl+- / Ctrl+0 (R-SSH-20); Settings ▸ Appearance sizes everything.
            Action::TextLarger => self.settings.step_font_size(1),
            Action::TextSmaller => self.settings.step_font_size(-1),
            Action::TextActualSize => self.settings.step_font_size(0),
            Action::Shortcuts => self.shortcuts = Some(0.0),
            Action::ToggleLog => {
                if let Some(session) = self.active_session_mut() {
                    if session.logging() {
                        session.stop_log();
                    } else {
                        session.start_log();
                    }
                }
            }
            Action::Find => {
                if let Some(session) = self.active_session_mut() {
                    session.open_find();
                }
            }
            Action::SendSnippet(i) => self.send_snippet(i),
            Action::PickSnippet => self.open_picker(),
            Action::EditSnippets => {
                self.settings_dialog =
                    Some(settings::Dialog::open(ctx, &self.settings).on(settings::Page::Snippets));
            }
            Action::BroadcastTab => self.toggle_broadcast_tab(),
            Action::BroadcastAll => self.toggle_broadcast_all(),
            Action::StopReconnect => {
                if let Some(session) = self.active_session_mut() {
                    session.stop_reconnecting();
                }
            }
            Action::Settings => {
                self.settings_dialog = Some(settings::Dialog::open(ctx, &self.settings));
            }
        }
    }

    fn show_dialogs(&mut self, ctx: &egui::Context) {
        let mac = ctx.os() == egui::os::OperatingSystem::Mac;
        if let Some(height) = &mut self.shortcuts
            && dialogs::shortcut_sheet(ctx, mac, height)
        {
            self.shortcuts = None;
        }
        self.show_closing(ctx);
        self.show_event_log(ctx);
        self.show_editor(ctx);
        self.show_settings(ctx);
    }

    fn show_event_log(&mut self, ctx: &egui::Context) {
        let Some(mut height) = self.event_log else {
            return;
        };
        let Some(session) = self.active_session() else {
            self.event_log = None;
            return;
        };
        let action = dialogs::event_log(ctx, &session.target, &session.log, &mut height);
        match action {
            Some(dialogs::LogAction::Copy) => ctx.copy_text(session.log_text()),
            Some(dialogs::LogAction::Close) => {
                self.event_log = None;
                return;
            }
            None => {}
        }
        self.event_log = Some(height);
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
                    ..*session
                };
            }
            (Edited::Save(session), Some(i)) if i < sessions.len() => sessions[i] = *session,
            (Edited::Save(session), _) => sessions.push(*session),
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
        let closed = dialog.show(
            ctx,
            &mut self.settings,
            self.updates.status(),
            self.snippet_files.as_deref(),
        );
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
