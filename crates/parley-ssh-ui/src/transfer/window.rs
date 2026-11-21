//! The Transfer panel in the window: its sessions, the jobs' news, saved transfers by their sessions' saved names,
//! and what the panel asks the sessions to do.

use egui::Ui;
use parley_ssh_core::net::{Command, Event};

use super::{End, Order, Panel, Request, view::Choice};
use crate::ParleySshUi;
use crate::panes::PaneId;
use crate::session::Session;

/// The panel's width.
const WIDTH: f32 = 340.0;

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
        let Some(panel) = &mut self.transfer else {
            return;
        };
        let mut orders = Vec::new();
        let mut forwards = Vec::new();
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
        self.carry_orders(orders);
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
                let end = |pane: Option<PaneId>| {
                    pane.and_then(|p| self.session_in(p).map(|s| End::of(p, s)))
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

    fn carry_orders(&self, orders: Vec<Order>) {
        for order in orders {
            let (pane, command) = match order {
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
