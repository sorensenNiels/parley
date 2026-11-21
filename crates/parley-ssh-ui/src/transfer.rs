//! The Transfer panel (Phase 31, R-SSH-56, R-SSH-53): a source and a target, each an open session and a path, and
//! rsync's switches. Copy checks rsync on both ends (R-SSH-57), runs a dry run that lists what will be added,
//! changed and deleted, and copies once that is confirmed: rsync on the source straight to the target, with
//! progress and Cancel; through Parley SSH when the servers cannot reach each other (R-SSH-54). Transfers are
//! saved by name with their sessions' saved names. Beside the panes, so the terminals stay in use while it copies. `view` draws the panel, `window` ties it to the window's sessions.

use parley_ssh_core::net::{Event, Exec, JobEnding, JobId, Stream};
use parley_ssh_core::transfer::{self, Changes, Check, Direct, Mode, Progress, Remote};
use serde::{Deserialize, Serialize};

use crate::panes::PaneId;
use crate::session::Session;

mod desktop;
mod local;
mod view;
mod window;

pub use desktop::{DESKTOP, DesktopFiles, Setup};
pub use window::Io;

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
    /// Straight from server to server, or through Parley SSH for servers that cannot reach each other.
    pub route: Route,
}

/// Which way a copy goes (R-SSH-53, R-SSH-54).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Route {
    /// rsync on the source reaches the target itself.
    #[default]
    Straight,
    /// The source's forwarded port is carried through the target's connection in Parley SSH.
    Through,
}

impl Default for Switches {
    fn default() -> Self {
        Self {
            delete: false,
            keep: true,
            compress: true,
            excludes: String::new(),
            route: Route::Straight,
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
    /// Ask the target's connection for a way in (R-SSH-54); the window passes it on to the source with
    /// [`Panel::door`].
    OpenDoor(PaneId, JobId),
    /// Stop the source's forwarding.
    Unforward(PaneId, JobId),
    /// Run a program on the desktop as job `JobId`: its name, then its arguments (R-SSH-55); its output and end
    /// come back as the jobs' of [`DESKTOP`], and [`Order::End`] with [`DESKTOP`] ends it.
    Spawn(JobId, Vec<String>),
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
    /// Through Parley SSH: the target's door, then the source's forwarded port (R-SSH-54).
    Relaying { job: JobId },
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
    /// Each end's rsync, as its check found it.
    rsync: (String, String),
    /// The source's forwarding while the copy goes through Parley SSH, to stop when the run ends.
    relay: Option<JobId>,
    /// Where rsync runs: the source, or the desktop when one end is the desktop.
    runner: PaneId,
    /// A copy between the desktop and a server, and the relay it waits for (R-SSH-55).
    desktop: Option<transfer::Desktop>,
    pipe: Option<desktop::Pipe>,
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
    /// The source could not reach the target: offer to go through Parley SSH (R-SSH-54).
    pub offer_through: bool,
    /// The desktop's rsync and the relay, set by the window (R-SSH-55).
    pub setup: Setup,
    /// The launcher can show the file window for desktop paths.
    pub can_choose: bool,
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
    /// Copy again through Parley SSH.
    TryThrough,
    /// Choose the source's or the target's desktop path with the file window: a file, or a folder.
    Choose {
        target: bool,
        folder: bool,
    },
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

/// `failed` for a copy through Parley SSH: a connection that failed may be the target refusing to carry it.
fn through_failure(failed: Outcome, ending: &JobEnding) -> Outcome {
    match (failed, ending) {
        (Outcome::Failed(why), JobEnding::Exited(12 | 255)) => Outcome::Failed(format!(
            "{why}\nThrough Parley SSH, the target server must allow forwarding to its own SSH port \
             (AllowTcpForwarding in its sshd_config)."
        )),
        (other, _) => other,
    }
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
        self.offer_through = false;
        let desktop = source.pane == DESKTOP || target.pane == DESKTOP;
        let problem = if source.pane == target.pane {
            Some("Choose two different ends.".to_owned())
        } else if self.source_path.trim().is_empty() || self.target_path.trim().is_empty() {
            Some("Give a path at both ends.".to_owned())
        } else if desktop && self.setup.port == 0 {
            Some("Parley SSH could not listen for this computer's rsync.".to_owned())
        } else {
            None
        };
        if let Some(problem) = problem {
            self.outcome = Some(Outcome::Failed(problem));
            return Vec::new();
        }
        let (a, b) = (self.job(), self.job());
        let rsync = self.setup.rsync.clone();
        let check = |pane, id| {
            if pane == DESKTOP {
                Order::Spawn(id, vec![rsync.clone(), "--version".to_owned()])
            } else {
                Order::Exec(
                    pane,
                    Exec {
                        id,
                        command: transfer::check_command(),
                        agent: false,
                    },
                )
            }
        };
        let orders = vec![check(source.pane, a), check(target.pane, b)];
        let runner = source.pane;
        self.run = Some(Run {
            source,
            target,
            source_path: self.source_path.trim().to_owned(),
            target_path: self.target_path.trim().to_owned(),
            rsync: (String::new(), String::new()),
            relay: None,
            runner,
            desktop: None,
            pipe: None,
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
            desktop,
            ..
        }) = &mut self.run
        else {
            return Vec::new();
        };
        if desktop.is_some() {
            return self.confirm_desktop();
        }
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
            run.runner,
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
        let (mut orders, outcome) = match &mut run.stage {
            Stage::Checking { source, target } => (
                vec![
                    Order::End(run.source.pane, source.0),
                    Order::End(run.target.pane, target.0),
                ],
                Some(Outcome::Cancelled),
            ),
            Stage::Relaying { .. } => (Vec::new(), Some(Outcome::Cancelled)),
            // A dry run changes nothing, so it need not be waited for.
            Stage::Looking { job, .. } => {
                let mut orders = vec![Order::End(run.runner, *job)];
                orders.extend(desktop::end_pipe(run.pipe.as_ref()));
                (orders, Some(Outcome::Cancelled))
            }
            Stage::Review { .. } => (Vec::new(), None),
            Stage::Copying { job, cancelled, .. } => {
                *cancelled = true;
                // A desktop copy ends at the server, so that rsync on the desktop stops by itself and keeps its
                // partial files (R-SSH-55); before its relay has come, the desktop's rsync is ended.
                let at_server = desktop::end_pipe(run.pipe.as_ref());
                return if at_server.is_empty() {
                    vec![Order::End(run.runner, *job)]
                } else {
                    at_server
                };
            }
        };
        orders.extend(self.end(outcome));
        orders
    }

    /// Ends whatever runs at once, the panel closing: the copy is ended without waiting for rsync to stop.
    pub fn abandon(&mut self) -> Vec<Order> {
        let mut orders = self.cancel();
        orders.extend(self.end(None));
        orders
    }

    /// Ends the run with `outcome`, stopping the source's forwarding if there is one.
    fn end(&mut self, outcome: Option<Outcome>) -> Vec<Order> {
        self.outcome = outcome;
        match self.run.take() {
            Some(Run {
                source,
                relay: Some(id),
                ..
            }) => vec![Order::Unforward(source.pane, id)],
            _ => Vec::new(),
        }
    }

    /// The session in `pane` is gone or no longer connected.
    pub fn lost(&mut self, pane: PaneId) -> Vec<Order> {
        let Some(run) = &self.run else {
            return Vec::new();
        };
        let label = if run.source.pane == pane {
            run.source.label.clone()
        } else if run.target.pane == pane {
            run.target.label.clone()
        } else {
            return Vec::new();
        };
        self.end(Some(Outcome::Failed(format!(
            "The connection to {label} ended; copy again once it is back to go on where it stopped."
        ))))
    }

    /// The target's connection in `pane` handed out its door for job `id`: the source's pane to forward through
    /// it, while the run is waiting for that door (R-SSH-54).
    pub fn door(&self, pane: PaneId, id: JobId) -> Option<PaneId> {
        match &self.run {
            Some(Run {
                stage: Stage::Relaying { job },
                source,
                target,
                ..
            }) if *job == id && target.pane == pane => Some(source.pane),
            _ => None,
        }
    }

    /// Takes in what a job on `pane`'s connection said; returns what to do next.
    pub fn feed(&mut self, pane: PaneId, event: Event) -> Vec<Order> {
        let Some(run) = &mut self.run else {
            return Vec::new();
        };
        let from_source = pane == run.source.pane;
        let from_runner = pane == run.runner;
        let desktop_ends = [run.source.pane == DESKTOP, run.target.pane == DESKTOP];
        let desktop_rsync = self.setup.rsync.clone();
        match (&mut run.stage, event) {
            (Stage::Relaying { job }, Event::Forwarded { id, port })
                if *job == id && from_source =>
            {
                match port {
                    Ok(port) => {
                        run.relay = Some(id);
                        self.look("127.0.0.1", port)
                    }
                    Err(why) => self.end(Some(Outcome::Failed(why))),
                }
            }
            (Stage::Checking { source, target }, Event::JobOutput { id, stream, data }) => {
                for (job, output, _) in [source, target] {
                    if *job == id {
                        output.push(stream, &data);
                    }
                }
                Vec::new()
            }
            (Stage::Checking { source, target }, Event::JobEnded { id, ending }) => {
                for ((job, output, check), on_desktop) in
                    [&mut *source, &mut *target].into_iter().zip(desktop_ends)
                {
                    if *job == id {
                        let status = match ending {
                            JobEnding::Exited(n) => Some(n),
                            _ => None,
                        };
                        *check = Some(if on_desktop {
                            // Refused: the desktop's rsync could not be started at all.
                            let answer = (!matches!(ending, JobEnding::Refused(_)))
                                .then_some((status, output.out.as_str()));
                            transfer::check_desktop(&desktop_rsync, answer)
                        } else {
                            transfer::check(status, &output.out)
                        });
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
            ) if *job == id && from_runner => {
                match stream {
                    Stream::Out => changes.push(&data),
                    Stream::Err => output.push(stream, &data),
                }
                Vec::new()
            }
            (Stage::Looking { job, .. }, Event::JobEnded { id, ending })
                if *job == id && from_runner =>
            {
                self.looked(&ending)
            }
            (
                Stage::Copying {
                    job,
                    output,
                    progress,
                    ..
                },
                Event::JobOutput { id, stream, data },
            ) if *job == id && from_runner => {
                output.push(stream, &data);
                if let Some(now) = transfer::last_progress(&output.out) {
                    *progress = Some(now);
                }
                Vec::new()
            }
            (Stage::Copying { job, .. }, Event::JobEnded { id, ending })
                if *job == id && from_runner =>
            {
                self.copied(&ending)
            }
            // Another job's news, or not a job's at all (sessions keep only jobs' events).
            _ => Vec::new(),
        }
    }

    /// The dry run has ended: its list to review, or why there is none.
    fn looked(&mut self, ending: &JobEnding) -> Vec<Order> {
        let Some(run) = &mut self.run else {
            return Vec::new();
        };
        let through = run.relay.is_some();
        let Stage::Looking {
            output,
            changes,
            copy,
            ..
        } = &mut run.stage
        else {
            return Vec::new();
        };
        if let Some(failed) = failure(ending.clone(), &output.err) {
            self.offer_through = !through
                && matches!(ending, JobEnding::Exited(n) if *n != 0)
                && transfer::unreachable(&output.err);
            let failed = if through {
                through_failure(failed, ending)
            } else {
                failed
            };
            return self.end(Some(failed));
        }
        changes.finish();
        if changes.is_empty() {
            let done = format!("Nothing to copy: {} is up to date.", run.target.label);
            return self.end(Some(Outcome::Done(done)));
        }
        run.stage = Stage::Review {
            changes: std::mem::take(changes),
            copy: std::mem::take(copy),
        };
        Vec::new()
    }

    /// The copy has ended.
    fn copied(&mut self, ending: &JobEnding) -> Vec<Order> {
        let Some(Run {
            stage:
                Stage::Copying {
                    output,
                    progress,
                    cancelled,
                    ..
                },
            relay,
            ..
        }) = &self.run
        else {
            return Vec::new();
        };
        let outcome = match failure(ending.clone(), &output.err) {
            None => Outcome::Done(done_text(progress.as_ref())),
            Some(_) if *cancelled => Outcome::Cancelled,
            Some(failed) if relay.is_some() => through_failure(failed, ending),
            Some(failed) => failed,
        };
        self.end(Some(outcome))
    }

    /// Both checks are in: the dry run, straight or through Parley SSH, or say what is wrong.
    fn checked(&mut self, a: &Check, b: &Check) -> Vec<Order> {
        let job = self.job();
        let Some(run) = &mut self.run else {
            return Vec::new();
        };
        let (Check::Usable { path: rsync_a, .. }, Check::Usable { path: rsync_b, .. }) = (a, b)
        else {
            let problems: Vec<String> = [(a, &run.source.label), (b, &run.target.label)]
                .into_iter()
                .filter_map(|(check, label)| check.problem(label))
                .collect();
            return self.end(Some(Outcome::Failed(problems.join("\n"))));
        };
        run.rsync = (rsync_a.clone(), rsync_b.clone());
        if run.source.pane == DESKTOP || run.target.pane == DESKTOP {
            return self.look_desktop();
        }
        if self.switches.route == Route::Through || run.target.through_jump {
            run.stage = Stage::Relaying { job };
            return vec![Order::OpenDoor(run.target.pane, job)];
        }
        let (host, port) = (run.target.host.clone(), run.target.port);
        self.look(&host, port)
    }

    /// Starts the dry run, rsync on the source reaching the target at `host` and `port`.
    fn look(&mut self, host: &str, port: u16) -> Vec<Order> {
        let job = self.job();
        let Some(run) = &mut self.run else {
            return Vec::new();
        };
        let direct = Direct {
            rsync: run.rsync.0.clone(),
            source: run.source_path.clone(),
            target: Remote {
                user: run.target.user.clone(),
                host: host.to_owned(),
                port,
                host_key: run.target.host_key.clone().unwrap_or_default(),
                rsync: run.rsync.1.clone(),
            },
            target_path: run.target_path.clone(),
            options: self.switches.options(),
        };
        let commands = transfer::direct_command(&direct, Mode::DryRun)
            .and_then(|dry| transfer::direct_command(&direct, Mode::Copy).map(|copy| (dry, copy)));
        let (dry, copy) = match commands {
            Ok(commands) => commands,
            Err(why) => return self.end(Some(Outcome::Failed(why))),
        };
        run.stage = Stage::Looking {
            job,
            output: Output::default(),
            changes: Changes::default(),
            copy,
        };
        vec![Order::Exec(
            run.runner,
            Exec {
                id: job,
                command: dry,
                agent: true,
            },
        )]
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
