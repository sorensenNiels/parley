//! Broadcast (R-SSH-50): keys and pastes typed in the pane in front also go to the other connected panes of its
//! tab, or of every tab; each pane that gets them is marked. And sending snippets (R-SSH-49), which broadcast
//! too.

use egui::{Stroke, Ui};
use parley_ui_egui::{icons, theme};

use super::ParleySshUi;
use crate::session::Shared;
use crate::snippets::{Fill, Picked, Picker};
use crate::tabs::{Pane, Tab};

/// Two Escapes this close together (seconds) turn broadcast off.
const DOUBLE_ESCAPE: f64 = 0.6;

/// Where broadcast goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Off,
    /// The panes of the tab in front.
    Tab,
    /// Every pane of every tab.
    All,
}

impl ParleySshUi {
    /// Where keys typed in the pane in front go besides it.
    pub(super) fn broadcast_scope(&self) -> Scope {
        if self.broadcast_all {
            Scope::All
        } else if self.tabs.get(self.current).is_some_and(|t| t.broadcast) {
            Scope::Tab
        } else {
            Scope::Off
        }
    }

    /// Session ▸ Broadcast to this tab (Ctrl+Shift+B): on, or off again.
    pub(super) fn toggle_broadcast_tab(&mut self) {
        if self.broadcast_scope() == Scope::Off {
            if let Some(tab) = self.tabs.get_mut(self.current) {
                tab.broadcast = true;
            }
        } else {
            self.stop_broadcast();
        }
    }

    /// Session ▸ Broadcast to every tab: on, or off again.
    pub(super) fn toggle_broadcast_all(&mut self) {
        if self.broadcast_scope() == Scope::Off {
            self.broadcast_all = true;
        } else {
            self.stop_broadcast();
        }
    }

    pub(super) fn stop_broadcast(&mut self) {
        self.broadcast_all = false;
        for tab in &mut self.tabs {
            tab.broadcast = false;
        }
    }

    /// Escape twice quickly turns broadcast off; each Escape still goes to the hosts.
    pub(super) fn watch_double_escape(&mut self, ui: &Ui) {
        if self.broadcast_scope() == Scope::Off {
            return;
        }
        let (escape, now) = ui.input(|i| (i.key_pressed(egui::Key::Escape), i.time));
        if !escape {
            return;
        }
        match self.last_escape {
            Some(before) if now - before <= DOUBLE_ESCAPE => {
                self.stop_broadcast();
                self.last_escape = None;
            }
            _ => self.last_escape = Some(now),
        }
    }

    /// How many connected panes get what is typed, the one in front included; 0 while broadcast is off.
    pub(super) fn receivers(&self) -> usize {
        match self.broadcast_scope() {
            Scope::Off => 0,
            Scope::Tab => self.tabs.get(self.current).map_or(0, connected_panes),
            Scope::All => self.tabs.iter().map(connected_panes).sum(),
        }
    }

    /// Gives what the pane in front sent to the other panes in scope.
    pub(super) fn deliver(&mut self, shared: &[Shared]) {
        if shared.is_empty() {
            return;
        }
        let scope = self.broadcast_scope();
        let current = self.current;
        for (index, tab) in self.tabs.iter_mut().enumerate() {
            let in_scope = match scope {
                Scope::Off => false,
                Scope::Tab => index == current,
                Scope::All => true,
            };
            if !in_scope {
                continue;
            }
            let front = (index == current).then_some(tab.active);
            for (id, pane) in &mut tab.panes {
                if Some(*id) == front {
                    continue;
                }
                if let Some(session) = pane.session_mut() {
                    for s in shared {
                        session.receive(s);
                    }
                }
            }
        }
    }

    /// Sends the snippet with this index: asks for its placeholders first.
    pub(super) fn send_snippet(&mut self, index: usize) {
        let Some(snippet) = self.settings.snippets.get(index).cloned() else {
            return;
        };
        match Fill::new(snippet.clone()) {
            Some(fill) => self.fill = Some(fill),
            None => self.send_snippet_text(&snippet.text, snippet.enter),
        }
    }

    /// The text to the pane in front, as a paste, then Enter if wanted; and to every pane in scope.
    fn send_snippet_text(&mut self, text: &str, enter: bool) {
        let mut shared = vec![Shared::Paste(text.to_owned())];
        if enter {
            shared.push(Shared::Keys(b"\r".to_vec()));
        }
        let Some(session) = self.active_session_mut() else {
            self.note = Some("A snippet goes to a connected session: connect first.".to_owned());
            return;
        };
        for s in &shared {
            session.receive(s);
        }
        session.focus_screen();
        self.deliver(&shared);
    }

    /// Opens the snippet picker (Ctrl+Shift+S).
    pub(super) fn open_picker(&mut self) {
        self.picker = Some(Picker::default());
    }

    /// The picker and the placeholder question, when open.
    pub(super) fn snippet_dialogs(&mut self, ctx: &egui::Context) {
        if let Some(picker) = &mut self.picker
            && let Some(picked) = picker.show(ctx, &self.settings.snippets)
        {
            self.picker = None;
            match picked {
                Picked::Snippet(i) => self.send_snippet(i),
                Picked::Cancel => {
                    if let Some(session) = self.active_session_mut() {
                        session.focus_screen();
                    }
                }
            }
        }
        if let Some(fill) = &mut self.fill
            && let Some(answer) = fill.show(ctx)
        {
            let enter = fill.snippet.enter;
            self.fill = None;
            match answer {
                Some(text) => self.send_snippet_text(&text, enter),
                None => {
                    if let Some(session) = self.active_session_mut() {
                        session.focus_screen();
                    }
                }
            }
        }
    }

    /// The snippet whose own key was pressed, taken from the input (R-SSH-49).
    pub(super) fn snippet_key(&self, ctx: &egui::Context) -> Option<usize> {
        let keys: Vec<(usize, parley_ui_egui::keymap::Chord)> = self
            .settings
            .snippets
            .iter()
            .enumerate()
            .filter_map(|(i, s)| s.chord().map(|c| (i, c)))
            .collect();
        if keys.is_empty() {
            return None;
        }
        ctx.input_mut(|input| {
            let mut found = None;
            input.events.retain(|event| {
                let egui::Event::Key {
                    key,
                    pressed,
                    modifiers,
                    ..
                } = event
                else {
                    return true;
                };
                let chord = parley_ui_egui::keymap::Chord::exact(*key, *modifiers);
                match keys.iter().find(|(_, c)| *c == chord) {
                    Some((i, _)) => {
                        if *pressed {
                            found = Some(*i);
                        }
                        false
                    }
                    None => true,
                }
            });
            found
        })
    }
}

/// The connected panes of `tab`.
fn connected_panes(tab: &Tab) -> usize {
    tab.panes.iter().filter(|(_, p)| p.connected()).count()
}

/// The mark on a pane that gets broadcast keys: a border and a badge.
pub fn mark(ui: &mut Ui, rect: egui::Rect, id: crate::panes::PaneId) {
    let colour = theme::palette(ui).warning_text;
    ui.painter().rect_stroke(
        rect.shrink(1.0),
        0.0,
        Stroke::new(2.0, colour),
        egui::StrokeKind::Inside,
    );
    let at = egui::Rect::from_min_size(
        rect.left_top() + egui::vec2(8.0, 6.0),
        egui::vec2(110.0, 20.0),
    );
    ui.push_id(("ssh-broadcast-mark", id), |ui| {
        ui.put(at, |ui: &mut Ui| {
            theme::icon_badge(ui, icons::LIGHTNING, "Broadcast", colour)
        });
    });
}

/// Whether a pane shows a session (only those get broadcast).
pub const fn receives(pane: &Pane) -> bool {
    matches!(pane, Pane::Session(_))
}
