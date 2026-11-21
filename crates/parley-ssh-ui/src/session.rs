//! One SSH session in the window (D45): its connection, its terminal, where it stands, its event log (R-SSH-16),
//! and the questions it asks while connecting (R-SSH-3, R-SSH-4, R-SSH-12) and pasting (R-SSH-9).

use std::fmt::Write as _;
use std::sync::{Arc, Mutex};

use parley_ssh_core::keys;
use parley_ssh_core::known_hosts::KnownHosts;
use parley_ssh_core::net::{self, Algorithms, Command, Connection, Ending, Event, Target};
use parley_ssh_core::term::{ShellEvent, Terminal};

use crate::dialogs;
use crate::reconnect::{self, Reconnect};
use crate::screen::{self, Out};
use crate::sessions::SavedSession;

/// One line of the event log: when, and what (R-SSH-16).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogLine {
    pub at: String,
    pub text: String,
}

impl LogLine {
    fn now(text: String) -> Self {
        Self {
            at: chrono::Local::now().format("%H:%M:%S%.3f").to_string(),
            text,
        }
    }
}

/// Where the session stands (R-SSH-10's status bar).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State {
    Connecting,
    /// Key exchange is done; signing on.
    SigningOn,
    Running,
    Ended(Ending),
}

/// The open session.
#[derive(Debug)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "independent facts about the session, not one state"
)]
pub struct Session {
    pub target: Target,
    /// What the session was opened from: a saved session, or the defaults with the quick connect's target.
    pub source: SavedSession,
    /// Whether `source` is a saved session, to open again by its name (R-SSH-15).
    pub saved: bool,
    /// The last problem that did not end the connection (a key file that cannot be read), for the status bar.
    pub problem: Option<String>,
    connection: Connection,
    terminal: Arc<Mutex<Terminal>>,
    pub state: State,
    /// The shell started once: signing on worked (R-SSH-58 keeps a split pane only then).
    started: bool,
    pub algorithms: Option<Algorithms>,
    pub fingerprint: Option<String>,
    /// The host key trusted, as an OpenSSH line, for transfers to this host (R-SSH-53).
    pub host_key_line: Option<String>,
    /// What the jobs on this connection said since the Transfer panel last looked (Phase 31).
    jobs: Vec<Event>,
    /// What happened, for the event log (R-SSH-16).
    pub log: Vec<LogLine>,
    pub view: screen::View,
    host_key: Option<dialogs::HostKey>,
    prompt: Option<dialogs::Prompt>,
    /// The server's banner, shown with the next prompt (R-SSH-4).
    banner: Option<String>,
    paste: Option<String>,
    /// Take the keyboard on the next frame (after a dialog closed).
    grab_focus: bool,
    known_hosts: KnownHosts,
    /// Keys and pastes sent from this pane, for broadcast (R-SSH-50).
    shared: Vec<Shared>,
    /// Reconnecting after a lost connection, between tries (R-SSH-51).
    reconnect: Option<Reconnect>,
    /// How long to try reconnecting, in minutes; 0 for never (the settings', set each frame).
    pub reconnect_minutes: u8,
    /// The connection ended since the last frame: whether to reconnect is decided in `show`.
    just_ended: bool,
    /// Find in the scrollback, while its bar is open (R-SSH-24).
    pub find: Option<crate::find::Find>,
    /// When the command running now started (OSC 133;C), for R-SSH-46's long commands.
    command_since: Option<f64>,
    /// The session log being written (R-SSH-25).
    log_file: Option<crate::log::SessionLog>,
    /// The host rang the bell since the window last looked (R-SSH-19).
    pub rang: bool,
    /// Until when the pane's border flashes for the bell.
    pub flash_until: f64,
}

/// A command that ran at least this long, in seconds, is said when it ends (R-SSH-46).
const LONG_COMMAND: f64 = 10.0;

/// A desktop notification to show: its title and text (R-SSH-46).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    pub title: String,
    pub body: String,
}

/// What one pane sends that others may get too (R-SSH-50).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Shared {
    Keys(Vec<u8>),
    /// Text to paste, encoded by each pane for its own modes.
    Paste(String),
}

impl Session {
    /// Starts connecting to `source` (`saved` when it is a saved session); `repaint` wakes the UI when there is
    /// news.
    pub fn open(
        source: SavedSession,
        saved: bool,
        known_hosts: KnownHosts,
        repaint: impl Fn() + Send + Sync + 'static,
    ) -> Self {
        Self::open_on(
            source,
            saved,
            known_hosts,
            Arc::new(Mutex::new(Terminal::new(80, 24))),
            repaint,
        )
    }

    fn open_on(
        source: SavedSession,
        saved: bool,
        known_hosts: KnownHosts,
        terminal: Arc<Mutex<Terminal>>,
        repaint: impl Fn() + Send + Sync + 'static,
    ) -> Self {
        let target = source.target();
        let connection = match source.options() {
            Ok(options) => Connection::open(
                target.clone(),
                options,
                known_hosts.clone(),
                Arc::clone(&terminal),
                repaint,
            ),
            // Said as any connection that fails, so a split pane is taken out again (R-SSH-58).
            Err(problem) => Connection::failed(problem, Arc::clone(&terminal)),
        };
        Self {
            target,
            source,
            saved,
            problem: None,
            connection,
            terminal,
            state: State::Connecting,
            started: false,
            algorithms: None,
            fingerprint: None,
            host_key_line: None,
            jobs: Vec::new(),
            log: Vec::new(),
            view: screen::View::default(),
            host_key: None,
            prompt: None,
            banner: None,
            paste: None,
            grab_focus: true,
            known_hosts,
            shared: Vec::new(),
            reconnect: None,
            reconnect_minutes: 0,
            just_ended: false,
            find: None,
            command_since: None,
            log_file: None,
            rang: false,
            flash_until: 0.0,
        }
    }

    /// Connects again with the same settings, on the same screen, as `PuTTY`'s Restart Session (R-SSH-5); also
    /// Restart now while reconnecting.
    pub fn restart(&mut self, repaint: impl Fn() + Send + Sync + 'static) {
        self.reconnect = None;
        self.reopen(repaint);
    }

    /// A new connection on the same screen, keeping the event log and any reconnecting.
    fn reopen(&mut self, repaint: impl Fn() + Send + Sync + 'static) {
        let terminal = Arc::clone(&self.terminal);
        net::lock(&terminal).feed(b"\r\n");
        let zoom = self.view.zoom;
        let log = std::mem::take(&mut self.log);
        let log_file = self.log_file.take();
        let reconnect = self.reconnect.take();
        let minutes = self.reconnect_minutes;
        *self = Self::open_on(
            self.source.clone(),
            self.saved,
            self.known_hosts.clone(),
            terminal,
            repaint,
        );
        self.view.zoom = zoom;
        self.log = log;
        self.reconnect = reconnect;
        if let Some(file) = log_file {
            net::lock(&self.terminal).record(true);
            self.log_file = Some(file);
        }
        self.reconnect_minutes = minutes;
    }

    /// Stops reconnecting (the status bar's Stop).
    pub fn stop_reconnecting(&mut self) {
        if self.reconnect.take().is_some() {
            self.say("Stopped reconnecting.");
        }
    }

    /// Reconnecting: the next try's number and the seconds until it.
    pub fn reconnecting(&self, now: f64) -> Option<(u32, u64)> {
        self.reconnect
            .as_ref()
            .map(|r| (r.attempt + 1, r.seconds_left(now)))
    }

    /// A line on the screen, faint, and in the event log.
    fn say(&mut self, text: &str) {
        net::lock(&self.terminal).feed(format!("\r\n\x1b[2m[{text}]\x1b[0m\r\n").as_bytes());
        self.log.push(LogLine::now(text.to_owned()));
    }

    /// After the connection ended: starts, goes on with or gives up reconnecting (R-SSH-51); and starts a try
    /// when it is due.
    fn follow_up(&mut self, ctx: &egui::Context, now: f64) {
        if std::mem::take(&mut self.just_ended)
            && let State::Ended(ending) = self.state.clone()
        {
            match &mut self.reconnect {
                Some(r) if matches!(ending, Ending::Failed(_)) => {
                    let (attempt, more) = (r.attempt + 1, r.failed(now));
                    if more {
                        let wait = r.seconds_left(now);
                        self.say(&format!("Try {attempt} failed. Trying again in {wait} s."));
                    } else {
                        self.reconnect = None;
                        let minutes = self.reconnect_minutes;
                        self.say(&format!("Gave up reconnecting after {minutes} minutes."));
                    }
                }
                Some(_) => self.stop_reconnecting(),
                None if self.reconnect_minutes > 0 && reconnect::is_lost(&ending, self.started) => {
                    self.reconnect = Some(Reconnect::start(now, self.reconnect_minutes));
                    let why = match &ending {
                        Ending::Failed(why) => why.as_str(),
                        _ => "",
                    };
                    self.say(&format!("The connection was lost ({why}). Reconnecting…"));
                }
                None => {}
            }
        }
        if let Some(r) = &self.reconnect
            && matches!(self.state, State::Ended(_))
        {
            if r.due(now) {
                let ctx = ctx.clone();
                self.reopen(move || ctx.request_repaint());
            } else {
                ctx.request_repaint_after(std::time::Duration::from_secs_f64(
                    (r.next - now).max(0.05),
                ));
            }
        }
    }

    pub fn is_connected(&self) -> bool {
        !matches!(self.state, State::Ended(_))
    }

    /// Its shell has started: signing on worked.
    pub fn is_started(&self) -> bool {
        self.started
    }

    /// It ended before its shell started: it could not connect, or signing on failed or was cancelled.
    pub fn failed_to_start(&self) -> bool {
        !self.started && matches!(self.state, State::Ended(_))
    }

    /// The terminal's size, columns × rows.
    pub fn size(&self) -> (usize, usize) {
        let t = net::lock(&self.terminal);
        (t.columns(), t.rows())
    }

    /// The window title the host set, if any.
    pub fn host_title(&self) -> String {
        net::lock(&self.terminal).title().to_owned()
    }

    /// Gives the screen the keyboard on the next frame, after a menu took it.
    /// While the find bar is open its field gets the keyboard instead (R-SSH-24).
    pub fn focus_screen(&mut self) {
        match &mut self.find {
            Some(find) => find.bar.focus(),
            None => self.grab_focus = true,
        }
    }

    /// The event log as text, one line each, for Copy (R-SSH-16).
    pub fn log_text(&self) -> String {
        self.log.iter().fold(String::new(), |mut text, l| {
            // Writing to a String cannot fail.
            let _ = writeln!(text, "{}  {}", l.at, l.text);
            text
        })
    }

    pub fn disconnect(&self) {
        self.connection.send(Command::Disconnect);
    }

    /// Runs a command on a side channel (Phase 31); what it says comes from [`Self::take_jobs`].
    /// Sends a transfer's command: a job, or a forwarding (Phase 31).
    pub fn send_job(&self, command: Command) {
        self.connection.send(command);
    }

    /// What the jobs said since the last call.
    pub fn take_jobs(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.jobs)
    }

    /// Whether the connection goes through a jump host: another server cannot reach it directly, so a copy to it
    /// goes through Parley SSH (R-SSH-54).
    pub fn through_jump(&self) -> bool {
        !self.source.proxy_jump.trim().is_empty()
    }

    /// Takes in what the connection said since the last frame.
    pub fn poll(&mut self) {
        let events: Vec<Event> = self.connection.events().collect();
        for event in events {
            match event {
                Event::Log(line) => self.log.push(LogLine::now(line)),
                Event::Problem(problem) => {
                    self.log.push(LogLine::now(problem.clone()));
                    self.problem = Some(problem);
                }
                Event::HostKey(question) => self.host_key = Some(dialogs::HostKey::new(question)),
                Event::Banner(text) => self.banner = Some(text),
                Event::Prompt(prompt) => {
                    self.prompt = Some(dialogs::Prompt::new(prompt, self.banner.take()));
                }
                // Kept for the tab, Restart and reconnecting, which then do not ask again (R-SSH-62).
                Event::User(user) => {
                    self.target.user.clone_from(&user);
                    self.source.user = user;
                }
                Event::Secured {
                    algorithms,
                    fingerprint,
                    host_key,
                } => {
                    self.algorithms = Some(algorithms);
                    self.fingerprint = Some(fingerprint);
                    self.host_key_line = Some(host_key);
                    if self.state == State::Connecting {
                        self.state = State::SigningOn;
                    }
                }
                // How the user signed on is for the key sign-on offer (R-SSH-67, Phase 32 step 2).
                Event::Started { with: _ } => {
                    if self.log_file.is_none() && self.source.log_mode != crate::log::LogMode::Off {
                        self.start_log();
                    }
                    if self.reconnect.take().is_some() {
                        self.say("Reconnected: this is a new shell.");
                    }
                    self.state = State::Running;
                    self.started = true;
                    self.problem = None;
                    self.grab_focus = true;
                }
                // For the Transfer panel, which started them (Phase 31).
                job @ (Event::JobOutput { .. }
                | Event::JobEnded { .. }
                | Event::Door { .. }
                | Event::Forwarded { .. }) => self.jobs.push(job),
                // Only the key setup asks for them (R-SSH-67, Phase 32 step 2).
                Event::AgentKeys(_) => {}
                Event::Ended(ending) => {
                    self.log.push(LogLine::now(ending_text(&ending)));
                    self.host_key = None;
                    self.prompt = None;
                    self.state = State::Ended(ending);
                    self.just_ended = true;
                }
            }
        }
    }

    /// Draws the screen; while `active` (the pane in front) also its questions, else a note that one waits.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        look: &screen::Look,
        confirm_paste: bool,
        active: bool,
    ) {
        self.poll();
        let now = ui.input(|i| i.time);
        self.follow_up(ui.ctx(), now);
        self.view.mouse_reporting = self.source.mouse_reporting;
        let asking = self.host_key.is_some() || self.prompt.is_some() || self.paste.is_some();
        let grab = active && !asking && std::mem::take(&mut self.grab_focus);
        let area = ui.max_rect();
        self.find_bar(ui);
        for out in screen::show(ui, &self.terminal, &mut self.view, look, grab) {
            match out {
                Out::Send(bytes) => self.send(bytes),
                Out::Typed(bytes) => {
                    self.shared.push(Shared::Keys(bytes.clone()));
                    self.send(bytes);
                }
                Out::Resized { columns, rows } => {
                    self.connection.send(Command::Resize { columns, rows });
                }
                Out::Paste(text) => self.paste(&text, confirm_paste),
                Out::OpenLink(url) => {
                    self.log
                        .push(LogLine::now(format!("Opened the link {url}")));
                    ui.ctx().open_url(egui::OpenUrl::new_tab(url));
                }
            }
        }
        if active {
            self.show_dialogs(ui.ctx());
        } else if asking {
            // One question at a time, in the pane in front: this one waits until it is clicked.
            let note = "Waiting for your answer: click here";
            ui.painter().text(
                area.center(),
                egui::Align2::CENTER_CENTER,
                note,
                egui::FontId::proportional(14.0),
                theme_text(ui),
            );
        }
    }

    /// Pastes `text` as the Edit menu does (R-SSH-9), asking first when needed.
    pub fn paste(&mut self, text: &str, confirm: bool) {
        let modes = *net::lock(&self.terminal).modes();
        if confirm && keys::paste_needs_confirmation(text, &modes) {
            self.paste = Some(text.to_owned());
        } else {
            self.shared.push(Shared::Paste(text.to_owned()));
            self.send(keys::paste(text, &modes));
        }
    }

    /// What the shell asked for since the last call (Phase 35): a command's start and end, a notification, the
    /// clipboard. The clipboard is set only when `allow_clipboard` (R-SSH-47); the notes are for the window.
    pub fn shell_events(
        &mut self,
        ctx: &egui::Context,
        now: f64,
        allow_clipboard: bool,
    ) -> Vec<Note> {
        let (events, bells, recorded) = {
            let mut t = net::lock(&self.terminal);
            (t.take_shell_events(), t.take_bells(), t.take_recorded())
        };
        self.rang |= bells > 0;
        if let Some(file) = &mut self.log_file
            && let Err(e) = file.write(&recorded)
        {
            self.log.push(LogLine::now(e.clone()));
            self.problem = Some(e);
            self.stop_log();
        }
        let mut notes = Vec::new();
        for event in events {
            match event {
                ShellEvent::CommandStarted => self.command_since = Some(now),
                ShellEvent::CommandEnded(exit) => {
                    if let Some(since) = self.command_since.take()
                        && now - since >= LONG_COMMAND
                    {
                        let how = match exit {
                            Some(0) => "finished".to_owned(),
                            Some(status) => format!("failed with status {status}"),
                            None => "ended".to_owned(),
                        };
                        notes.push(Note {
                            title: self.target.label(),
                            body: format!("A command {how} after {} s.", (now - since).round()),
                        });
                    }
                }
                ShellEvent::Notify { title, body } => notes.push(Note {
                    title: if title.is_empty() {
                        self.target.label()
                    } else {
                        title
                    },
                    body,
                }),
                ShellEvent::Clipboard(text) => {
                    let count = text.chars().count();
                    if allow_clipboard {
                        ctx.copy_text(text);
                        self.log.push(LogLine::now(format!(
                            "The server set the clipboard ({count} characters)"
                        )));
                    } else {
                        self.log.push(LogLine::now(format!(
                            "The server asked to set the clipboard ({count} characters); the setting refuses it"
                        )));
                    }
                }
            }
        }
        notes
    }

    /// Whether a session log is being written (R-SSH-25).
    pub fn logging(&self) -> bool {
        self.log_file.is_some()
    }

    /// Starts the session log: the session's own kind, or text when it has none.
    pub fn start_log(&mut self) {
        let mode = match self.source.log_mode {
            crate::log::LogMode::Off => crate::log::LogMode::Text,
            mode => mode,
        };
        let pattern = Some(self.source.log_pattern.trim())
            .filter(|p| !p.is_empty())
            .unwrap_or(crate::log::DEFAULT_PATTERN)
            .to_owned();
        match crate::log::SessionLog::open(&pattern, mode, &self.target.host, &self.target.label())
        {
            Ok(file) => {
                self.log
                    .push(LogLine::now(format!("Logging to {}", file.path.display())));
                net::lock(&self.terminal).record(true);
                self.log_file = Some(file);
            }
            Err(e) => {
                self.log.push(LogLine::now(e.clone()));
                self.problem = Some(e);
            }
        }
    }

    pub fn stop_log(&mut self) {
        net::lock(&self.terminal).record(false);
        if let Some(file) = self.log_file.take() {
            self.log.push(LogLine::now(format!(
                "Stopped logging to {}",
                file.path.display()
            )));
        }
    }

    /// Scrolls to the command before or after the one at the view's top (R-SSH-45).
    pub fn jump_command(&self, back: bool) {
        screen::scrollbar::jump(&mut net::lock(&self.terminal), back);
    }

    /// Opens find in the scrollback, or gives its field the keyboard again (Ctrl+Shift+F, R-SSH-24).
    pub fn open_find(&mut self) {
        // The field takes the keyboard, not the screen.
        self.grab_focus = false;
        match &mut self.find {
            Some(find) => find.bar.focus(),
            None => self.find = Some(crate::find::Find::opened()),
        }
    }

    /// The find bar above the screen, and what the screen highlights.
    fn find_bar(&mut self, ui: &mut egui::Ui) {
        let Some(find) = &mut self.find else {
            self.view.highlights.clear();
            self.view.current_match = None;
            return;
        };
        let closed = egui::Frame::new()
            .inner_margin(egui::Margin::symmetric(8, 4))
            .fill(parley_ui_egui::theme::palette(ui).chrome)
            .show(ui, |ui| find.bar(ui))
            .inner
            .is_some();
        let (shown, current) = find.update(&mut net::lock(&self.terminal));
        self.view.highlights = shown;
        self.view.current_match = current;
        if closed {
            self.find = None;
            self.view.highlights.clear();
            self.view.current_match = None;
            self.grab_focus = true;
        }
    }

    /// Keys and pastes sent from this pane since the last call, for broadcast (R-SSH-50).
    pub fn take_shared(&mut self) -> Vec<Shared> {
        std::mem::take(&mut self.shared)
    }

    /// What another pane sent while broadcasting, or a snippet (R-SSH-49, R-SSH-50): a paste is encoded for this
    /// pane's own modes, and never asks.
    pub fn receive(&self, shared: &Shared) {
        match shared {
            Shared::Keys(bytes) => self.send(bytes.clone()),
            Shared::Paste(text) => {
                let modes = *net::lock(&self.terminal).modes();
                self.send(keys::paste(text, &modes));
            }
        }
    }

    pub fn copy_selection(&self) -> Option<String> {
        self.view.selected_text(&net::lock(&self.terminal))
    }

    pub fn copy_all(&self) -> String {
        net::lock(&self.terminal).all_text()
    }

    pub fn clear_scrollback(&self) {
        net::lock(&self.terminal).clear_history();
    }

    pub fn reset_terminal(&mut self) {
        net::lock(&self.terminal).reset();
        self.view.clear_selection();
    }

    fn send(&self, bytes: Vec<u8>) {
        if !bytes.is_empty() && self.state == State::Running {
            self.connection.send(Command::Input(bytes));
        }
    }

    fn show_dialogs(&mut self, ctx: &egui::Context) {
        if let Some(dialog) = &mut self.host_key
            && let Some(answer) = dialog.show(ctx, &self.known_hosts)
        {
            self.connection.send(Command::HostKey(answer));
            self.host_key = None;
            self.grab_focus = true;
        }
        if let Some(dialog) = &mut self.prompt
            && let Some(answers) = dialog.show(ctx)
        {
            let answers = match answers {
                dialogs::Answered::Cancel => None,
                dialogs::Answered::Answers(answers) => Some(answers),
            };
            self.connection.send(Command::Answers(answers));
            self.prompt = None;
            self.grab_focus = true;
        }
        if let Some(text) = &self.paste
            && let Some(go) = dialogs::confirm_paste(ctx, text)
        {
            if go {
                let modes = *net::lock(&self.terminal).modes();
                self.shared.push(Shared::Paste(text.clone()));
                self.send(keys::paste(text, &modes));
            }
            self.paste = None;
            self.grab_focus = true;
        }
    }
}

fn theme_text(ui: &egui::Ui) -> egui::Color32 {
    parley_ui_egui::theme::palette(ui).text
}

/// How a session ended, in words for the status bar and the log (R-SSH-5).
pub fn ending_text(ending: &Ending) -> String {
    match ending {
        Ending::Exited(0) => "The shell ended.".to_owned(),
        Ending::Exited(status) => format!("The shell ended with exit status {status}."),
        Ending::Signal(signal) => format!("The shell was ended by signal {signal}."),
        Ending::Closed => "The server closed the connection.".to_owned(),
        Ending::Disconnected => "Disconnected.".to_owned(),
        Ending::Failed(why) | Ending::Refused(why) => format!("Could not connect: {why}."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endings_are_said_in_plain_words_r_ssh_5() {
        assert_eq!(ending_text(&Ending::Exited(0)), "The shell ended.");
        assert_eq!(
            ending_text(&Ending::Exited(127)),
            "The shell ended with exit status 127."
        );
        assert_eq!(
            ending_text(&Ending::Signal("KILL".into())),
            "The shell was ended by signal KILL."
        );
        assert_eq!(
            ending_text(&Ending::Failed(
                "could not reach x: connection refused".into()
            )),
            "Could not connect: could not reach x: connection refused."
        );
    }

    /// A session whose jump hosts cannot be read: each connection fails at once, as a lost network would.
    fn failing() -> Session {
        let source = SavedSession {
            host: "box".into(),
            user: "me".into(),
            proxy_jump: "@".into(),
            ..SavedSession::default()
        };
        let known = KnownHosts::new(
            std::env::temp_dir()
                .join("parley-ssh-reconnect")
                .join("known_hosts"),
            None,
        );
        Session::open(source, false, known, || {})
    }

    fn screen(s: &Session) -> String {
        net::lock(&s.terminal).all_text()
    }

    #[test]
    fn a_lost_connection_is_tried_again_with_waits_until_stopped_r_ssh_51() {
        let ctx = egui::Context::default();
        let mut s = failing();
        s.reconnect_minutes = 10;
        s.poll();
        // As if the shell had run: the failure is a lost connection.
        s.started = true;
        s.follow_up(&ctx, 0.0);
        assert!(
            screen(&s).contains("The connection was lost"),
            "{}",
            screen(&s)
        );
        assert_eq!(s.reconnecting(0.0), Some((1, 1)));
        s.follow_up(&ctx, 1.0);
        s.poll();
        s.follow_up(&ctx, 1.0);
        assert!(
            screen(&s).contains("Try 1 failed. Trying again in 2 s."),
            "{}",
            screen(&s)
        );
        assert_eq!(s.reconnecting(1.0), Some((2, 2)));
        assert!(
            s.log.iter().any(|l| l.text.contains("lost")),
            "the log is kept across tries"
        );
        s.stop_reconnecting();
        assert_eq!(s.reconnecting(1.0), None);
        assert!(screen(&s).contains("Stopped reconnecting."));
    }

    #[test]
    fn nothing_reconnects_when_the_setting_is_off_or_the_shell_never_ran_r_ssh_51() {
        let ctx = egui::Context::default();
        let mut never_ran = failing();
        never_ran.reconnect_minutes = 10;
        never_ran.poll();
        never_ran.follow_up(&ctx, 0.0);
        assert_eq!(never_ran.reconnecting(0.0), None);
        let mut off = failing();
        off.poll();
        off.started = true;
        off.follow_up(&ctx, 0.0);
        assert_eq!(off.reconnecting(0.0), None);
    }

    #[test]
    fn find_in_the_scrollback_counts_matches_with_and_without_case_and_closes_r_ssh_24() {
        use egui_kittest::kittest::Queryable;
        let mut s = failing();
        net::lock(&s.terminal).feed(b"Error one\r\nerror two\r\n");
        s.open_find();
        let mut h = egui_kittest::Harness::builder()
            .with_size(egui::vec2(800.0, 400.0))
            .build_ui_state(
                |ui, s: &mut Session| {
                    parley_ui_egui::test_support::install_fonts(ui);
                    s.show(ui, &screen::Look::default(), false, true);
                },
                s,
            );
        h.run_steps(2);
        h.get_by_label(parley_ui_egui::find::FIELD)
            .type_text("error");
        h.run_steps(2);
        h.get_by_label("1 of 2");
        assert_eq!(h.state().view.highlights.len(), 2);
        h.get_by_label("Match case").click();
        h.run_steps(2);
        h.get_by_label("1 of 1");
        h.get_by_label("Close find").click();
        h.run_steps(2);
        assert!(h.state().find.is_none());
        assert_eq!(h.state().view.highlights, []);
    }

    #[test]
    fn a_long_command_and_the_hosts_notifications_become_notes_r_ssh_46() {
        let ctx = egui::Context::default();
        let mut s = failing();
        net::lock(&s.terminal).feed(b"\x1b]133;C\x07");
        assert_eq!(s.shell_events(&ctx, 0.0, true), []);
        net::lock(&s.terminal).feed(b"\x1b]133;D;2\x07\x1b]133;C\x07");
        let notes = s.shell_events(&ctx, 12.0, true);
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].title, "me@box");
        assert_eq!(notes[0].body, "A command failed with status 2 after 12 s.");
        net::lock(&s.terminal).feed(b"\x1b]133;D;0\x07\x1b]777;notify;Make;done\x07");
        let notes = s.shell_events(&ctx, 15.0, true);
        assert_eq!(
            notes,
            [Note {
                title: "Make".into(),
                body: "done".into()
            }],
            "3 s is not long"
        );
    }

    #[test]
    fn the_clipboard_is_set_only_when_the_setting_allows_r_ssh_47() {
        let ctx = egui::Context::default();
        let mut s = failing();
        net::lock(&s.terminal).feed(b"\x1b]52;c;aGVq\x07");
        s.shell_events(&ctx, 0.0, false);
        assert!(
            s.log
                .iter()
                .any(|l| l.text.contains("the setting refuses it"))
        );
        net::lock(&s.terminal).feed(b"\x1b]52;c;aGVq\x07");
        s.shell_events(&ctx, 0.0, true);
        assert!(
            s.log
                .iter()
                .any(|l| l.text == "The server set the clipboard (3 characters)")
        );
    }

    #[test]
    fn a_session_log_keeps_what_the_host_sends_while_on_r_ssh_25() {
        let ctx = egui::Context::default();
        let folder =
            std::env::temp_dir().join(format!("parley-ssh-session-log-{}", std::process::id()));
        let mut s = failing();
        s.source.log_pattern = format!("{}/&H.log", folder.display());
        net::lock(&s.terminal).feed(b"before\r\n");
        s.start_log();
        assert!(s.logging());
        net::lock(&s.terminal).feed(b"\x1b[32mafter \xc3\xa6\r\n");
        s.shell_events(&ctx, 0.0, true);
        s.stop_log();
        net::lock(&s.terminal).feed(b"not kept\r\n");
        s.shell_events(&ctx, 0.0, true);
        let kept = std::fs::read_to_string(folder.join("box.log")).expect("the log");
        assert!(
            kept.contains("after æ\n") && !kept.contains("before") && !kept.contains("not kept"),
            "{kept}"
        );
        assert!(
            s.log
                .iter()
                .any(|l| l.text.starts_with("Stopped logging to"))
        );
        std::fs::remove_dir_all(&folder).expect("cleaned");
    }

    #[test]
    fn the_hosts_bell_is_noticed_for_the_window_r_ssh_19() {
        let ctx = egui::Context::default();
        let mut s = failing();
        s.shell_events(&ctx, 0.0, true);
        assert!(!s.rang);
        net::lock(&s.terminal).feed(b"\x07");
        s.shell_events(&ctx, 0.0, true);
        assert!(s.rang);
    }
}
