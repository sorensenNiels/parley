//! The window's tabs and panes (R-SSH-39..41, D47): opening, splitting, moving between and closing them, the tab
//! bar, and drawing the tab in front with its dividers.

use egui::{CentralPanel, CursorIcon, Panel, Sense, Stroke, Ui, UiBuilder, ViewportCommand};
use parley_ui_egui::theme;

use super::{Closing, ParleySshUi, dialogs, start};
use crate::panes::{self, Axis, PaneId};
use crate::session::Session;
use crate::sessions::SavedSession;
use crate::tabs::{self, BarAction, Pane, Tab};
use crate::window_keys::WindowKey;

impl ParleySshUi {
    fn new_pane_id(&mut self) -> PaneId {
        self.next_pane += 1;
        self.next_pane
    }

    fn start_pane(&self) -> Pane {
        Pane::Start(start::Form::from_settings(&self.settings))
    }

    fn session_pane(&self, ctx: &egui::Context, source: SavedSession, saved: bool) -> Pane {
        let ctx = ctx.clone();
        let session = Session::open(source, saved, self.known_hosts.clone(), move || {
            ctx.request_repaint();
        });
        Pane::Session(Box::new(session))
    }

    /// A new pane for what the pane in front shows: its session again, or a start screen.
    fn like_active(&self, ctx: &egui::Context) -> Pane {
        match self.active_session() {
            Some(s) => self.session_pane(ctx, s.source.clone(), s.saved),
            None => self.start_pane(),
        }
    }

    /// Connects pane `pane` of the tab in front to `source`.
    pub(super) fn connect_pane(
        &mut self,
        ctx: &egui::Context,
        pane: PaneId,
        source: SavedSession,
        saved: bool,
    ) {
        crate::palette::remember_connection(&mut self.settings.recent, &source, saved);
        let new = self.session_pane(ctx, source, saved);
        if let Some(tab) = self.tabs.get_mut(self.current)
            && let Some(slot) = tab.pane_mut(pane)
        {
            *slot = new;
            tab.watch(pane);
        }
        self.note = None;
    }

    /// A new tab in front, with `pane` or the start screen (R-SSH-39).
    pub(super) fn new_tab(&mut self, _ctx: &egui::Context, pane: Option<Pane>) {
        let pane = pane.unwrap_or_else(|| self.start_pane());
        let id = self.new_pane_id();
        self.tabs.push(Tab::new(id, pane));
        self.current = self.tabs.len() - 1;
    }

    /// The session in front again, in a new tab (R-SSH-41).
    pub(super) fn duplicate(&mut self, ctx: &egui::Context) {
        let pane = self.like_active(ctx);
        self.new_tab(ctx, Some(pane));
    }

    /// A new pane beside (`Row`) or below (`Column`) the one in front (R-SSH-40).
    pub(super) fn split(&mut self, ctx: &egui::Context, axis: Axis) {
        let pane = self.like_active(ctx);
        let id = self.new_pane_id();
        if let Some(tab) = self.tabs.get_mut(self.current) {
            tab.split(id, pane, axis);
        }
        self.note = None;
    }

    /// Takes out the split panes whose sessions failed to start, and says why in the status bar (R-SSH-58).
    pub(super) fn drop_failed_panes(&mut self) {
        let mut dropped = false;
        for tab in &mut self.tabs {
            for why in tab.drop_failed() {
                self.note = Some(format!("The new pane was closed. {why}"));
                dropped = true;
            }
        }
        if dropped {
            self.focus_active();
        }
    }

    pub(super) fn window_key(&mut self, ctx: &egui::Context, key: WindowKey) {
        let count = self.tabs.len();
        match key {
            WindowKey::NewTab => self.new_tab(ctx, None),
            WindowKey::CloseTab => self.close_active(),
            WindowKey::NextTab => self.select((self.current + 1) % count),
            WindowKey::PreviousTab => self.select((self.current + count - 1) % count),
            WindowKey::Tab(n) if n <= count => self.select(n - 1),
            WindowKey::Tab(_) => {}
            WindowKey::SplitRight => self.split(ctx, Axis::Row),
            WindowKey::SplitDown => self.split(ctx, Axis::Column),
            WindowKey::Focus(direction) => {
                let area = self.area;
                if let Some(tab) = self.tabs.get_mut(self.current)
                    && let Some(next) = tab.layout.neighbour(area, tab.active, direction)
                {
                    tab.active = next;
                    self.focus_active();
                }
            }
            WindowKey::Duplicate => self.duplicate(ctx),
            WindowKey::NewWindow => self.spawn(&[]),
            WindowKey::Restart => self.carry_out(ctx, crate::bars::Action::Restart),
            WindowKey::FontSize(step) => self.settings.step_font_size(step),
            WindowKey::Snippets => self.open_picker(),
            WindowKey::Transfer => self.open_transfer(),
            WindowKey::Find => self.carry_out(ctx, crate::bars::Action::Find),
            WindowKey::Palette => self.palette = Some(crate::palette::Palette::default()),
            WindowKey::PreviousCommand | WindowKey::NextCommand => {
                if let Some(session) = self.active_session_mut() {
                    session.jump_command(key == WindowKey::PreviousCommand);
                }
            }
            WindowKey::Broadcast => self.toggle_broadcast_tab(),
        }
    }

    pub(super) fn select(&mut self, tab: usize) {
        if tab < self.tabs.len() {
            self.current = tab;
            self.note = None;
            self.focus_active();
        }
    }

    /// The pane in front takes the keyboard on the next frame.
    fn focus_active(&mut self) {
        if let Some(session) = self.active_session_mut() {
            session.focus_screen();
        }
    }

    /// Close (Ctrl+Shift+W): the pane in front of a split tab, else the tab; asks first while connected.
    pub(super) fn close_active(&mut self) {
        let Some(tab) = self.tabs.get(self.current) else {
            return;
        };
        let target = if tab.is_split() {
            Closing::Pane(self.current, tab.active)
        } else {
            Closing::Tab(self.current)
        };
        self.ask_or_close(target);
    }

    fn ask_or_close(&mut self, target: Closing) {
        if self.settings.confirm_close && self.connected(target) > 0 {
            self.closing = Some(target);
        } else {
            self.close(target);
        }
    }

    /// How many connected sessions closing `target` would end.
    fn connected(&self, target: Closing) -> usize {
        let count = |tab: &Tab| tab.panes.iter().filter(|(_, p)| p.connected()).count();
        match target {
            Closing::Window => self.tabs.iter().map(count).sum(),
            Closing::Tab(i) => self.tabs.get(i).map_or(0, count),
            Closing::Pane(i, pane) => usize::from(
                self.tabs
                    .get(i)
                    .and_then(|t| t.pane(pane))
                    .is_some_and(Pane::connected),
            ),
        }
    }

    fn close(&mut self, target: Closing) {
        match target {
            Closing::Pane(i, pane) => {
                if let Some(Pane::Session(session)) =
                    self.tabs.get_mut(i).and_then(|t| t.remove(pane))
                {
                    session.disconnect();
                }
                self.focus_active();
            }
            Closing::Tab(i) if i < self.tabs.len() => {
                let tab = self.tabs.remove(i);
                for (_, pane) in &tab.panes {
                    if let Some(session) = pane.session() {
                        session.disconnect();
                    }
                }
                // Closing the last tab shows the start screen (R-SSH-39).
                if self.tabs.is_empty() {
                    let id = self.new_pane_id();
                    self.tabs.push(Tab::new(id, self.start_pane()));
                }
                self.current = self.current.min(self.tabs.len() - 1);
                if self.current > i {
                    self.current -= 1;
                }
                self.focus_active();
            }
            // The window closes through the viewport (`show_closing`); a tab gone already needs nothing.
            Closing::Window | Closing::Tab(_) => {}
        }
    }

    /// Closing the window while connected asks first, when the settings say so (R-SSH-10).
    pub(super) fn guard_close(&mut self, ctx: &egui::Context) {
        if !ctx.input(|i| i.viewport().close_requested()) || self.close_confirmed {
            return;
        }
        if self.settings.confirm_close && self.connected(Closing::Window) > 0 {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            self.closing = Some(Closing::Window);
        }
    }

    pub(super) fn show_closing(&mut self, ctx: &egui::Context) {
        let Some(target) = self.closing else { return };
        let count = self.connected(target);
        let what = match (target, self.active_session()) {
            (Closing::Pane(..), Some(s)) => s.target.label(),
            _ if count == 1 => "a session".to_owned(),
            _ => format!("{count} sessions"),
        };
        let Some(close) = dialogs::confirm_close(ctx, &what) else {
            return;
        };
        self.closing = None;
        if !close {
            return;
        }
        if target == Closing::Window {
            self.close_confirmed = true;
            for tab in &self.tabs {
                for (_, pane) in &tab.panes {
                    if let Some(session) = pane.session() {
                        session.disconnect();
                    }
                }
            }
            ctx.send_viewport_cmd(ViewportCommand::Close);
        } else {
            self.close(target);
        }
    }

    /// The tab bar, from two tabs on (R-SSH-39).
    pub(super) fn tab_bar(&mut self, ui: &mut Ui) {
        if self.tabs.len() < 2 {
            return;
        }
        let frame = theme::bar(
            ui,
            egui::Margin {
                left: 6,
                right: 6,
                top: 4,
                bottom: 0,
            },
        );
        let action = Panel::top("ssh-tabs")
            .frame(frame)
            .show(ui, |ui| tabs::bar(ui, &self.tabs, self.current))
            .inner;
        match action {
            Some(BarAction::Select(i)) => self.select(i),
            Some(BarAction::Close(i)) => self.ask_or_close(Closing::Tab(i)),
            Some(BarAction::New) => self.new_tab(ui.ctx(), None),
            Some(BarAction::Move(from, to)) if from < self.tabs.len() && to < self.tabs.len() => {
                let tab = self.tabs.remove(from);
                self.tabs.insert(to, tab);
                self.current = to;
            }
            Some(BarAction::Move(..)) | None => {}
        }
    }

    /// The tab in front: its panes, each in its own part of the space, and the dividers between them (R-SSH-40).
    pub(super) fn central(&mut self, ui: &mut Ui) {
        self.connect_restored(ui.ctx());
        let look = self.settings.screen_look(ui.visuals().dark_mode);
        let palette = look.palette;
        let confirm_paste = self.settings.confirm_paste;
        let zoom = self.settings.text_size;
        let reconnect_minutes = if self.settings.reconnect {
            self.settings.reconnect_minutes.max(1)
        } else {
            0
        };
        let accent = theme::palette(ui).accent;
        let start_fill = theme::palette(ui).bg;
        let frame = egui::Frame::NONE.fill(palette.background);
        let mut choices = Vec::new();
        let mut close = None;
        let current = self.current;
        if self.tabs.get(current).is_some_and(Tab::shows_start) {
            self.openssh.refresh();
        }
        let broadcast_all = self.broadcast_all;
        let now = ui.input(|i| i.time);
        let mut shared = Vec::new();
        let lists = crate::start::Lists {
            sessions: &self.settings.sessions,
            recent: &self.settings.recent,
            openssh: &self.openssh,
            all_open: self.settings.all_sessions_open,
        };
        let Some(tab) = self.tabs.get_mut(current) else {
            return;
        };
        let broadcasting = broadcast_all || tab.broadcast;
        CentralPanel::default().frame(frame).show(ui, |ui| {
            let area = ui.max_rect();
            self.area = area;
            drag_dividers(ui, tab, area);
            let split = tab.is_split();
            let pressed = ui.input(|i| {
                i.pointer
                    .primary_pressed()
                    .then(|| i.pointer.interact_pos())
                    .flatten()
            });
            for (id, rect) in tab.layout.rects(area) {
                if pressed.is_some_and(|p| rect.contains(p)) {
                    tab.active = id;
                }
                let active = id == tab.active;
                let mut child =
                    ui.new_child(UiBuilder::new().max_rect(rect).id_salt(("ssh-pane", id)));
                child.set_clip_rect(rect);
                match tab.pane_mut(id) {
                    Some(Pane::Start(form)) => {
                        child.painter().rect_filled(rect, 0.0, start_fill);
                        if let Some(choice) = form.show(&mut child, lists, active) {
                            choices.push((id, choice));
                        }
                    }
                    Some(Pane::Session(session)) => {
                        session.view.zoom = zoom;
                        session.reconnect_minutes = reconnect_minutes;
                        session.show(&mut child, &look, confirm_paste, active);
                        // Only the pane in front is typed in; what others sent is theirs alone (R-SSH-50).
                        let sent = session.take_shared();
                        if active {
                            shared = sent;
                        }
                    }
                    None => {}
                }
                let pane = tab.pane(id);
                let marks = Marks {
                    flash: palette.foreground,
                    accent,
                    broadcasting,
                    active: split && active,
                };
                marks.draw(ui, pane, rect, id, now);
                if split && close_pane_button(ui, rect, id) {
                    close = Some(id);
                }
            }
        });
        if broadcasting {
            self.deliver(&shared);
        }
        for (pane, choice) in choices {
            self.chosen(ui.ctx(), pane, choice);
        }
        if let Some(pane) = close {
            if let Some(tab) = self.tabs.get_mut(current) {
                // The question names the session in front: this one.
                tab.active = pane;
            }
            self.ask_or_close(Closing::Pane(current, pane));
        }
    }
}

/// The close button in the top right corner of a pane of a split tab (R-SSH-58); true when clicked.
fn close_pane_button(ui: &mut Ui, pane: egui::Rect, id: PaneId) -> bool {
    let size = egui::vec2(20.0, 20.0);
    let at = egui::Rect::from_min_size(pane.right_top() + egui::vec2(-size.x - 4.0, 4.0), size);
    let button = theme::icon_button(ui, "🗙").small();
    let response = ui
        .push_id(("ssh-close-pane", id), |ui| ui.put(at, button))
        .inner
        .on_hover_text("Close pane");
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::accesskit::Role::Button, true, "Close pane")
    });
    response.clicked()
}

/// The dividers of the tab's splits, dragged to resize the panes on either side.
fn drag_dividers(ui: &Ui, tab: &mut Tab, area: egui::Rect) {
    for (i, (rect, axis, divided, share)) in tab.layout.dividers(area).into_iter().enumerate() {
        let response = ui.interact(rect, ui.scope_id().with(("ssh-divider", i)), Sense::drag());
        if response.hovered() || response.dragged() {
            ui.ctx().set_cursor_icon(match axis {
                Axis::Row => CursorIcon::ResizeHorizontal,
                Axis::Column => CursorIcon::ResizeVertical,
            });
        }
        if response.dragged()
            && let Some(pointer) = response.interact_pointer_pos()
        {
            *share = panes::share_at(divided, axis, pointer);
        }
    }
}

/// What is drawn over a pane: the bell's flash (R-SSH-19), the broadcast mark (R-SSH-50) and, in a split tab, the
/// border of the pane in front (R-SSH-40).
struct Marks {
    flash: egui::Color32,
    accent: egui::Color32,
    broadcasting: bool,
    active: bool,
}

impl Marks {
    fn draw(&self, ui: &mut Ui, pane: Option<&Pane>, rect: egui::Rect, id: PaneId, now: f64) {
        if let Some(session) = pane.and_then(Pane::session)
            && session.flash_until > now
        {
            let stroke = Stroke::new(3.0, self.flash);
            ui.painter()
                .rect_stroke(rect.shrink(1.5), 0.0, stroke, egui::StrokeKind::Inside);
            let left = std::time::Duration::from_secs_f64(session.flash_until - now);
            ui.ctx().request_repaint_after(left);
        }
        if self.broadcasting && pane.is_some_and(crate::broadcast::receives) {
            crate::broadcast::mark(ui, rect, id);
        }
        if self.active {
            let border = Stroke::new(1.5, self.accent);
            ui.painter()
                .rect_stroke(rect.shrink(0.5), 0.0, border, egui::StrokeKind::Inside);
        }
    }
}
