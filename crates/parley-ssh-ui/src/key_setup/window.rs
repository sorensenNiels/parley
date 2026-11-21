//! Key setup in the window (R-SSH-67, D63): the offer bars' answers, the dialog for the pane it was opened for, its
//! orders carried out on that pane's connection, and the saved session told to use the key.

use std::path::PathBuf;

use super::{KeySetup, Offer, Order, computer_name, remember};
use crate::ParleySshUi;
use crate::panes::PaneId;
use crate::session::Session;

impl ParleySshUi {
    fn session_at(&mut self, pane: PaneId) -> Option<&mut Session> {
        self.tabs
            .iter_mut()
            .find_map(|tab| tab.pane_mut(pane))
            .and_then(crate::tabs::Pane::session_mut)
    }

    /// The pane in front.
    fn active_pane_id(&self) -> Option<PaneId> {
        self.tabs.get(self.current).map(|tab| tab.active)
    }

    /// Session ▸ Set up key sign-on… on the pane in front.
    pub(crate) fn open_key_setup(&mut self) {
        if let Some(pane) = self.active_pane_id() {
            self.key_setup_for(pane);
        }
    }

    fn key_setup_for(&mut self, pane: PaneId) {
        let Some(session) = self.session_at(pane) else {
            return;
        };
        if !session.is_connected() {
            return;
        }
        session.end_offer();
        let server = session.target.label();
        let saved = session.saved.then(|| session.source.name.clone());
        let key_files: Vec<PathBuf> = session
            .source
            .key_files
            .iter()
            .map(|k| k.trim())
            .filter(|k| !k.is_empty())
            .map(PathBuf::from)
            .collect();
        let ssh_dir = std::env::home_dir().unwrap_or_default().join(".ssh");
        let user = ["USER", "USERNAME"]
            .iter()
            .find_map(|v| std::env::var(v).ok())
            .unwrap_or_default();
        let comment = format!("{user}@{}", computer_name());
        let (setup, orders) = KeySetup::open(pane, server, saved, &key_files, &ssh_dir, comment);
        self.key_setup = Some(setup);
        self.carry_key_orders(pane, orders);
    }

    /// Each frame: the offer bars' answers, then the dialog.
    pub(crate) fn key_setup_frame(&mut self, ctx: &egui::Context) {
        let mut answers = Vec::new();
        for tab in &mut self.tabs {
            for (id, pane) in &mut tab.panes {
                if let Some(answer) = pane.session_mut().and_then(Session::take_offer) {
                    answers.push((*id, answer));
                }
            }
        }
        for (pane, answer) in answers {
            match answer {
                Offer::SetUp => self.key_setup_for(pane),
                Offer::NotForThisSession => self.no_key_offer(pane),
            }
        }
        let Some(pane) = self.key_setup.as_ref().map(|s| s.pane) else {
            return;
        };
        let Some(session) = self.session_at(pane) else {
            // Its pane closed: so does the setup.
            self.key_setup = None;
            return;
        };
        let events = session.take_key_events();
        let mut orders = Vec::new();
        let mut height = self.key_list_height;
        if let Some(setup) = &mut self.key_setup {
            for event in events {
                orders.extend(setup.feed(event));
            }
            orders.extend(setup.show(ctx, &mut height));
        }
        self.key_list_height = height;
        self.carry_key_orders(pane, orders);
    }

    pub(crate) fn carry_key_orders(&mut self, pane: PaneId, orders: Vec<Order>) {
        for order in orders {
            match order {
                Order::Send(command) => {
                    if let Some(session) = self.session_at(pane) {
                        session.send_job(command);
                    }
                }
                Order::Remember { session: name, key } => {
                    if let Some(saved) = self.settings.sessions.iter_mut().find(|s| s.name == name)
                    {
                        remember(saved, &key);
                    }
                    if let Some(session) = self.session_at(pane) {
                        remember(&mut session.source, &key);
                    }
                }
                Order::Close => self.key_setup = None,
            }
        }
    }

    /// "Not for this session": the saved session never offers it again.
    pub(crate) fn no_key_offer(&mut self, pane: PaneId) {
        let Some(session) = self.session_at(pane) else {
            return;
        };
        session.source.key_offer_off = true;
        let name = session.saved.then(|| session.source.name.clone());
        if let Some(name) = name
            && let Some(saved) = self.settings.sessions.iter_mut().find(|s| s.name == name)
        {
            saved.key_offer_off = true;
        }
    }
}
