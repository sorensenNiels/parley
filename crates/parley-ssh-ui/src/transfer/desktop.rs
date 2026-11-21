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

/// A program to run on the desktop: its name and arguments, the folder to start it in, and a folder to put first on
/// its `PATH`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Launch {
    pub argv: Vec<String>,
    pub folder: Option<String>,
    pub path_first: Option<String>,
}

impl Launch {
    pub(super) fn plain(argv: Vec<String>) -> Self {
        Self {
            argv,
            ..Self::default()
        }
    }
}

/// Where the desktop's rsync starts and how it names the desktop path and the relay.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Place {
    folder: Option<String>,
    path_first: Option<String>,
    /// The desktop path as rsync gets it.
    path: String,
    /// The relay program as rsync's `-e` names it.
    program: String,
}

/// On Windows, MSYS2's rsync gets no drive letter, whatever its mount table says (R-SSH-66): it starts in the path's
/// drive and gets the rest, and finds the relay by name on its `PATH`, its folder put first. Elsewhere both stay as
/// they are.
pub(super) fn place(windows: bool, path: &str, program: &str) -> Place {
    if !windows {
        return Place {
            folder: None,
            path_first: None,
            path: path.to_owned(),
            program: program.to_owned(),
        };
    }
    let (folder, path) = transfer::windows_place(path);
    // Split on both separators: the program's path is a Windows one, whatever this runs on in tests.
    let (dir, name) = program
        .rsplit_once(['\\', '/'])
        .map_or(("", program), |(dir, name)| (dir, name));
    Place {
        folder,
        path_first: (!dir.is_empty()).then(|| dir.to_owned()),
        path,
        program: name.strip_suffix(".exe").unwrap_or(name).to_owned(),
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
        let place = place(cfg!(windows), desktop_path, &self.setup.program);
        run.desktop = Some(Desktop {
            relay: vec![
                place.program.clone(),
                "--relay".to_owned(),
                self.setup.port.to_string(),
                String::new(),
            ],
            server_rsync: server_rsync.clone(),
            desktop_path: place.path.clone(),
            server_path: server_path.clone(),
            upload,
            options: self.switches.options(),
        });
        run.place = place;
        run.runner = DESKTOP;
        match self.desktop_argv(Mode::DryRun) {
            Ok(launch) => {
                if let Some(run) = &mut self.run {
                    run.stage = Stage::Looking {
                        job,
                        output: Output::default(),
                        changes: Changes::default(),
                        copy: String::new(),
                    };
                }
                vec![Order::Spawn(job, launch)]
            }
            Err(why) => self.end(Some(Outcome::Failed(why))),
        }
    }

    /// Copies what the dry run listed, rsync on the desktop again with a new token.
    pub(super) fn confirm_desktop(&mut self) -> Vec<Order> {
        let job = self.job();
        match self.desktop_argv(Mode::Copy) {
            Ok(launch) => {
                if let Some(run) = &mut self.run {
                    run.stage = Stage::Copying {
                        job,
                        output: Output::default(),
                        progress: None,
                        cancelled: false,
                    };
                }
                vec![Order::Spawn(job, launch)]
            }
            Err(why) => self.end(Some(Outcome::Failed(why))),
        }
    }

    /// The desktop's rsync and its arguments for `mode`, with a new one-time token the relay must bring.
    fn desktop_argv(&mut self, mode: Mode) -> Result<Launch, String> {
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
        Ok(Launch {
            argv,
            folder: run.place.folder.clone(),
            path_first: run.place.path_first.clone(),
        })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn on_windows_rsync_starts_in_the_drive_and_finds_the_relay_on_its_path_r_ssh_66() {
        let p = place(
            true,
            r"C:\Users\Niels\æøå\",
            r"C:\Program Files\Parley 5250\parley-ssh.exe",
        );
        assert_eq!(p.folder.as_deref(), Some(r"C:\"));
        assert_eq!(p.path, "Users/Niels/æøå/");
        assert_eq!(p.program, "parley-ssh");
        assert_eq!(
            p.path_first.as_deref(),
            Some(r"C:\Program Files\Parley 5250")
        );

        let elsewhere = place(false, "/home/niels/æøå/", "/opt/Parley SSH/parley-ssh");
        assert_eq!(elsewhere.folder, None);
        assert_eq!(elsewhere.path_first, None);
        assert_eq!(elsewhere.path, "/home/niels/æøå/");
        assert_eq!(elsewhere.program, "/opt/Parley SSH/parley-ssh");
    }
}
