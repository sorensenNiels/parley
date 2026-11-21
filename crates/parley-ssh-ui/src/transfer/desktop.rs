//! The desktop as one end of a transfer (R-SSH-55): rsync runs here, with Parley SSH's relay as its remote
//! shell; the relay comes back to the window with a one-time token, and its data goes over the session's open
//! connection.

use parley_ssh_core::net::{JobId, pipe};
use parley_ssh_core::transfer::{self, Changes, Desktop, Mode};

use super::{End, Order, Outcome, Output, PaneId, Panel, Stage};

/// The desktop's place among the panes: never a real pane's number.
pub const DESKTOP: PaneId = PaneId::MAX;

/// How the window runs rsync on the desktop.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Setup {
    /// The desktop's rsync: bundled, named in the settings, or the one on the `PATH` (R-SSH-66).
    pub rsync: String,
    /// The program rsync runs as its remote shell: Parley SSH itself, in its relay mode.
    pub program: String,
    /// The window's loopback port for relays.
    pub port: u16,
}

/// Choosing desktop paths through the system's file window, which only the launcher can show.
pub trait DesktopFiles {
    /// A file or a folder the user chose, or `None` when they cancelled.
    fn choose(&self, folder: bool) -> Option<String>;
}

/// The relay a desktop rsync run waits for: its token, the server's pane, and its job once it has come.
#[derive(Debug)]
pub(super) struct Pipe {
    token: String,
    server: PaneId,
    job: Option<JobId>,
}

/// Ending the relay's job at the server, if it has come.
pub(super) fn end_pipe(pipe: Option<&Pipe>) -> Vec<Order> {
    pipe.and_then(|p| p.job.map(|job| Order::End(p.server, job)))
        .into_iter()
        .collect()
}

impl End {
    pub fn desktop() -> Self {
        Self {
            pane: DESKTOP,
            label: "this computer".to_owned(),
            user: String::new(),
            host: String::new(),
            port: 0,
            host_key: None,
            through_jump: false,
        }
    }
}

/// A desktop path as the desktop's rsync takes it.
fn rsync_path(path: &str) -> String {
    if cfg!(windows) {
        transfer::msys_path(path)
    } else {
        path.to_owned()
    }
}

impl Panel {
    /// Both checks found rsync and one end is the desktop: the dry run, rsync on the desktop.
    pub(super) fn look_desktop(&mut self) -> Vec<Order> {
        let job = self.job();
        let Some(run) = &mut self.run else {
            return Vec::new();
        };
        let upload = run.source.pane == DESKTOP;
        let (desktop_path, server_path, server_rsync) = if upload {
            (&run.source_path, &run.target_path, &run.rsync.1)
        } else {
            (&run.target_path, &run.source_path, &run.rsync.0)
        };
        run.desktop = Some(Desktop {
            relay: vec![
                rsync_path(&self.setup.program),
                "--relay".to_owned(),
                self.setup.port.to_string(),
                String::new(),
            ],
            server_rsync: server_rsync.clone(),
            desktop_path: rsync_path(desktop_path),
            server_path: server_path.clone(),
            upload,
            options: self.switches.options(),
        });
        run.runner = DESKTOP;
        match self.desktop_argv(Mode::DryRun) {
            Ok(argv) => {
                if let Some(run) = &mut self.run {
                    run.stage = Stage::Looking {
                        job,
                        output: Output::default(),
                        changes: Changes::default(),
                        copy: String::new(),
                    };
                }
                vec![Order::Spawn(job, argv)]
            }
            Err(why) => self.end(Some(Outcome::Failed(why))),
        }
    }

    /// Copies what the dry run listed, rsync on the desktop again with a new token.
    pub(super) fn confirm_desktop(&mut self) -> Vec<Order> {
        let job = self.job();
        match self.desktop_argv(Mode::Copy) {
            Ok(argv) => {
                if let Some(run) = &mut self.run {
                    run.stage = Stage::Copying {
                        job,
                        output: Output::default(),
                        progress: None,
                        cancelled: false,
                    };
                }
                vec![Order::Spawn(job, argv)]
            }
            Err(why) => self.end(Some(Outcome::Failed(why))),
        }
    }

    /// The desktop's rsync and its arguments for `mode`, with a new one-time token the relay must bring.
    fn desktop_argv(&mut self, mode: Mode) -> Result<Vec<String>, String> {
        let token = pipe::token()?;
        let rsync = self.setup.rsync.clone();
        let Some(run) = &mut self.run else {
            return Err("Nothing is being copied".to_owned());
        };
        let server = if run.source.pane == DESKTOP {
            run.target.pane
        } else {
            run.source.pane
        };
        let Some(desktop) = &mut run.desktop else {
            return Err("Nothing is being copied".to_owned());
        };
        if let Some(word) = desktop.relay.last_mut() {
            word.clone_from(&token);
        }
        let mut argv = vec![rsync];
        argv.extend(transfer::desktop_args(desktop, mode));
        run.pipe = Some(Pipe {
            token,
            server,
            job: None,
        });
        Ok(argv)
    }

    /// A relay came with `token`: the server's pane to run its command on, and the job to run it as; `None` for a
    /// token that is not the one waited for, or one used already.
    pub fn pipe(&mut self, token: &str) -> Option<(PaneId, JobId)> {
        let waiting = self.run.as_ref()?.pipe.as_ref()?;
        if waiting.job.is_some() || !pipe::same_token(token, &waiting.token) {
            return None;
        }
        let job = self.job();
        let pipe = self.run.as_mut()?.pipe.as_mut()?;
        pipe.job = Some(job);
        Some((pipe.server, job))
    }
}
