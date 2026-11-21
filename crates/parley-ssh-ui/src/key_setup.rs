//! Setting up key sign-on from the window (R-SSH-67, R-SSH-68, D63): which key, making one when there is none,
//! adding it to the server's `authorized_keys` over a side channel, IBM i's folders and their Fix, and the saved
//! session told to use the key. The steps are a small state machine fed with the session's job events; what it
//! needs done comes back as [`Order`]s; [`view`] draws it.

mod view;
mod window;

use std::path::{Path, PathBuf};

use parley_ssh_core::Secret;
use parley_ssh_core::key_setup::{
    self as setup, Added, Checked, OpenFolder, PublicLine, make_key, usual_keys,
};
use parley_ssh_core::net::{Command, Event, Exec, JobEnding, JobId};

use crate::panes::PaneId;

/// Key setup's jobs are numbered from here, far from the Transfer panel's, so the session hands each its own.
pub const KEY_JOBS: u64 = 1 << 62;

/// Whether `event` is for key setup: its jobs and the agent's keys.
pub fn is_key_event(event: &Event) -> bool {
    match event {
        Event::JobOutput { id, .. }
        | Event::JobEnded { id, .. }
        | Event::Door { id, .. }
        | Event::Forwarded { id, .. } => id.0 >= KEY_JOBS,
        Event::AgentKeys(_) => true,
        _ => false,
    }
}

/// What was clicked in the bar that offers key sign-on (D63).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Offer {
    SetUp,
    /// Kept in the saved session, which then never offers it again.
    NotForThisSession,
}

/// The offer's one line, above the terminal.
pub fn offer_bar(ui: &mut egui::Ui) -> Option<Offer> {
    ui.horizontal(|ui| {
        ui.label("Sign on with a key next time?");
        if ui
            .add(parley_ui_egui::theme::primary_button(ui, "Set up…"))
            .clicked()
        {
            Some(Offer::SetUp)
        } else if ui
            .add(parley_ui_egui::theme::ghost_button("Not for this session"))
            .clicked()
        {
            Some(Offer::NotForThisSession)
        } else {
            None
        }
    })
    .inner
}

/// A key that can be put on the server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Choice {
    /// A key file on this computer, with its public key or why it cannot be read.
    File(PathBuf, Result<PublicLine, String>),
    /// One of the running agent's keys.
    Agent(PublicLine),
    /// A new key (R-SSH-68).
    New,
}

impl Choice {
    fn line(&self) -> Option<&PublicLine> {
        match self {
            Self::File(_, Ok(line)) | Self::Agent(line) => Some(line),
            Self::File(_, Err(_)) | Self::New => None,
        }
    }
}

/// What the agent said about its keys.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentState {
    Asking,
    Listed,
    Unavailable(String),
}

/// Where the setup is.
#[derive(Debug)]
pub enum Stage {
    Choose,
    /// Making a key: its passphrase, asked twice.
    Make {
        passphrase: Secret,
        again: Secret,
        error: Option<String>,
    },
    /// The server's system and folders are being checked.
    Checking {
        job: JobId,
        output: String,
    },
    /// The key is being added.
    Adding {
        job: JobId,
        output: String,
        checked: Checked,
    },
    Done {
        added: Added,
        to_fix: Vec<OpenFolder>,
        fix: Fix,
    },
    Failed(String),
}

/// IBM i's folders after the key was added (D63).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fix {
    /// Nothing to fix, or not asked yet.
    Offered,
    /// Asking whether to change the folders.
    Asking,
    Running {
        job: JobId,
        output: String,
    },
    Fixed,
    Failed(String),
}

/// What the window carries out for the setup.
#[derive(Debug)]
pub enum Order {
    /// A command for the session's connection.
    Send(Command),
    /// The saved session (by name) signs on with this key from now on.
    Remember {
        session: String,
        key: Choice,
    },
    Close,
}

/// A key setup for the session in one pane.
#[derive(Debug)]
pub struct KeySetup {
    pub pane: PaneId,
    /// `user@host` on the server.
    pub server: String,
    /// The saved session's name; `None` for one not saved.
    pub saved: Option<String>,
    pub choices: Vec<Choice>,
    pub chosen: usize,
    /// The user picked a key: the agent's keys coming later do not move the choice.
    touched: bool,
    /// How many of the choices are the session's own key files: they come first.
    own: usize,
    pub agent: AgentState,
    pub stage: Stage,
    /// The key being put on the server.
    key: Option<Choice>,
    ssh_dir: PathBuf,
    /// A new key's comment: `user@computer`.
    pub comment: String,
    next_job: u64,
}

impl KeySetup {
    /// Opens for the session in `pane` signed on as `server` (`user@host`), with its key files; asks the agent for
    /// its keys at once. `ssh_dir` is this computer's `~/.ssh`.
    pub fn open(
        pane: PaneId,
        server: String,
        saved: Option<String>,
        key_files: &[PathBuf],
        ssh_dir: &Path,
        comment: String,
    ) -> (Self, Vec<Order>) {
        let mut choices: Vec<Choice> = key_files
            .iter()
            .map(|path| Choice::File(path.clone(), setup::public_line(path)))
            .collect();
        for path in usual_keys(ssh_dir) {
            if !key_files.contains(&path) {
                let line = setup::public_line(&path);
                choices.push(Choice::File(path, line));
            }
        }
        choices.push(Choice::New);
        let chosen = choices
            .iter()
            .position(|c| c.line().is_some())
            .unwrap_or(choices.len() - 1);
        let setup = Self {
            pane,
            server,
            saved,
            choices,
            chosen,
            touched: false,
            own: key_files.len(),
            agent: AgentState::Asking,
            stage: Stage::Choose,
            key: None,
            ssh_dir: ssh_dir.to_owned(),
            comment,
            next_job: 0,
        };
        (setup, vec![Order::Send(Command::AgentKeys)])
    }

    /// The user picked choice `i`.
    pub fn choose(&mut self, i: usize) {
        if i < self.choices.len() {
            self.chosen = i;
            self.touched = true;
        }
    }

    fn job(&mut self) -> JobId {
        self.next_job += 1;
        JobId(KEY_JOBS + self.next_job)
    }

    /// Set up with the key chosen: a new one is made first.
    pub fn start(&mut self) -> Vec<Order> {
        match self.choices.get(self.chosen) {
            Some(Choice::New) => {
                self.stage = Stage::Make {
                    passphrase: Secret::new(String::new()),
                    again: Secret::new(String::new()),
                    error: None,
                };
                Vec::new()
            }
            Some(choice) if choice.line().is_some() => {
                self.key = Some(choice.clone());
                self.check()
            }
            Some(_) | None => Vec::new(),
        }
    }

    /// Makes the key with the passphrase typed, then goes on with it.
    pub fn make(&mut self) -> Vec<Order> {
        let Stage::Make {
            passphrase,
            again,
            error,
        } = &mut self.stage
        else {
            return Vec::new();
        };
        if passphrase.expose() != again.expose() {
            *error = Some("The passphrases are not the same.".to_owned());
            return Vec::new();
        }
        match make_key(&self.ssh_dir, self.comment.trim(), Some(passphrase)) {
            Ok(made) => {
                let choice = Choice::File(made.private, Ok(made.public));
                self.choices.insert(self.chosen, choice.clone());
                self.key = Some(choice);
                self.check()
            }
            Err(why) => {
                *error = Some(why);
                Vec::new()
            }
        }
    }

    /// Back to the choice of key, from making one.
    pub fn back(&mut self) {
        if matches!(self.stage, Stage::Make { .. }) {
            self.stage = Stage::Choose;
        }
    }

    fn check(&mut self) -> Vec<Order> {
        let job = self.job();
        self.stage = Stage::Checking {
            job,
            output: String::new(),
        };
        vec![exec(job, setup::check_command().to_owned())]
    }

    /// Asks before Fix changes the folders.
    pub fn ask_fix(&mut self) {
        if let Stage::Done { fix, to_fix, .. } = &mut self.stage
            && !to_fix.is_empty()
        {
            *fix = Fix::Asking;
        }
    }

    /// Fix, once asked: takes the group's and others' write permission from the folders found.
    pub fn fix(&mut self) -> Vec<Order> {
        let job = self.job();
        let Stage::Done { to_fix, fix, .. } = &mut self.stage else {
            return Vec::new();
        };
        if *fix != Fix::Asking {
            return Vec::new();
        }
        *fix = Fix::Running {
            job,
            output: String::new(),
        };
        vec![exec(job, setup::fix_command(to_fix))]
    }

    /// Not now: back from asking.
    pub fn keep_folders(&mut self) {
        if let Stage::Done { fix, .. } = &mut self.stage
            && *fix == Fix::Asking
        {
            *fix = Fix::Offered;
        }
    }

    /// Closes the dialog; a job still running is ended.
    pub fn close(&self) -> Vec<Order> {
        let running = match &self.stage {
            Stage::Checking { job, .. }
            | Stage::Adding { job, .. }
            | Stage::Done {
                fix: Fix::Running { job, .. },
                ..
            } => Some(*job),
            Stage::Choose | Stage::Make { .. } | Stage::Done { .. } | Stage::Failed(_) => None,
        };
        let mut orders: Vec<Order> = running
            .map(|job| Order::Send(Command::EndJob(job)))
            .into_iter()
            .collect();
        orders.push(Order::Close);
        orders
    }

    /// What the session's connection said for the setup.
    pub fn feed(&mut self, event: Event) -> Vec<Order> {
        match event {
            Event::AgentKeys(keys) => {
                self.agent_keys(keys);
                Vec::new()
            }
            Event::JobOutput { id, data, .. } => {
                if let Some(output) = self.output_of(id) {
                    output.push_str(&String::from_utf8_lossy(&data));
                }
                Vec::new()
            }
            Event::JobEnded { id, ending } => self.ended(id, &ending),
            _ => Vec::new(),
        }
    }

    /// The agent's keys go after the session's own key files and before the usual ones; the first is chosen
    /// unless the user chose, or the session has a key file of its own that can be read.
    fn agent_keys(&mut self, keys: Result<Vec<PublicLine>, String>) {
        match keys {
            Ok(keys) if keys.is_empty() => {
                self.agent = AgentState::Unavailable("The agent holds no keys.".to_owned());
            }
            Ok(keys) => {
                let at = self.own;
                let own_line = self.choices[..at].iter().any(|c| c.line().is_some());
                let count = keys.len();
                for (n, key) in keys.into_iter().enumerate() {
                    self.choices.insert(at + n, Choice::Agent(key));
                }
                if !self.touched && !own_line {
                    self.chosen = at;
                } else if self.chosen >= at {
                    self.chosen += count;
                }
                self.agent = AgentState::Listed;
            }
            Err(why) => self.agent = AgentState::Unavailable(why),
        }
    }

    fn output_of(&mut self, id: JobId) -> Option<&mut String> {
        match &mut self.stage {
            Stage::Checking { job, output } | Stage::Adding { job, output, .. } if *job == id => {
                Some(output)
            }
            Stage::Done {
                fix: Fix::Running { job, output },
                ..
            } if *job == id => Some(output),
            _ => None,
        }
    }

    fn ended(&mut self, id: JobId, ending: &JobEnding) -> Vec<Order> {
        let status = match ending {
            JobEnding::Exited(status) => Some(*status),
            JobEnding::Signal(_) | JobEnding::Stopped | JobEnding::Closed => None,
            JobEnding::Refused(why) => {
                if self.output_of(id).is_some() {
                    self.stage = Stage::Failed(format!("The server refused the command: {why}"));
                }
                return Vec::new();
            }
        };
        match std::mem::replace(&mut self.stage, Stage::Choose) {
            Stage::Checking { job, output } if job == id => {
                let checked = setup::read_checked(&output);
                let job = self.job();
                let Some(line) = self.key.as_ref().and_then(Choice::line).cloned() else {
                    self.stage = Stage::Failed("No key was chosen.".to_owned());
                    return Vec::new();
                };
                self.stage = Stage::Adding {
                    job,
                    output: String::new(),
                    checked,
                };
                vec![exec(job, setup::add_command(&line))]
            }
            Stage::Adding {
                job,
                output,
                checked,
            } if job == id => match setup::read_added(&output, status) {
                Ok(added) => {
                    self.stage = Stage::Done {
                        added,
                        to_fix: checked.to_fix().to_vec(),
                        fix: Fix::Offered,
                    };
                    self.remember()
                }
                Err(why) => {
                    self.stage = Stage::Failed(why);
                    Vec::new()
                }
            },
            Stage::Done {
                added,
                to_fix,
                fix: Fix::Running { job, output },
            } if job == id => {
                let fix = if status == Some(0) {
                    Fix::Fixed
                } else {
                    let last = output.lines().map(str::trim).rfind(|l| !l.is_empty());
                    Fix::Failed(last.map_or_else(
                        || "The folders could not be changed.".to_owned(),
                        |l| format!("The folders could not be changed: {l}"),
                    ))
                };
                self.stage = Stage::Done { added, to_fix, fix };
                Vec::new()
            }
            other => {
                self.stage = other;
                Vec::new()
            }
        }
    }

    /// The saved session uses the key from now on.
    fn remember(&self) -> Vec<Order> {
        match (&self.saved, &self.key) {
            (Some(session), Some(key)) => vec![Order::Remember {
                session: session.clone(),
                key: key.clone(),
            }],
            _ => Vec::new(),
        }
    }
}

fn exec(id: JobId, command: String) -> Order {
    Order::Send(Command::Exec(Exec {
        id,
        command,
        agent: false,
    }))
}

/// `key` remembered in a saved session: a key file goes first in its list, an agent key becomes its only agent key.
pub fn remember(session: &mut crate::sessions::SavedSession, key: &Choice) {
    match key {
        Choice::File(path, _) => {
            let path = path.display().to_string();
            session.key_files.retain(|f| f.trim() != path);
            session.key_files.insert(0, path);
        }
        Choice::Agent(line) => {
            session.use_agent = true;
            session.agent_key.clone_from(&line.fingerprint);
        }
        Choice::New => {}
    }
}

/// This computer's name for a new key's comment, from the environment; "computer" when it gives none.
pub fn computer_name() -> String {
    ["HOSTNAME", "COMPUTERNAME"]
        .iter()
        .find_map(|v| std::env::var(v).ok().filter(|n| !n.trim().is_empty()))
        .or_else(|| {
            std::fs::read_to_string("/etc/hostname")
                .ok()
                .map(|n| n.trim().to_owned())
                .filter(|n| !n.is_empty())
        })
        .unwrap_or_else(|| "computer".to_owned())
}

#[cfg(test)]
mod tests;
