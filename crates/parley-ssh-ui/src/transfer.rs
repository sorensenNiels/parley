//! The Transfer panel (Phase 31, R-SSH-56's first part, R-SSH-53): a source and a target, each an open session
//! and a path; Copy checks rsync on both ends (R-SSH-57), then runs rsync on the source straight to the target,
//! with progress and Cancel. Beside the panes, so the terminals stay in use while it copies.

use egui::{ComboBox, ProgressBar, TextEdit, Ui};
use parley_ssh_core::net::{Event, Exec, JobEnding, JobId, Stream};
use parley_ssh_core::transfer::{self, Check, Direct, Progress, Remote};
use parley_ui_egui::theme;

use crate::ParleySshUi;
use crate::panes::PaneId;
use crate::session::Session;

/// The most of a job's output kept: enough for rsync's last progress line and its last error lines.
const KEPT: usize = 4096;

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
    run: Option<Run>,
    pub outcome: Option<Outcome>,
    next_job: u64,
}

/// What the user asked for in the panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Request {
    Copy,
    Cancel,
    Close,
}

impl Panel {
    #[cfg(test)]
    fn running(&self) -> bool {
        self.run.is_some()
    }

    fn job(&mut self) -> JobId {
        self.next_job += 1;
        JobId(self.next_job)
    }

    /// Starts copying from `source` to `target`: first the check on both (R-SSH-57).
    pub fn start(&mut self, source: End, target: End) -> Vec<Order> {
        self.outcome = None;
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

    /// Ends the copy, or the check (R-SSH-56): rsync's partial files stay for the next try.
    pub fn cancel(&mut self) -> Vec<Order> {
        let Some(run) = &mut self.run else {
            return Vec::new();
        };
        match &mut run.stage {
            Stage::Checking { source, target } => {
                let orders = vec![
                    Order::End(run.source.pane, source.0),
                    Order::End(run.target.pane, target.0),
                ];
                self.run = None;
                self.outcome = Some(Outcome::Cancelled);
                orders
            }
            Stage::Copying { job, cancelled, .. } => {
                *cancelled = true;
                vec![Order::End(run.source.pane, *job)]
            }
        }
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
                Stage::Copying {
                    job,
                    output,
                    progress,
                    ..
                },
                Event::JobOutput { id, stream, data },
            ) if *job == id && pane == run.source.pane => {
                output.push(stream, &data);
                if let Some(now) = transfer::last_progress(&output.out) {
                    *progress = Some(now);
                }
                Vec::new()
            }
            (
                Stage::Copying {
                    job,
                    output,
                    progress,
                    cancelled,
                },
                Event::JobEnded { id, ending },
            ) if *job == id && pane == run.source.pane => {
                self.outcome = Some(match (ending, *cancelled) {
                    (JobEnding::Exited(0), _) => Outcome::Done(done_text(progress.as_ref())),
                    (_, true) | (JobEnding::Stopped, _) => Outcome::Cancelled,
                    (JobEnding::Exited(n), false) => match transfer::outcome(n, &output.err) {
                        Ok(()) => Outcome::Done(done_text(progress.as_ref())),
                        Err(why) => Outcome::Failed(why),
                    },
                    (JobEnding::Signal(name), false) => {
                        Outcome::Failed(format!("rsync was ended by the signal {name}."))
                    }
                    (JobEnding::Closed, false) => Outcome::Failed(
                        "The server closed the transfer without saying why.".to_owned(),
                    ),
                    (JobEnding::Refused(why), false) => Outcome::Failed(why),
                });
                self.run = None;
                Vec::new()
            }
            // Another job's news, or not a job's at all (sessions keep only jobs' events).
            _ => Vec::new(),
        }
    }

    /// Both checks are in: copy, or say what is wrong.
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
        };
        let command = match transfer::direct_command(&direct) {
            Ok(command) => command,
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
                command,
                agent: true,
            },
        );
        self.run = Some(Run {
            stage: Stage::Copying {
                job,
                output: Output::default(),
                progress: None,
                cancelled: false,
            },
            ..run
        });
        vec![order]
    }

    /// Draws the panel; `sessions` are the connected sessions to choose from.
    pub fn show(&mut self, ui: &mut Ui, sessions: &[(PaneId, String)]) -> Option<Request> {
        let mut request = None;
        ui.horizontal(|ui| {
            theme::section_label(ui, "Transfer");
            theme::button_row(ui, |ui| {
                if ui
                    .add(theme::ghost_button("Close"))
                    .on_hover_text("Closes the panel; a copy running is cancelled")
                    .clicked()
                {
                    request = Some(Request::Close);
                }
            });
        });
        let idle = self.run.is_none();
        ui.add_enabled_ui(idle, |ui| {
            end_row(
                ui,
                "From",
                "transfer-source",
                &mut self.source,
                &mut self.source_path,
                sessions,
            );
            end_row(
                ui,
                "To",
                "transfer-target",
                &mut self.target,
                &mut self.target_path,
                sessions,
            );
        });
        theme::hint(
            ui,
            "rsync on the first server copies straight to the second. A path ending in / copies a folder's \
             contents; without it, the folder itself.",
        );
        theme::hint(
            ui,
            "While it copies, the first server signs on to the second with your agent, which its administrator \
             could use meanwhile.",
        );
        ui.add_space(6.0);
        match &self.run {
            None => {
                let ready = self.source.is_some() && self.target.is_some();
                theme::button_row(ui, |ui| {
                    if ui
                        .add_enabled(ready, theme::primary_button(ui, "Copy"))
                        .clicked()
                    {
                        request = Some(Request::Copy);
                    }
                });
            }
            Some(run) => {
                show_stage(ui, &run.stage);
                theme::button_row(ui, |ui| {
                    if ui.button("Cancel").clicked() {
                        request = Some(Request::Cancel);
                    }
                });
            }
        }
        match &self.outcome {
            Some(Outcome::Done(text)) => theme::hint(ui, text.clone()),
            Some(Outcome::Cancelled) => {
                theme::hint(ui, "Cancelled. Copy again to go on where it stopped.");
            }
            Some(Outcome::Failed(why)) => theme::error(ui, why.clone()),
            None => {}
        }
        request
    }
}

fn done_text(progress: Option<&Progress>) -> String {
    match progress {
        Some(p) => format!("Copied {} in {}.", transfer::bytes_text(p.bytes), p.time),
        None => "Copied.".to_owned(),
    }
}

/// A session to choose and its path.
fn end_row(
    ui: &mut Ui,
    label: &str,
    id: &str,
    pane: &mut Option<PaneId>,
    path: &mut String,
    sessions: &[(PaneId, String)],
) {
    ui.label(theme::semibold(label));
    let shown = pane
        .and_then(|p| sessions.iter().find(|(id, _)| *id == p))
        .map_or("Choose a session", |(_, name)| name.as_str());
    ComboBox::from_id_salt(id)
        .width(ui.available_width())
        .selected_text(shown)
        .show_ui(ui, |ui| {
            for (p, name) in sessions {
                ui.selectable_value(pane, Some(*p), name);
            }
        });
    ui.add(
        TextEdit::singleline(path)
            .hint_text("/path/on/the/server")
            .desired_width(f32::INFINITY),
    );
    ui.add_space(4.0);
}

fn show_stage(ui: &mut Ui, stage: &Stage) {
    match stage {
        Stage::Checking { .. } => {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Checking rsync on both servers…");
            });
        }
        Stage::Copying { progress: None, .. } => {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Starting…");
            });
        }
        Stage::Copying {
            progress: Some(p), ..
        } => {
            ui.add(ProgressBar::new(f32::from(p.percent) / 100.0).show_percentage());
            theme::hint(
                ui,
                format!(
                    "{} at {}, {} left",
                    transfer::bytes_text(p.bytes),
                    p.speed,
                    p.time
                ),
            );
        }
    }
}

/// The panel's width.
const WIDTH: f32 = 320.0;

impl ParleySshUi {
    /// Opens the Transfer panel, with the session in front as the source.
    pub(crate) fn open_transfer(&mut self) {
        let front = self.tabs[self.current].active;
        let has_session = self.active_session().is_some();
        let panel = self.transfer.get_or_insert_with(Panel::default);
        if panel.source.is_none() && has_session {
            panel.source = Some(front);
        }
    }

    /// Takes in the jobs' news, carries out what the panel asks and draws it, before the panes.
    pub(crate) fn transfer_panel(&mut self, ui: &mut Ui) {
        let mut news = Vec::new();
        let mut sessions = Vec::new();
        for tab in &mut self.tabs {
            for (id, pane) in &mut tab.panes {
                if let Some(session) = pane.session_mut() {
                    news.extend(session.take_jobs().into_iter().map(|e| (*id, e)));
                    if session.is_connected() {
                        sessions.push((*id, session.target.label()));
                    }
                }
            }
        }
        let Some(panel) = &mut self.transfer else {
            return;
        };
        let mut orders = Vec::new();
        for (pane, event) in news {
            orders.extend(panel.feed(pane, event));
        }
        if let Some(run) = &panel.run {
            for pane in [run.source.pane, run.target.pane] {
                if !sessions.iter().any(|(id, _)| *id == pane) {
                    panel.lost(pane);
                }
            }
        }
        let request = egui::Panel::right("ssh-transfer")
            .exact_size(WIDTH)
            .resizable(false)
            .show(ui, |ui| panel.show(ui, &sessions))
            .inner;
        match request {
            Some(Request::Copy) => {
                let (source, target) = (panel.source, panel.target);
                let end = |pane: Option<PaneId>| {
                    pane.and_then(|p| self.session_in(p).map(|s| End::of(p, s)))
                };
                if let (Some(a), Some(b)) = (end(source), end(target))
                    && let Some(panel) = &mut self.transfer
                {
                    orders.extend(panel.start(a, b));
                }
            }
            Some(Request::Cancel) => orders.extend(panel.cancel()),
            Some(Request::Close) => {
                orders.extend(panel.cancel());
                self.transfer = None;
            }
            None => {}
        }
        for order in orders {
            match order {
                Order::Exec(pane, exec) => {
                    if let Some(session) = self.session_in(pane) {
                        session.exec(exec);
                    }
                }
                Order::End(pane, id) => {
                    if let Some(session) = self.session_in(pane) {
                        session.end_job(id);
                    }
                }
            }
        }
    }

    /// The session in `pane`, in any tab.
    fn session_in(&self, pane: PaneId) -> Option<&Session> {
        self.tabs
            .iter()
            .find_map(|tab| tab.pane(pane))
            .and_then(crate::tabs::Pane::session)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn end(pane: PaneId, host: &str) -> End {
        End {
            pane,
            label: format!("niels@{host}"),
            user: "niels".into(),
            host: host.into(),
            port: 22,
            host_key: Some("ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIAV5".into()),
            through_jump: false,
        }
    }

    fn panel() -> Panel {
        Panel {
            source: Some(1),
            source_path: "/data/æøå/".into(),
            target: Some(2),
            target_path: "/backup".into(),
            ..Panel::default()
        }
    }

    fn out(id: JobId, text: &str) -> Event {
        Event::JobOutput {
            id,
            stream: Stream::Out,
            data: text.as_bytes().to_vec(),
        }
    }

    fn ended(id: JobId, ending: JobEnding) -> Event {
        Event::JobEnded { id, ending }
    }

    const NONE: [Order; 0] = [];

    const RSYNC: &str = "rsync  version 3.2.7  protocol version 31\nparley-rsync: rsync\n";

    /// Starts and answers both checks with `a` and `b`; the orders that follow.
    fn checked(p: &mut Panel, a: &str, b: &str) -> Vec<Order> {
        let orders = p.start(end(1, "a"), end(2, "b"));
        let [Order::Exec(1, ca), Order::Exec(2, cb)] = orders.as_slice() else {
            panic!("a check on each end: {orders:?}")
        };
        assert_eq!(ca.command, transfer::check_command());
        assert!(!ca.agent && !cb.agent, "no agent for a check");
        let (ia, ib) = (ca.id, cb.id);
        assert_eq!(p.feed(1, out(ia, a)), NONE);
        assert!(
            p.feed(1, ended(ia, JobEnding::Exited(0))).is_empty(),
            "waits for the other"
        );
        p.feed(2, out(ib, b));
        p.feed(2, ended(ib, JobEnding::Exited(0)))
    }

    #[test]
    fn copy_checks_both_ends_then_runs_rsync_on_the_source_with_the_agent_r_ssh_53() {
        let mut p = panel();
        let orders = checked(&mut p, RSYNC, RSYNC);
        let [Order::Exec(1, copy)] = orders.as_slice() else {
            panic!("rsync on the source: {orders:?}")
        };
        assert!(copy.agent, "the agent for the copy (D58)");
        assert!(
            copy.command.contains("'/data/æøå/' 'niels@b:/backup'"),
            "{}",
            copy.command
        );
        let id = copy.id;
        p.feed(
            1,
            out(
                id,
                "  1,000,000  50%   2.00MB/s    0:00:01 (xfr#1, to-chk=1/2)\r",
            ),
        );
        p.feed(2, out(id, "  9,999,999  99%   2.00MB/s    0:00:01\r"));
        let Some(Run {
            stage:
                Stage::Copying {
                    progress: Some(now),
                    ..
                },
            ..
        }) = &p.run
        else {
            panic!("copying with progress")
        };
        assert_eq!(now.percent, 50, "only the source's job counts");
        p.feed(
            1,
            out(
                id,
                "  2,000,000 100%   2.00MB/s    0:00:02 (xfr#2, to-chk=0/2)\n",
            ),
        );
        p.feed(1, ended(id, JobEnding::Exited(0)));
        assert!(!p.running());
        assert_eq!(
            p.outcome,
            Some(Outcome::Done("Copied 2.0 MB in 0:00:02.".into()))
        );
    }

    #[test]
    fn an_old_rsync_stops_before_anything_is_copied_r_ssh_57() {
        let mut p = panel();
        let orders = checked(
            &mut p,
            RSYNC,
            "rsync  version 2.6.9  protocol version 29\nparley-rsync: rsync\n",
        );
        assert_eq!(orders, NONE, "nothing copied");
        assert!(!p.running());
        let Some(Outcome::Failed(why)) = &p.outcome else {
            panic!("failed")
        };
        assert!(why.starts_with("niels@b has rsync 2.6.9"), "{why}");
    }

    #[test]
    fn a_failed_copy_and_a_cancelled_one_are_said_r_ssh_56() {
        let mut p = panel();
        let orders = checked(&mut p, RSYNC, RSYNC);
        let [Order::Exec(1, copy)] = orders.as_slice() else {
            panic!("copying")
        };
        let id = copy.id;
        p.feed(
            1,
            Event::JobOutput {
                id,
                stream: Stream::Err,
                data: b"niels@b: Permission denied (publickey).\n".to_vec(),
            },
        );
        p.feed(1, ended(id, JobEnding::Exited(255)));
        assert!(
            matches!(&p.outcome, Some(Outcome::Failed(why)) if why.contains("authorized_keys"))
        );

        let orders = checked(&mut p, RSYNC, RSYNC);
        let [Order::Exec(1, copy)] = orders.as_slice() else {
            panic!("copying")
        };
        let id = copy.id;
        assert_eq!(p.cancel(), [Order::End(1, id)]);
        assert!(p.running(), "until rsync has ended");
        p.feed(1, ended(id, JobEnding::Stopped));
        assert_eq!(p.outcome, Some(Outcome::Cancelled));
    }

    #[test]
    fn what_cannot_be_copied_directly_is_said_at_once_r_ssh_53() {
        let mut p = panel();
        let mut b = end(2, "b");
        b.through_jump = true;
        assert_eq!(p.start(end(1, "a"), b), NONE);
        assert!(matches!(&p.outcome, Some(Outcome::Failed(why)) if why.contains("jump host")));
        assert!(
            p.start(end(1, "a"), end(1, "a")).is_empty(),
            "the same session twice"
        );
        p.target_path = "  ".into();
        assert_eq!(p.start(end(1, "a"), end(2, "b")), NONE, "no path");
        assert!(!p.running());
    }

    #[test]
    fn a_session_lost_while_copying_ends_the_transfer_r_ssh_56() {
        let mut p = panel();
        checked(&mut p, RSYNC, RSYNC);
        p.lost(3);
        assert!(p.running(), "another session is of no concern");
        p.lost(2);
        assert!(!p.running());
        assert!(matches!(&p.outcome, Some(Outcome::Failed(why)) if why.contains("niels@b")));
    }
}
