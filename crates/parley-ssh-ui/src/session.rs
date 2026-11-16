//! One SSH session in the window (D45): its connection, its terminal, where it stands, and the questions it
//! asks while connecting (R-SSH-3, R-SSH-4) and pasting (R-SSH-9).

use std::sync::{Arc, Mutex};

use parley_ssh_core::keys;
use parley_ssh_core::known_hosts::KnownHosts;
use parley_ssh_core::net::{self, Algorithms, Command, Connection, Ending, Event, Target};
use parley_ssh_core::term::Terminal;

use crate::dialogs;
use crate::screen::{self, Out, Shortcut};

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
pub struct Session {
    pub target: Target,
    connection: Connection,
    terminal: Arc<Mutex<Terminal>>,
    pub state: State,
    pub algorithms: Option<Algorithms>,
    pub fingerprint: Option<String>,
    /// What happened, for the event log (R-SSH-16 shows it in Phase 27).
    pub log: Vec<String>,
    pub view: screen::View,
    host_key: Option<dialogs::HostKey>,
    prompt: Option<dialogs::Prompt>,
    /// The server's banner, shown with the next prompt (R-SSH-4).
    banner: Option<String>,
    paste: Option<String>,
    /// Take the keyboard on the next frame (after a dialog closed).
    grab_focus: bool,
    known_hosts: KnownHosts,
}

impl Session {
    /// Starts connecting; `repaint` wakes the UI when there is news.
    pub fn open(
        target: Target,
        known_hosts: KnownHosts,
        repaint: impl Fn() + Send + Sync + 'static,
    ) -> Self {
        Self::open_on(
            target,
            known_hosts,
            Arc::new(Mutex::new(Terminal::new(80, 24))),
            repaint,
        )
    }

    fn open_on(
        target: Target,
        known_hosts: KnownHosts,
        terminal: Arc<Mutex<Terminal>>,
        repaint: impl Fn() + Send + Sync + 'static,
    ) -> Self {
        let connection = Connection::open(
            target.clone(),
            known_hosts.clone(),
            Arc::clone(&terminal),
            repaint,
        );
        Self {
            target,
            connection,
            terminal,
            state: State::Connecting,
            algorithms: None,
            fingerprint: None,
            log: Vec::new(),
            view: screen::View::default(),
            host_key: None,
            prompt: None,
            banner: None,
            paste: None,
            grab_focus: true,
            known_hosts,
        }
    }

    /// Connects again with the same settings, on the same screen, as `PuTTY`'s Restart Session (R-SSH-5).
    pub fn restart(&mut self, repaint: impl Fn() + Send + Sync + 'static) {
        let terminal = Arc::clone(&self.terminal);
        net::lock(&terminal).feed(b"\r\n");
        let zoom = self.view.zoom;
        *self = Self::open_on(
            self.target.clone(),
            self.known_hosts.clone(),
            terminal,
            repaint,
        );
        self.view.zoom = zoom;
    }

    pub fn is_connected(&self) -> bool {
        !matches!(self.state, State::Ended(_))
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
    pub fn focus_screen(&mut self) {
        self.grab_focus = true;
    }

    pub fn disconnect(&self) {
        self.connection.send(Command::Disconnect);
    }

    /// Takes in what the connection said since the last frame.
    pub fn poll(&mut self) {
        let events: Vec<Event> = self.connection.events().collect();
        for event in events {
            match event {
                Event::Log(line) => self.log.push(line),
                Event::HostKey(question) => self.host_key = Some(dialogs::HostKey::new(question)),
                Event::Banner(text) => self.banner = Some(text),
                Event::Prompt(prompt) => {
                    self.prompt = Some(dialogs::Prompt::new(prompt, self.banner.take()));
                }
                Event::Secured {
                    algorithms,
                    fingerprint,
                } => {
                    self.algorithms = Some(algorithms);
                    self.fingerprint = Some(fingerprint);
                    if self.state == State::Connecting {
                        self.state = State::SigningOn;
                    }
                }
                Event::Started => {
                    self.state = State::Running;
                    self.grab_focus = true;
                }
                Event::Ended(ending) => {
                    self.log.push(ending_text(&ending));
                    self.host_key = None;
                    self.prompt = None;
                    self.state = State::Ended(ending);
                }
            }
        }
    }

    /// Draws the screen and the session's dialogs; returns the shortcuts the window carries out.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        palette: &screen::Palette,
        confirm_paste: bool,
    ) -> Vec<Shortcut> {
        self.poll();
        let mut shortcuts = Vec::new();
        let dialog_open = self.host_key.is_some() || self.prompt.is_some() || self.paste.is_some();
        let grab = std::mem::take(&mut self.grab_focus) && !dialog_open;
        for out in screen::show(ui, &self.terminal, &mut self.view, palette, grab) {
            match out {
                Out::Send(bytes) => self.send(bytes),
                Out::Resized { columns, rows } => {
                    self.connection.send(Command::Resize { columns, rows });
                }
                Out::Paste(text) => self.paste(&text, confirm_paste),
                Out::Shortcut(shortcut) => shortcuts.push(shortcut),
            }
        }
        self.show_dialogs(ui.ctx());
        shortcuts
    }

    /// Pastes `text` as the Edit menu does (R-SSH-9), asking first when needed.
    pub fn paste(&mut self, text: &str, confirm: bool) {
        let modes = *net::lock(&self.terminal).modes();
        if confirm && keys::paste_needs_confirmation(text, &modes) {
            self.paste = Some(text.to_owned());
        } else {
            self.send(keys::paste(text, &modes));
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
            && let Some(answers) = dialog.show(ctx, &self.target)
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
                self.send(keys::paste(text, &modes));
            }
            self.paste = None;
            self.grab_focus = true;
        }
    }
}

/// How a session ended, in words for the status bar and the log (R-SSH-5).
pub fn ending_text(ending: &Ending) -> String {
    match ending {
        Ending::Exited(0) => "The shell ended.".to_owned(),
        Ending::Exited(status) => format!("The shell ended with exit status {status}."),
        Ending::Signal(signal) => format!("The shell was ended by signal {signal}."),
        Ending::Closed => "The server closed the connection.".to_owned(),
        Ending::Disconnected => "Disconnected.".to_owned(),
        Ending::Failed(why) => format!("Could not connect: {why}."),
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
}
