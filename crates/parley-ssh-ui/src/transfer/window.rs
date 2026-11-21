//! The Transfer panel in the window: its sessions, the jobs' news, saved transfers by their sessions' saved names,
//! and what the panel asks the sessions to do.

use egui::Ui;
use parley_ssh_core::net::pipe::PipeServer;
use parley_ssh_core::net::{Command, Event};

use super::local::Locals;
use super::{DESKTOP, DesktopFiles, End, Order, Panel, Request, view::Choice};
use crate::ParleySshUi;
use crate::panes::PaneId;
use crate::session::Session;

/// The panel's width.
const WIDTH: f32 = 340.0;

/// The desktop's side of transfers (R-SSH-55): its programs, the relays' port, and the file window.
#[derive(Default)]
pub struct Io {
    locals: Locals,
    pipes: Option<PipeServer>,
    pub(crate) files: Option<Box<dyn DesktopFiles>>,
}

impl std::fmt::Debug for Io {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Io")
            .field("locals", &self.locals)
            .field("pipes", &self.pipes)
            .finish_non_exhaustive()
    }
}

/// The rsync that came with Parley SSH (R-SSH-66): beside the program, or in a macOS app's Resources; else the one
/// on the `PATH`.
fn bundled_rsync() -> String {
    let name = if cfg!(windows) { "rsync.exe" } else { "rsync" };
    let Ok(exe) = std::env::current_exe() else {
        return "rsync".to_owned();
    };
    let beside = exe.with_file_name(name);
    let resources = exe
        .parent()
        .and_then(std::path::Path::parent)
        .map(|contents| contents.join("Resources").join(name));
    [Some(beside), resources]
        .into_iter()
        .flatten()
        .find(|path| path.is_file())
        .map_or_else(|| "rsync".to_owned(), |path| path.display().to_string())
}

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

    /// Runs saved transfer `i` (from the palette): its sessions are opened if they are not, and the dry run starts
    /// once both are connected.
    pub(crate) fn run_saved_transfer(&mut self, ctx: &egui::Context, i: usize) {
        self.open_transfer();
        if self.load_transfer(ctx, i)
            && let Some(panel) = &mut self.transfer
        {
            panel.waiting = true;
        }
    }

    /// Fills the panel from saved transfer `i`, opening its sessions where they are not open; false when one of
    /// them is no longer saved.
    fn load_transfer(&mut self, ctx: &egui::Context, i: usize) -> bool {
        let Some(saved) = self.settings.transfers.get(i).cloned() else {
            return false;
        };
        let source = self.pane_for(ctx, &saved.source);
        let target = self.pane_for(ctx, &saved.target);
        let Some(panel) = &mut self.transfer else {
            return false;
        };
        panel.load(
            &saved,
            source.as_ref().ok().copied(),
            target.as_ref().ok().copied(),
        );
        match (source, target) {
            (Ok(_), Ok(_)) => true,
            (Err(why), _) | (_, Err(why)) => {
                panel.outcome = Some(super::Outcome::Failed(why));
                false
            }
        }
    }

    /// The pane of the open session saved as `name`, connected ones first, else a new one connecting to it.
    fn pane_for(&mut self, ctx: &egui::Context, name: &str) -> Result<PaneId, String> {
        // A saved transfer names the desktop with no name (R-SSH-55).
        if name.is_empty() {
            return Ok(DESKTOP);
        }
        let open: Vec<(PaneId, bool)> = self
            .tabs
            .iter()
            .flat_map(|tab| tab.panes.iter())
            .filter_map(|(id, pane)| {
                pane.session()
                    .filter(|s| s.saved && s.source.name == name)
                    .map(|s| (*id, s.is_connected()))
            })
            .collect();
        if let Some((pane, _)) = open.iter().find(|(_, on)| *on).or(open.first()) {
            return Ok(*pane);
        }
        let source = self
            .settings
            .sessions
            .iter()
            .find(|s| s.name == name)
            .cloned()
            .ok_or_else(|| format!("No session is saved as {name} any more."))?;
        self.open_source(ctx, source, true);
        Ok(self.tabs[self.current].active)
    }

    /// The saved name of the session in `pane`.
    fn saved_name(&self, pane: Option<PaneId>) -> Option<String> {
        if pane == Some(DESKTOP) {
            return Some(String::new());
        }
        self.session_in(pane?)
            .filter(|s| s.saved)
            .map(|s| s.source.name.clone())
    }

    /// Takes in the jobs' news, carries out what the panel asks and draws it, before the panes.
    pub(crate) fn transfer_panel(&mut self, ui: &mut Ui) {
        let mut news = Vec::new();
        let mut sessions: Vec<Choice> = Vec::new();
        for tab in &mut self.tabs {
            for (id, pane) in &mut tab.panes {
                if let Some(session) = pane.session_mut() {
                    news.extend(session.take_jobs().into_iter().map(|e| (*id, e)));
                    sessions.push((*id, session.target.label(), session.is_connected()));
                }
            }
        }
        news.extend(
            self.transfer_io
                .locals
                .take()
                .into_iter()
                .map(|e| (DESKTOP, e)),
        );
        sessions.insert(0, (DESKTOP, "This computer".to_owned(), true));
        let piped = self
            .transfer_io
            .pipes
            .as_ref()
            .map(PipeServer::take)
            .unwrap_or_default();
        let can_choose = self.transfer_io.files.is_some();
        let Some(panel) = &mut self.transfer else {
            return;
        };
        panel.can_choose = can_choose;
        let mut orders = Vec::new();
        let mut forwards = Vec::new();
        // A relay with the token waited for runs its command on the server's connection; any other is closed by
        // being dropped (R-SSH-55).
        for relay in piped {
            if let Some((pane, id)) = panel.pipe(&relay.token) {
                forwards.push((
                    pane,
                    Command::Pipe {
                        id,
                        command: relay.command,
                        stream: relay.stream,
                    },
                ));
            }
        }
        for (pane, event) in news {
            match event {
                // The target's way in goes to the source's connection, which the panel names (R-SSH-54).
                Event::Door { id, door } => {
                    if let Some(source) = panel.door(pane, id) {
                        forwards.push((source, Command::Forward { id, door }));
                    }
                }
                event => orders.extend(panel.feed(pane, event)),
            }
        }
        let connected = |pane: PaneId| sessions.iter().any(|(id, _, on)| *id == pane && *on);
        if let Some(run) = &panel.run {
            for pane in [run.source.pane, run.target.pane] {
                if !connected(pane) {
                    orders.extend(panel.lost(pane));
                }
            }
        }
        let ready = panel.source.is_some_and(connected) && panel.target.is_some_and(connected);
        let mut request = egui::Panel::right("ssh-transfer")
            .exact_size(WIDTH)
            .resizable(false)
            .show(ui, |ui| panel.show(ui, &sessions, &self.settings.transfers))
            .inner;
        if panel.waiting && ready && !panel.running() {
            request = Some(Request::Copy);
        }
        orders.extend(self.transfer_request(ui.ctx(), request));
        for (pane, command) in forwards {
            if let Some(session) = self.session_in(pane) {
                session.send_job(command);
            }
        }
        self.carry_orders(ui.ctx(), orders);
    }

    /// Readies the desktop's side for a run: the rsync to use, Parley SSH as its remote shell, and the relays'
    /// port, listened on from the first desktop transfer on.
    fn prepare_desktop(&mut self, ctx: &egui::Context) -> Result<(), String> {
        if self.transfer_io.pipes.is_none() {
            let ctx = ctx.clone();
            let server = PipeServer::start(move || ctx.request_repaint()).map_err(|e| {
                format!("Parley SSH could not listen for this computer's rsync: {e}")
            })?;
            self.transfer_io.pipes = Some(server);
        }
        let named = self.settings.transfer_rsync.trim();
        let rsync = if named.is_empty() {
            bundled_rsync()
        } else {
            named.to_owned()
        };
        let program = std::env::current_exe()
            .map_err(|e| format!("Parley SSH could not find its own program: {e}"))?
            .display()
            .to_string();
        let port = self.transfer_io.pipes.as_ref().map_or(0, PipeServer::port);
        if let Some(panel) = &mut self.transfer {
            panel.setup = super::Setup {
                rsync,
                program,
                port,
            };
        }
        Ok(())
    }

    /// Does what the panel asked; the orders for the sessions that follow.
    fn transfer_request(&mut self, ctx: &egui::Context, request: Option<Request>) -> Vec<Order> {
        let Some(request) = request else {
            return Vec::new();
        };
        match request {
            Request::Copy => {
                let Some(panel) = &self.transfer else {
                    return Vec::new();
                };
                let (source, target) = (panel.source, panel.target);
                if (source == Some(DESKTOP) || target == Some(DESKTOP))
                    && let Err(why) = self.prepare_desktop(ctx)
                {
                    if let Some(panel) = &mut self.transfer {
                        panel.outcome = Some(super::Outcome::Failed(why));
                    }
                    return Vec::new();
                }
                let end = |pane: Option<PaneId>| match pane {
                    Some(DESKTOP) => Some(End::desktop()),
                    Some(p) => self.session_in(p).map(|s| End::of(p, s)),
                    None => None,
                };
                match ((end(source), end(target)), &mut self.transfer) {
                    ((Some(a), Some(b)), Some(panel)) => panel.start(a, b),
                    _ => Vec::new(),
                }
            }
            Request::Confirm => self
                .transfer
                .as_mut()
                .map(Panel::confirm)
                .unwrap_or_default(),
            Request::Cancel => self
                .transfer
                .as_mut()
                .map(Panel::cancel)
                .unwrap_or_default(),
            Request::TryThrough => {
                if let Some(panel) = &mut self.transfer {
                    panel.switches.route = super::Route::Through;
                }
                self.transfer_request(ctx, Some(Request::Copy))
            }
            Request::Close => {
                let orders = self
                    .transfer
                    .as_mut()
                    .map(Panel::abandon)
                    .unwrap_or_default();
                self.transfer = None;
                orders
            }
            Request::Load(i) => {
                self.load_transfer(ctx, i);
                Vec::new()
            }
            Request::Save => {
                self.save_transfer();
                Vec::new()
            }
            Request::Choose { target, folder } => {
                let chosen = self
                    .transfer_io
                    .files
                    .as_ref()
                    .and_then(|files| files.choose(folder));
                if let (Some(path), Some(panel)) = (chosen, &mut self.transfer) {
                    let path = if folder && !path.ends_with(['/', '\\']) {
                        format!("{path}/")
                    } else {
                        path
                    };
                    if target {
                        panel.target_path = path;
                    } else {
                        panel.source_path = path;
                    }
                }
                Vec::new()
            }
            Request::Forget(i) => {
                if i < self.settings.transfers.len() {
                    let gone = self.settings.transfers.remove(i);
                    if let Some(panel) = &mut self.transfer {
                        panel.outcome =
                            Some(super::Outcome::Done(format!("Forgot {}.", gone.name)));
                    }
                }
                Vec::new()
            }
        }
    }

    /// Saves the panel's transfer by its name, over one saved under that name.
    fn save_transfer(&mut self) {
        let Some(panel) = &self.transfer else {
            return;
        };
        let names = (self.saved_name(panel.source), self.saved_name(panel.target));
        let Some(panel) = &mut self.transfer else {
            return;
        };
        let (Some(source), Some(target)) = names else {
            panel.outcome = Some(super::Outcome::Failed(
                "Only a transfer between saved sessions can be saved: save both sessions first."
                    .to_owned(),
            ));
            return;
        };
        let saved = panel.saved(source, target);
        panel.outcome = Some(super::Outcome::Done(format!("Saved as {}.", saved.name)));
        let transfers = &mut self.settings.transfers;
        match transfers.iter_mut().find(|t| t.name == saved.name) {
            Some(old) => *old = saved,
            None => transfers.push(saved),
        }
    }

    fn carry_orders(&mut self, ctx: &egui::Context, orders: Vec<Order>) {
        for order in orders {
            let (pane, command) = match order {
                Order::Spawn(id, argv) => {
                    let ctx = ctx.clone();
                    self.transfer_io
                        .locals
                        .spawn(id, &argv, move || ctx.request_repaint());
                    continue;
                }
                Order::End(DESKTOP, id) => {
                    self.transfer_io.locals.end(id);
                    continue;
                }
                Order::Exec(pane, exec) => (pane, Command::Exec(exec)),
                Order::End(pane, id) => (pane, Command::EndJob(id)),
                Order::OpenDoor(pane, id) => (pane, Command::OpenDoor(id)),
                Order::Unforward(pane, id) => (pane, Command::Unforward(id)),
            };
            if let Some(session) = self.session_in(pane) {
                session.send_job(command);
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
