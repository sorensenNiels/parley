//! Recording and playing keyboard macros (R-KEY-3, R-KEY-4). The operator's keys come in
//! through [`Session::key`], which records them and keeps them out of a running macro; the
//! macro presses keys through [`Session::press`].

use std::time::{Duration, Instant};

use super::Session;
use crate::macros::{Macro, Step};
use crate::password::Password;
use crate::session::Key;

/// How long a macro waits for the host before it gives up.
pub const MACRO_TIMEOUT: Duration = Duration::from_secs(30);

/// What the session is doing with macros.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MacroState {
    Off,
    Recording,
    Playing,
    /// At a `password` step: the UI asks the operator for it ([`Session::give_macro_password`]);
    /// Parley keeps none (R-NET-7).
    NeedsPassword,
}

/// A macro being played.
#[derive(Debug)]
pub(super) struct Player {
    name: String,
    steps: Vec<Step>,
    next: usize,
    /// The unlock count after the last step that was not `wait`: a `wait` step ends when the
    /// host has unlocked the keyboard since.
    mark: u64,
    /// When the macro was first seen waiting for the host, for the time-out.
    waiting_since: Option<Instant>,
    needs_password: bool,
}

impl Session {
    /// A key from the operator. While a macro runs, Reset and Attention stop it and other keys
    /// are ignored; while recording, keys the session accepts are recorded.
    pub fn key(&mut self, key: Key) {
        if let Some(player) = &self.player {
            if matches!(key, Key::Reset | Key::Attention) {
                let name = player.name.clone();
                self.stop_macro(&format!("Macro \"{name}\" stopped."));
            }
            return;
        }
        let hidden = self.cursor_in_hidden_field();
        let had_error = self.error.is_some();
        let locked = self.locked;
        self.press(key);
        let Some(steps) = self.recording.as_mut() else {
            return;
        };
        // Keys the session refused are not recorded, nor the Reset that clears their error.
        let refused = self.error.is_some() && !had_error;
        let accepted = match key {
            Key::Reset => !had_error,
            Key::Attention | Key::SystemRequest => true,
            Key::Char(_)
            | Key::Aid(_)
            | Key::FieldExit
            | Key::FieldPlus
            | Key::FieldMinus
            | Key::Tab
            | Key::BackTab
            | Key::Up
            | Key::Down
            | Key::Left
            | Key::Right
            | Key::Home
            | Key::End
            | Key::Backspace
            | Key::Delete
            | Key::ToggleInsert
            | Key::EraseEof
            | Key::MoveTo { .. } => !had_error && !locked && !refused,
        };
        if accepted {
            record(steps, key, hidden);
        }
    }

    /// The keys of a clicked hotspot (R-UI-20), each as [`Session::key`] takes it, except that
    /// while recording the move to an option field is kept too (a click elsewhere is not): the
    /// macro then types the option where the click did.
    pub fn hotspot_keys(&mut self, keys: &[Key]) {
        for &key in keys {
            let ready = self.player.is_none() && !self.locked && self.error.is_none();
            self.key(key);
            if let (Key::MoveTo { .. }, true, Some(steps)) = (key, ready, self.recording.as_mut()) {
                steps.push(Step::Key(key));
            }
        }
    }

    pub const fn macro_state(&self) -> MacroState {
        match (&self.player, &self.recording) {
            (Some(p), _) if p.needs_password => MacroState::NeedsPassword,
            (Some(_), _) => MacroState::Playing,
            (None, Some(_)) => MacroState::Recording,
            (None, None) => MacroState::Off,
        }
    }

    /// The name of the macro being played.
    pub fn playing(&self) -> Option<&str> {
        self.player.as_ref().map(|p| p.name.as_str())
    }

    /// Starts recording the operator's keys (not while a macro runs).
    pub fn start_recording(&mut self) {
        if self.player.is_none() {
            self.recording = Some(Vec::new());
        }
    }

    /// Stops recording and gives what was recorded, if recording.
    pub fn stop_recording(&mut self) -> Option<Macro> {
        self.recording.take().map(|steps| Macro {
            session: None,
            steps,
        })
    }

    /// Plays a macro from the first screen that waits for input: at once if the screen
    /// shown does, else when the host sends one (a macro run at connect may sign on itself).
    pub fn play(&mut self, name: &str, steps: Vec<Step>) {
        self.recording = None;
        self.player = Some(Player {
            name: name.to_owned(),
            steps,
            next: 0,
            mark: self.unlocks,
            waiting_since: None,
            needs_password: false,
        });
        self.advance_macro();
    }

    /// The password a `password` step asked for; `None` stops the macro.
    pub fn give_macro_password(&mut self, password: Option<&Password>) {
        let Some(player) = self.player.as_mut().filter(|p| p.needs_password) else {
            return;
        };
        let Some(password) = password else {
            let name = player.name.clone();
            return self.stop_macro(&format!("Macro \"{name}\" stopped: no password."));
        };
        player.needs_password = false;
        player.next += 1;
        self.type_text(password.expose());
        let unlocks = self.unlocks;
        if let Some(player) = self.player.as_mut() {
            player.mark = unlocks;
        }
        self.advance_macro();
    }

    /// Gives up on a macro that has waited for the host longer than [`MACRO_TIMEOUT`]; true if
    /// it did, so the screen shows why.
    pub fn check_macro_time(&mut self, now: Instant) -> bool {
        let Some(player) = self.player.as_mut().filter(|p| !p.needs_password) else {
            return false;
        };
        let since = *player.waiting_since.get_or_insert(now);
        if now.duration_since(since) < MACRO_TIMEOUT {
            return false;
        }
        let name = player.name.clone();
        self.stop_macro(&format!(
            "Macro \"{name}\" stopped: the host did not answer within {} s.",
            MACRO_TIMEOUT.as_secs()
        ));
        true
    }

    fn stop_macro(&mut self, why: &str) {
        self.player = None;
        self.notice = Some(why.to_owned());
    }

    /// Plays steps until the macro has to wait for the host or for a password, or ends.
    pub(super) fn advance_macro(&mut self) {
        loop {
            let free = !self.locked && self.pending_read.is_some();
            let error = self.error.is_some();
            let unlocks = self.unlocks;
            let Some(player) = self.player.as_mut() else {
                return;
            };
            if error {
                let name = player.name.clone();
                return self.stop_macro(&format!(
                    "Macro \"{name}\" stopped at an error: see the screen."
                ));
            }
            if player.needs_password || !free {
                return;
            }
            player.waiting_since = None;
            let Some(step) = player.steps.get(player.next).cloned() else {
                self.player = None;
                return;
            };
            match &step {
                Step::Wait if unlocks == player.mark => return,
                // Parley keeps no password (R-NET-7): the UI asks for it every time.
                Step::Password => {
                    player.needs_password = true;
                    return;
                }
                Step::Wait | Step::Text(_) | Step::Key(_) => player.next += 1,
            }
            match step {
                Step::Wait | Step::Password => continue,
                Step::Text(text) => self.type_text(&text),
                Step::Key(key) => self.press(key),
            }
            let unlocks = self.unlocks;
            if let Some(player) = self.player.as_mut() {
                player.mark = unlocks;
            }
        }
    }

    fn type_text(&mut self, text: &str) {
        for c in text.chars() {
            self.press(Key::Char(c));
        }
    }

    /// Whether the cursor is in a non-display field, such as a password.
    fn cursor_in_hidden_field(&self) -> bool {
        self.screen
            .field_at(self.screen.cursor)
            .and_then(|i| self.screen.fields().get(i))
            .is_some_and(|f| self.field_is_hidden(f.start))
    }
}

/// Adds an accepted key to the recording. Text in a non-display field becomes one `password`
/// step; editing keys there and mouse clicks are left out. An AID key is followed by `wait`.
fn record(steps: &mut Vec<Step>, key: Key, hidden: bool) {
    match key {
        Key::Char(_) | Key::Backspace | Key::Delete | Key::EraseEof if hidden => {
            if steps.last() != Some(&Step::Password) {
                steps.push(Step::Password);
            }
        }
        Key::Char(c) => match steps.last_mut() {
            Some(Step::Text(text)) => text.push(c),
            _ => steps.push(Step::Text(c.to_string())),
        },
        Key::MoveTo { .. } => {}
        Key::Aid(_) | Key::Attention | Key::SystemRequest => {
            steps.push(Step::Key(key));
            steps.push(Step::Wait);
        }
        Key::FieldExit
        | Key::FieldPlus
        | Key::FieldMinus
        | Key::Tab
        | Key::BackTab
        | Key::Up
        | Key::Down
        | Key::Left
        | Key::Right
        | Key::Home
        | Key::End
        | Key::Backspace
        | Key::Delete
        | Key::ToggleInsert
        | Key::EraseEof
        | Key::Reset => steps.push(Step::Key(key)),
    }
}
