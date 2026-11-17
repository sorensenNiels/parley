//! Tabs (R-SSH-39, D47): each holds panes (R-SSH-40), a start screen or a session each, laid out by `panes`; and
//! the tab bar, which shows once there are two tabs.

use egui::{Color32, CornerRadius, Rect, RichText, Sense, Stroke, Ui, Vec2};
use parley_ui_egui::theme;

use crate::panes::{Layout, PaneId};
use crate::session::{Session, State};
use crate::start;

/// The longest tab title shown; the rest is cut with an ellipsis.
const TITLE_CHARS: usize = 28;

/// What a pane shows.
pub enum Pane {
    /// The start screen, to choose what this pane connects to.
    Start(start::Form),
    Session(Box<Session>),
}

impl std::fmt::Debug for Pane {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Start(_) => f.write_str("Pane::Start"),
            Self::Session(s) => write!(f, "Pane::Session({})", s.target.label()),
        }
    }
}

impl Pane {
    pub fn session(&self) -> Option<&Session> {
        match self {
            Self::Session(s) => Some(s),
            Self::Start(_) => None,
        }
    }

    pub fn session_mut(&mut self) -> Option<&mut Session> {
        match self {
            Self::Session(s) => Some(s),
            Self::Start(_) => None,
        }
    }

    /// Whether closing it would end a connection.
    pub fn connected(&self) -> bool {
        self.session().is_some_and(Session::is_connected)
    }
}

/// One tab: its panes and which one is in front.
#[derive(Debug)]
pub struct Tab {
    pub layout: Layout,
    pub panes: Vec<(PaneId, Pane)>,
    pub active: PaneId,
    /// Panes added to a split whose sessions have not started yet, each with the pane in front before it: one
    /// whose sign-on fails is taken out again (R-SSH-58).
    pending: Vec<(PaneId, Option<PaneId>)>,
    /// Keys typed in the pane in front go to the tab's other panes too (R-SSH-50).
    pub broadcast: bool,
    /// Panes restored from last time, connected when the tab is first shown (R-SSH-52).
    pub restore: Vec<(PaneId, crate::sessions::SavedSession, bool)>,
}

impl Tab {
    pub fn new(id: PaneId, pane: Pane) -> Self {
        Self {
            layout: Layout::Pane(id),
            panes: vec![(id, pane)],
            active: id,
            pending: Vec::new(),
            broadcast: false,
            restore: Vec::new(),
        }
    }

    pub fn pane(&self, id: PaneId) -> Option<&Pane> {
        self.panes.iter().find(|(i, _)| *i == id).map(|(_, p)| p)
    }

    pub fn pane_mut(&mut self, id: PaneId) -> Option<&mut Pane> {
        self.panes
            .iter_mut()
            .find(|(i, _)| *i == id)
            .map(|(_, p)| p)
    }

    pub fn active_pane(&self) -> Option<&Pane> {
        self.pane(self.active)
    }

    pub fn active_pane_mut(&mut self) -> Option<&mut Pane> {
        let active = self.active;
        self.pane_mut(active)
    }

    /// One of its panes is a start screen.
    pub fn shows_start(&self) -> bool {
        self.panes.iter().any(|(_, p)| matches!(p, Pane::Start(_)))
    }

    pub fn is_split(&self) -> bool {
        self.panes.len() > 1
    }

    /// Adds `pane` beside or below the one in front, and brings it to the front.
    pub fn split(&mut self, id: PaneId, pane: Pane, axis: crate::panes::Axis) {
        let from = self.active;
        if self.layout.split(from, id, axis) {
            if matches!(pane, Pane::Session(_)) {
                self.pending.push((id, Some(from)));
            }
            self.panes.push((id, pane));
            self.active = id;
        }
    }

    /// Takes the pane `id` out; the first pane left comes to the front. `None` when it is the last one.
    pub fn remove(&mut self, id: PaneId) -> Option<Pane> {
        if !self.layout.remove(id) {
            return None;
        }
        let index = self.panes.iter().position(|(i, _)| *i == id)?;
        let (_, pane) = self.panes.remove(index);
        if self.active == id {
            self.active = self.layout.panes().first().copied().unwrap_or(id);
        }
        Some(pane)
    }

    /// Watches the session just connected in pane `id` of a split tab: it is taken out if it fails to start
    /// (R-SSH-58). A tab's only pane stays, to show why.
    pub fn watch(&mut self, id: PaneId) {
        if self.is_split() && !self.pending.iter().any(|(p, _)| *p == id) {
            self.pending.push((id, None));
        }
    }

    /// Takes out the watched panes whose sessions failed to start (R-SSH-58), bringing back the pane that was in
    /// front before each; returns why each one failed.
    pub fn drop_failed(&mut self) -> Vec<String> {
        let mut why = Vec::new();
        for (id, from) in std::mem::take(&mut self.pending) {
            // A pane closed meanwhile, or no longer a session, needs no more watching.
            let Some(session) = self.pane_mut(id).and_then(Pane::session_mut) else {
                continue;
            };
            session.poll();
            if !session.failed_to_start() {
                if !session.is_started() && session.is_connected() {
                    self.pending.push((id, from));
                }
                continue;
            }
            let reason = match &session.state {
                State::Ended(ending) => crate::session::ending_text(ending),
                State::Connecting | State::SigningOn | State::Running => String::new(),
            };
            if self.remove(id).is_some() {
                if let Some(from) = from.filter(|f| self.pane(*f).is_some()) {
                    self.active = from;
                }
                why.push(reason);
            }
        }
        why
    }

    /// The title: what the host set in the pane in front, else `user@host`, else "New tab".
    pub fn title(&self) -> String {
        match self.active_pane().and_then(Pane::session) {
            Some(session) => {
                let host_title = session.host_title();
                if host_title.is_empty() {
                    session.target.label()
                } else {
                    host_title
                }
            }
            None => "New tab".to_owned(),
        }
    }

    /// The colour of the session in the pane in front (R-SSH-61).
    pub fn color(&self) -> parley_ui_egui::SessionColor {
        self.active_pane()
            .and_then(Pane::session)
            .map(|s| s.source.color)
            .unwrap_or_default()
    }

    /// The state of the pane in front, for the dot.
    fn state(&self) -> Option<&State> {
        self.active_pane().and_then(Pane::session).map(|s| &s.state)
    }
}

/// What was done on the tab bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BarAction {
    Select(usize),
    Close(usize),
    New,
    /// Move the tab at the first index to the second.
    Move(usize, usize),
}

/// The tab bar: each tab with its dot, title and close button; a middle click closes; a tab is dragged along the
/// bar to move it; a plus at the end opens a new tab.
pub fn bar(ui: &mut Ui, tabs: &[Tab], current: usize) -> Option<BarAction> {
    let p = theme::palette(ui);
    let mut action = None;
    let mut rects: Vec<Rect> = Vec::with_capacity(tabs.len());
    let mut dragging = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        for (i, tab) in tabs.iter().enumerate() {
            let front = i == current;
            let frame = egui::Frame::NONE
                .inner_margin(egui::Margin::symmetric(10, 5))
                .corner_radius(CornerRadius {
                    nw: 6,
                    ne: 6,
                    sw: 0,
                    se: 0,
                })
                .fill(if front { p.bg } else { Color32::TRANSPARENT });
            // The tab senses clicks and drags behind what it holds, so its close button stays on top and gets
            // its own clicks (a sense added after the button would take them).
            let tab_ui = egui::UiBuilder::new()
                .id_salt(("ssh-tab", i))
                .sense(Sense::click_and_drag());
            let shown = ui.scope_builder(tab_ui, |ui| {
                frame
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let (dot, _) = ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
                            ui.painter().circle_filled(
                                dot.center(),
                                3.5,
                                dot_color(ui, tab.state()),
                            );
                            let title = cut(&tab.title());
                            let text = if front {
                                theme::semibold(title)
                            } else {
                                RichText::new(title).color(p.muted)
                            };
                            ui.label(text);
                            let close = ui
                                .add(theme::icon_button(ui, "🗙").small())
                                .on_hover_text("Close tab");
                            close.widget_info(|| {
                                egui::WidgetInfo::labeled(
                                    egui::accesskit::Role::Button,
                                    true,
                                    "Close tab",
                                )
                            });
                            close.clicked()
                        })
                        .inner
                    })
                    .inner
            });
            let response = shown.response;
            let rect = response.rect;
            let color = theme::session_color(ui, tab.color());
            parley_ui_egui::session_color::tab_stripe(ui, rect, color);
            if front {
                ui.painter().hline(
                    rect.x_range(),
                    rect.bottom() - 1.0,
                    Stroke::new(2.0, p.accent),
                );
            }
            if shown.inner || response.middle_clicked() {
                action = Some(BarAction::Close(i));
            } else if response.clicked() || response.drag_started() {
                action = action.or(Some(BarAction::Select(i)));
            }
            if response.dragged() {
                dragging = Some(i);
            }
            rects.push(rect);
        }
        if ui
            .add(theme::icon_button(ui, "+"))
            .on_hover_text("New tab")
            .clicked()
        {
            action = Some(BarAction::New);
        }
    });
    if let (Some(from), Some(pointer)) = (dragging, ui.ctx().pointer_interact_pos())
        && let Some(to) = rects.iter().position(|r| r.x_range().contains(pointer.x))
        && to != from
    {
        action = Some(BarAction::Move(from, to));
    }
    action
}

fn dot_color(ui: &Ui, state: Option<&State>) -> Color32 {
    let p = theme::palette(ui);
    match state {
        Some(State::Running) => p.ok,
        Some(State::Connecting | State::SigningOn) => p.warning_text,
        Some(State::Ended(_)) | None => p.muted,
    }
}

/// `title` cut to [`TITLE_CHARS`] characters.
fn cut(title: &str) -> String {
    if title.chars().count() <= TITLE_CHARS {
        return title.to_owned();
    }
    let mut cut: String = title.chars().take(TITLE_CHARS - 1).collect();
    cut.push('…');
    cut
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::panes::Axis;
    use crate::settings::Settings;

    fn start() -> Pane {
        Pane::Start(start::Form::from_settings(&Settings::default()))
    }

    #[test]
    fn a_tab_splits_and_closes_its_panes_r_ssh_40() {
        let mut tab = Tab::new(1, start());
        assert!(!tab.is_split());
        tab.split(2, start(), Axis::Row);
        assert_eq!(
            (tab.active, tab.panes.len()),
            (2, 2),
            "the new pane is in front"
        );
        tab.split(3, start(), Axis::Column);
        assert!(tab.remove(3).is_some());
        assert_eq!(tab.active, 1, "the first pane left comes to the front");
        assert!(tab.remove(2).is_some());
        assert!(tab.remove(1).is_none(), "the last pane stays");
        assert_eq!(tab.title(), "New tab");
    }

    #[test]
    fn long_titles_are_cut() {
        assert_eq!(cut("me@box"), "me@box");
        let long = "æ".repeat(40);
        assert_eq!(cut(&long).chars().count(), TITLE_CHARS);
        assert!(cut(&long).ends_with('…'));
    }
}
