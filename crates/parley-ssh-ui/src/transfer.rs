//! The Transfer panel (Phase 31, R-SSH-56, R-SSH-53): a source and a target, each an open session and a path, and
//! rsync's switches. Copy checks rsync on both ends (R-SSH-57), runs a dry run that lists what will be added,
//! changed and deleted, and copies once that is confirmed: rsync on the source straight to the target, with
//! progress and Cancel. Transfers are saved by name with their sessions' saved names. Beside the panes, so the
//! terminals stay in use while it copies. `view` draws the panel, `window` ties it to the window's sessions.

use parley_ssh_core::net::{Event, Exec, JobEnding, JobId, Stream};
use parley_ssh_core::transfer::{self, Changes, Check, Direct, Mode, Progress, Remote};
use serde::{Deserialize, Serialize};

use crate::panes::PaneId;
use crate::session::Session;

mod view;
mod window;

/// The most of a job's output kept: enough for rsync's last progress line and its last error lines.
const KEPT: usize = 4096;

/// rsync's switches as the panel shows them (R-SSH-56).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Switches {
    /// Delete files at the target that the source lacks; off.
    pub delete: bool,
    /// Keep permissions, owner and times; on.
    pub keep: bool,
    /// Compress on the way; on.
    pub compress: bool,
    /// Patterns left out, one a line.
    pub excludes: String,
}

impl Default for Switches {
    fn default() -> Self {
        Self {
            delete: false,
            keep: true,
            compress: true,
            excludes: String::new(),
        }
    }
}

impl Switches {
    fn options(&self) -> transfer::Options {
        transfer::Options {
            delete: self.delete,
            keep: self.keep,
            compress: self.compress,
            excludes: self.excludes.lines().map(str::to_owned).collect(),
        }
    }
}

/// A transfer kept by name (R-SSH-56): its sessions by their saved names, so it runs again in any window.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SavedTransfer {
    pub name: String,
    pub source: String,
    pub source_path: String,
    pub target: String,
    pub target_path: String,
    pub switches: Switches,
}

/// One end of a transfer, as the window sees the session in that pane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct End {
    pub pane: PaneId,
    pub label: String,
    pub user: String,
    pub host: String,
    pub port: u16,
    pub host_key: Option<String>,
    pub through_jump: bool,
}

impl End {
    fn of(pane: PaneId, session: &Session) -> Self {
        Self {
            pane,
            label: session.target.label(),
            user: session.target.user.clone(),
            host: session.target.host.clone(),
            port: session.target.port,
            host_key: session.host_key_line.clone(),
            through_jump: session.through_jump(),
        }
    }
}

/// Something for a session to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Order {
    Exec(PaneId, Exec),
    End(PaneId, JobId),
}

/// A job's output so far, kept short.
#[derive(Debug, Default)]
struct Output {
    out: String,
    err: String,
}

impl Output {
    fn push(&mut self, stream: Stream, data: &[u8]) {
        let text = match stream {
            Stream::Out => &mut self.out,
            Stream::Err => &mut self.err,
        };
        text.push_str(&String::from_utf8_lossy(data));
        if text.len() > KEPT {
            let mut cut = text.len() - KEPT;
            while !text.is_char_boundary(cut) {
                cut += 1;
            }
            text.drain(..cut);
        }
    }
}

/// Where a running transfer is.
#[derive(Debug)]
enum Stage {
    /// `rsync --version` on both ends; each found rsync once its job ends.
    Checking {
        source: (JobId, Output, Option<Check>),
        target: (JobId, Output, Option<Check>),
    },
    /// The dry run, with the copy's command for once it is confirmed.
    Looking {
        job: JobId,
        output: Output,
        changes: Changes,
        copy: String,
    },
    /// The dry run's list, waiting for the user.
    Review { changes: Changes, copy: String },
    Copying {
        job: JobId,
        output: Output,
        progress: Option<Progress>,
        cancelled: bool,
    },
}

#[derive(Debug)]
struct Run {
    source: End,
    target: End,
    source_path: String,
    target_path: String,
    stage: Stage,
}

/// How the last transfer ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Done(String),
    Cancelled,
    Failed(String),
}

/// The panel's state.
#[derive(Debug, Default)]
pub struct Panel {
    pub source: Option<PaneId>,
    pub source_path: String,
    pub target: Option<PaneId>,
    pub target_path: String,
    pub switches: Switches,
    /// The name to save the transfer under.
    pub name: String,
    /// Start as soon as both sessions are connected: a saved transfer run from the palette.
    pub waiting: bool,
    run: Option<Run>,
    pub outcome: Option<Outcome>,
    next_job: u64,
}

/// What the user asked for in the panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Request {
    /// Check and dry run.
    Copy,
    /// Copy what the dry run listed.
    Confirm,
    Cancel,
    Close,
    /// Fill the panel from the saved transfer at this index.
    Load(usize),
    Save,
    /// Forget the saved transfer at this index.
    Forget(usize),
}

/// How a job that did not end well ended, in plain words; `err` is what it wrote to stderr.
fn failure(ending: JobEnding, err: &str) -> Option<Outcome> {
    Some(match ending {
        JobEnding::Exited(n) => Outcome::Failed(transfer::outcome(n, err).err()?),
        JobEnding::Stopped => Outcome::Cancelled,
        JobEnding::Signal(name) => {
            Outcome::Failed(format!("rsync was ended by the signal {name}."))
        }
        JobEnding::Closed => {
            Outcome::Failed("The server closed the transfer without saying why.".to_owned())
        }
        JobEnding::Refused(why) => Outcome::Failed(why),
    })
}

impl Panel {
    fn running(&self) -> bool {
        self.run.is_some()
    }

    fn job(&mut self) -> JobId {
        self.next_job += 1;
        JobId(self.next_job)
    }

    /// Fills the panel from `saved`, with the panes its sessions are in.
    pub fn load(&mut self, saved: &SavedTransfer, source: Option<PaneId>, target: Option<PaneId>) {
        self.source = source;
        self.target = target;
        self.source_path.clone_from(&saved.source_path);
        self.target_path.clone_from(&saved.target_path);
        self.switches = saved.switches.clone();
        self.name.clone_from(&saved.name);
        self.outcome = None;
    }

    /// The panel as a transfer to save, given its sessions' saved names.
    pub fn saved(&self, source: String, target: String) -> SavedTransfer {
        SavedTransfer {
            name: self.name.trim().to_owned(),
            source,
            source_path: self.source_path.trim().to_owned(),
            target,
            target_path: self.target_path.trim().to_owned(),
            switches: self.switches.clone(),
        }
    }

    /// Starts copying from `source` to `target`: first the check on both (R-SSH-57).
    pub fn start(&mut self, source: End, target: End) -> Vec<Order> {
        self.outcome = None;
        self.waiting = false;
        let problem = if source.pane == target.pane {
            Some("Choose two different sessions.".to_owned())
        } else if target.through_jump {
            Some(format!(
                "{} is reached through a jump host: copying through Parley SSH comes later (R-SSH-54).",
                target.label
            ))
        } else if self.source_path.trim().is_empty() || self.target_path.trim().is_empty() {
            Some("Give a path on both servers.".to_owned())
        } else {
            None
        };
        if let Some(problem) = problem {
            self.outcome = Some(Outcome::Failed(problem));
            return Vec::new();
        }
        let (a, b) = (self.job(), self.job());
        let check = |id| Exec {
            id,
            command: transfer::check_command(),
            agent: false,
        };
        let orders = vec![
            Order::Exec(source.pane, check(a)),
            Order::Exec(target.pane, check(b)),
        ];
        self.run = Some(Run {
            source,
            target,
            source_path: self.source_path.trim().to_owned(),
            target_path: self.target_path.trim().to_owned(),
            stage: Stage::Checking {
                source: (a, Output::default(), None),
                target: (b, Output::default(), None),
            },
        });
        orders
    }

    /// Copies what the dry run listed.
    pub fn confirm(&mut self) -> Vec<Order> {
        let Some(Run {
            stage: Stage::Review { copy, .. },
            ..
        }) = &mut self.run
        else {
            return Vec::new();
        };
        let command = std::mem::take(copy);
        let job = self.job();
        let Some(run) = &mut self.run else {
            return Vec::new();
        };
        run.stage = Stage::Copying {
            job,
            output: Output::default(),
            progress: None,
            cancelled: false,
        };
        vec![Order::Exec(
            run.source.pane,
            Exec {
                id: job,
                command,
                agent: true,
            },
        )]
    }

    /// Ends the copy, the dry run or the check (R-SSH-56): rsync's partial files stay for the next try.
    pub fn cancel(&mut self) -> Vec<Order> {
        self.waiting = false;
        let Some(run) = &mut self.run else {
            return Vec::new();
        };
        let (orders, outcome) = match &mut run.stage {
            Stage::Checking { source, target } => (
                vec![
                    Order::End(run.source.pane, source.0),
                    Order::End(run.target.pane, target.0),
                ],
                Some(Outcome::Cancelled),
            ),
            // A dry run changes nothing, so it need not be waited for.
            Stage::Looking { job, .. } => (
                vec![Order::End(run.source.pane, *job)],
                Some(Outcome::Cancelled),
            ),
            Stage::Review { .. } => (Vec::new(), None),
            Stage::Copying { job, cancelled, .. } => {
                *cancelled = true;
                return vec![Order::End(run.source.pane, *job)];
            }
        };
        self.run = None;
        self.outcome = outcome;
        orders
    }

    /// The session in `pane` is gone or no longer connected.
    pub fn lost(&mut self, pane: PaneId) {
        if let Some(run) = &self.run
            && (run.source.pane == pane || run.target.pane == pane)
        {
            let label = if run.source.pane == pane {
                &run.source.label
            } else {
                &run.target.label
            };
            self.outcome = Some(Outcome::Failed(format!(
                "The connection to {label} ended; copy again once it is back to go on where it stopped."
            )));
            self.run = None;
        }
    }

    /// Takes in what a job on `pane`'s connection said; returns what to do next.
    pub fn feed(&mut self, pane: PaneId, event: Event) -> Vec<Order> {
        let Some(run) = &mut self.run else {
            return Vec::new();
        };
        let from_source = pane == run.source.pane;
        match (&mut run.stage, event) {
            (Stage::Checking { source, target }, Event::JobOutput { id, stream, data }) => {
                for (job, output, _) in [source, target] {
                    if *job == id {
                        output.push(stream, &data);
                    }
                }
                Vec::new()
            }
            (Stage::Checking { source, target }, Event::JobEnded { id, ending }) => {
                for (job, output, check) in [&mut *source, &mut *target] {
                    if *job == id {
                        let status = match ending {
                            JobEnding::Exited(n) => Some(n),
                            _ => None,
                        };
                        *check = Some(transfer::check(status, &output.out));
                    }
                }
                let (Some(a), Some(b)) = (source.2.clone(), target.2.clone()) else {
                    return Vec::new();
                };
                self.checked(&a, &b)
            }
            (
                Stage::Looking {
                    job,
                    output,
                    changes,
                    ..
                },
                Event::JobOutput { id, stream, data },
            ) if *job == id && from_source => {
                match stream {
                    Stream::Out => changes.push(&data),
                    Stream::Err => output.push(stream, &data),
                }
                Vec::new()
            }
            (Stage::Looking { job, .. }, Event::JobEnded { id, ending })
                if *job == id && from_source =>
            {
                self.looked(ending);
                Vec::new()
            }
            (
                Stage::Copying {
                    job,
                    output,
                    progress,
                    ..
                },
                Event::JobOutput { id, stream, data },
            ) if *job == id && from_source => {
                output.push(stream, &data);
                if let Some(now) = transfer::last_progress(&output.out) {
                    *progress = Some(now);
                }
                Vec::new()
            }
            (Stage::Copying { job, .. }, Event::JobEnded { id, ending })
                if *job == id && from_source =>
            {
                self.copied(ending);
                Vec::new()
            }
            // Another job's news, or not a job's at all (sessions keep only jobs' events).
            _ => Vec::new(),
        }
    }

    /// The dry run has ended: its list to review, or why there is none.
    fn looked(&mut self, ending: JobEnding) {
        let Some(mut run) = self.run.take() else {
            return;
        };
        let Stage::Looking {
            output,
            mut changes,
            copy,
            ..
        } = std::mem::replace(
            &mut run.stage,
            Stage::Review {
                changes: Changes::default(),
                copy: String::new(),
            },
        )
        else {
            return;
        };
        if let Some(failed) = failure(ending, &output.err) {
            self.outcome = Some(failed);
            return;
        }
        changes.finish();
        if changes.is_empty() {
            self.outcome = Some(Outcome::Done(format!(
                "Nothing to copy: {} is up to date.",
                run.target.label
            )));
            return;
        }
        run.stage = Stage::Review { changes, copy };
        self.run = Some(run);
    }

    /// The copy has ended.
    fn copied(&mut self, ending: JobEnding) {
        let Some(Run {
            stage:
                Stage::Copying {
                    output,
                    progress,
                    cancelled,
                    ..
                },
            ..
        }) = self.run.take()
        else {
            return;
        };
        self.outcome = Some(match failure(ending, &output.err) {
            None => Outcome::Done(done_text(progress.as_ref())),
            Some(_) if cancelled => Outcome::Cancelled,
            Some(failed) => failed,
        });
    }

    /// Both checks are in: the dry run, or say what is wrong.
    fn checked(&mut self, a: &Check, b: &Check) -> Vec<Order> {
        let Some(run) = self.run.take() else {
            return Vec::new();
        };
        let (Check::Usable { path: rsync_a, .. }, Check::Usable { path: rsync_b, .. }) = (a, b)
        else {
            let problems: Vec<String> = [(a, &run.source.label), (b, &run.target.label)]
                .into_iter()
                .filter_map(|(check, label)| check.problem(label))
                .collect();
            self.outcome = Some(Outcome::Failed(problems.join("\n")));
            return Vec::new();
        };
        let direct = Direct {
            rsync: rsync_a.clone(),
            source: run.source_path.clone(),
            target: Remote {
                user: run.target.user.clone(),
                host: run.target.host.clone(),
                port: run.target.port,
                host_key: run.target.host_key.clone().unwrap_or_default(),
                rsync: rsync_b.clone(),
            },
            target_path: run.target_path.clone(),
            options: self.switches.options(),
        };
        let commands = transfer::direct_command(&direct, Mode::DryRun)
            .and_then(|dry| transfer::direct_command(&direct, Mode::Copy).map(|copy| (dry, copy)));
        let (dry, copy) = match commands {
            Ok(commands) => commands,
            Err(why) => {
                self.outcome = Some(Outcome::Failed(why));
                return Vec::new();
            }
        };
        let job = self.job();
        let order = Order::Exec(
            run.source.pane,
            Exec {
                id: job,
                command: dry,
                agent: true,
            },
        );
        self.run = Some(Run {
            stage: Stage::Looking {
                job,
                output: Output::default(),
                changes: Changes::default(),
                copy,
            },
            ..run
        });
        vec![order]
    }
}

fn done_text(progress: Option<&Progress>) -> String {
    match progress {
        Some(p) => format!("Copied {} in {}.", transfer::bytes_text(p.bytes), p.time),
        None => "Copied.".to_owned(),
    }
}

/// What a source path copies, in words (R-SSH-56): a trailing `/` copies a folder's contents.
fn copies_text(source: &str, target: &str) -> String {
    let source = source.trim();
    let target = target.trim();
    let into = if target.is_empty() {
        "the target"
    } else {
        target
    };
    if source.len() > 1 && source.ends_with('/') {
        return format!("Copies what is inside {source} into {into}.");
    }
    let name = source.rsplit('/').find(|p| !p.is_empty()).unwrap_or(source);
    if target.is_empty() {
        format!("Copies {name} itself into the target.")
    } else {
        format!(
            "Copies {name} itself into {target}, as {}/{name}.",
            target.trim_end_matches('/')
        )
    }
}

#[cfg(test)]
mod tests;
