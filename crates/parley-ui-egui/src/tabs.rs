//! More sessions at once (Phase 40, D53): sessions as tabs (R-UI-33), where more sessions open (R-UI-34), a
//! duplicate of the session (R-UI-30) and a group's sessions (R-UI-32).
//!
//! The session in front keeps its state in [`ParleyUi`]'s own fields, as when there was only one; every other
//! tab keeps its state in a [`Tab`], and choosing a tab swaps the two.

use egui::{Frame, Panel, RichText, Sense, Ui, ViewportCommand};
use parley_core::net::Connection;

use crate::keymap::KeyInput;
use crate::settings::MoreSessions;
use crate::{ParleyUi, Profile, dialog, reconnect, status, terminal, theme};

/// A session's own state (R-UI-33): in a tab behind the one in front, or a placeholder in the front tab's slot.
#[derive(Debug, Default)]
pub(crate) struct Tab {
    current: Option<Profile>,
    connection: Option<Connection>,
    online: bool,
    reconnect: Option<reconnect::Reconnect>,
    closed: Option<String>,
    view: terminal::View,
    clock: status::Clock,
    key_input: KeyInput,
    focus_terminal: bool,
}

/// A session to open beside this one (R-UI-34): in a tab, or in another Parley window, which the launcher
/// starts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewWindow {
    pub profile: Profile,
    /// A saved session, opened by its name; otherwise by its host.
    pub saved: bool,
    /// A duplicate (R-UI-30): it asks for no device name, so the IBM i gives it one of its own.
    pub duplicate: bool,
}

/// How the launcher opens another Parley window on a session (R-UI-34); `Err` says why it could not.
pub type OpenWindow = Box<dyn Fn(&NewWindow) -> Result<(), String>>;

pub(crate) struct WindowOpener(pub(crate) OpenWindow);

impl std::fmt::Debug for WindowOpener {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("WindowOpener")
    }
}

/// What the tab bar or Ctrl+Tab asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TabAction {
    Show(usize),
    Close(usize),
}

/// The dot on a tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mark {
    None,
    /// The host's message light is on.
    Message,
    /// The connection ended.
    Lost,
}

impl Mark {
    /// A session's dot: `connected` with the message light on, or a session that was open and ended.
    fn of(connected: bool, waiting: bool, current: Option<&Profile>, closed: bool) -> Self {
        if connected && waiting {
            Self::Message
        } else if !connected && current.is_some() && closed {
            Self::Lost
        } else {
            Self::None
        }
    }
}

/// The front session's fields and `tab`'s change places.
fn swap_front(ui: &mut ParleyUi, tab: &mut Tab) {
    std::mem::swap(&mut ui.current, &mut tab.current);
    std::mem::swap(&mut ui.connection, &mut tab.connection);
    std::mem::swap(&mut ui.online, &mut tab.online);
    std::mem::swap(&mut ui.reconnect, &mut tab.reconnect);
    std::mem::swap(&mut ui.closed, &mut tab.closed);
    std::mem::swap(&mut ui.view, &mut tab.view);
    std::mem::swap(&mut ui.clock, &mut tab.clock);
    std::mem::swap(&mut ui.key_input, &mut tab.key_input);
    std::mem::swap(&mut ui.focus_terminal, &mut tab.focus_terminal);
}

/// Session tabs (R-UI-33).
impl ParleyUi {
    /// The front session's fields and tab `i`'s change places.
    fn swap_with(&mut self, i: usize) {
        if let Some(slot) = self.tabs.get_mut(i) {
            let mut tab = std::mem::take(slot);
            swap_front(self, &mut tab);
            self.tabs[i] = tab;
        }
    }

    /// Brings tab `i` to the front.
    pub(crate) fn switch_to(&mut self, i: usize) {
        if i == self.active || i >= self.tabs.len() {
            return;
        }
        self.swap_with(self.active);
        self.swap_with(i);
        self.active = i;
        self.focus_terminal = true;
    }

    /// Opens `profile` in a new tab in front.
    fn open_tab(&mut self, ctx: &egui::Context, profile: Profile) {
        self.swap_with(self.active);
        self.tabs.push(Tab::default());
        self.active = self.tabs.len() - 1;
        self.connect(ctx, profile);
    }

    /// Closes the session in front; its tab goes, unless it is the only one, which shows the start screen.
    fn close_front(&mut self) {
        if let Some(connection) = self.connection.take() {
            self.end_recording(&mut connection.session());
            connection.close();
        }
        self.reconnect = None;
        if self.tabs.len() <= 1 {
            self.closed = Some("Disconnected.".to_owned());
            return;
        }
        self.tabs.remove(self.active);
        self.active = self.active.min(self.tabs.len() - 1);
        let mut next = std::mem::take(&mut self.tabs[self.active]);
        swap_front(self, &mut next);
        self.focus_terminal = true;
    }

    /// The session in front ended and asks to close (the host ended it, R-UI-5): its tab closes, or the window
    /// with the last one.
    pub(crate) fn session_over(&mut self, ctx: &egui::Context) {
        if self.tabs.len() <= 1 {
            self.exit_confirmed = true;
            ctx.send_viewport_cmd(ViewportCommand::Close);
        } else {
            self.close_front();
        }
    }

    /// The tabs behind the front one notice their connections coming up, ending or due again (R-NET-10); one
    /// whose host ended it and that asks to close, closes.
    pub(crate) fn follow_background(&mut self, ctx: &egui::Context) {
        let mut ended = Vec::new();
        for i in 0..self.tabs.len() {
            if i == self.active {
                continue;
            }
            self.swap_with(i);
            if self.follow_link(ctx) {
                ended.push(i);
            }
            self.swap_with(i);
        }
        for i in ended.into_iter().rev() {
            self.tabs.remove(i);
            if i < self.active {
                self.active -= 1;
            }
        }
    }

    /// The tab bar under the header, with two sessions or more; what was clicked in it.
    pub(crate) fn tab_panel(
        &self,
        ui: &mut Ui,
        frame: Frame,
        front: Option<&parley_core::Session>,
    ) -> Option<TabAction> {
        if self.tabs.len() < 2 {
            return None;
        }
        let labels: Vec<(String, Mark)> = (0..self.tabs.len())
            .map(|i| {
                if i == self.active {
                    // During the frame the front connection is out of its field: `front` is its session.
                    let waiting = front.is_some_and(parley_core::Session::message_waiting);
                    let mark = Mark::of(
                        front.is_some(),
                        waiting,
                        self.current.as_ref(),
                        self.closed.is_some(),
                    );
                    (tab_name(self.current.as_ref()), mark)
                } else {
                    let tab = &self.tabs[i];
                    let waiting = tab
                        .connection
                        .as_ref()
                        .is_some_and(|c| c.session().message_waiting());
                    let mark = Mark::of(
                        tab.connection.is_some(),
                        waiting,
                        tab.current.as_ref(),
                        tab.closed.is_some(),
                    );
                    (tab_name(tab.current.as_ref()), mark)
                }
            })
            .collect();
        Panel::top("tab_bar")
            .frame(frame)
            .show(ui, |ui| tab_bar(ui, &labels, self.active))
            .inner
    }

    /// Ctrl+Tab and Ctrl+Shift+Tab, with two tabs or more, before the screen takes the keys (R-UI-33).
    pub(crate) fn tab_keys(&self, ctx: &egui::Context) -> Option<TabAction> {
        let count = self.tabs.len();
        if count < 2 {
            return None;
        }
        let (back, forward) = ctx.input_mut(|i| {
            let shift = egui::Modifiers::CTRL | egui::Modifiers::SHIFT;
            let back = i.consume_key(shift, egui::Key::Tab);
            (
                back,
                !back && i.consume_key(egui::Modifiers::CTRL, egui::Key::Tab),
            )
        });
        if back {
            Some(TabAction::Show((self.active + count - 1) % count))
        } else if forward {
            Some(TabAction::Show((self.active + 1) % count))
        } else {
            None
        }
    }

    /// Does what the tab bar asked, once the frame is done with the front connection: a connected session that
    /// asks before closing is asked about first.
    pub(crate) fn tab_action(&mut self, action: TabAction) {
        match action {
            TabAction::Show(i) => self.switch_to(i),
            TabAction::Close(i) => {
                let asks = if i == self.active {
                    self.connection.is_some()
                        && self.current.as_ref().is_some_and(|p| p.confirm_exit)
                } else {
                    self.tabs.get(i).is_some_and(|t| {
                        t.connection.is_some() && t.current.as_ref().is_some_and(|p| p.confirm_exit)
                    })
                };
                if asks {
                    self.closing_tab = Some(i);
                } else {
                    self.close_tab(i);
                }
            }
        }
    }

    fn close_tab(&mut self, i: usize) {
        self.switch_to(i);
        self.close_front();
    }

    /// "Close the session?" for a tab that asks first.
    pub(crate) fn show_close_tab_question(&mut self, ctx: &egui::Context) {
        let Some(i) = self.closing_tab else {
            return;
        };
        let name = if i == self.active {
            tab_name(self.current.as_ref())
        } else {
            tab_name(self.tabs.get(i).and_then(|t| t.current.as_ref()))
        };
        let (close, (cancel, yes)) = dialog::dialog(
            ctx,
            "confirm_close_tab",
            "Close the session?",
            380.0,
            |ui| {
                ui.label(format!("The session \"{name}\" is still connected."));
                dialog::actions(ui, theme::primary_button(ui, "Close session"), true)
            },
        );
        if close || cancel {
            self.closing_tab = None;
        }
        if yes {
            self.closing_tab = None;
            self.close_tab(i);
        }
    }

    /// The names of the connected sessions, in every tab, that ask before closing.
    pub(crate) fn sessions_asking_to_exit(&self) -> Vec<String> {
        let front = (self.connection.is_some())
            .then_some(self.current.as_ref())
            .flatten();
        let back = self
            .tabs
            .iter()
            .filter(|t| t.connection.is_some())
            .filter_map(|t| t.current.as_ref());
        front
            .into_iter()
            .chain(back)
            .filter(|p| p.confirm_exit)
            .map(|p| p.name.clone())
            .collect()
    }
}

/// More sessions: a duplicate (R-UI-30) and groups (R-UI-32), opened where the settings say (R-UI-34).
impl ParleyUi {
    /// How the launcher opens another Parley window (R-UI-34).
    pub fn set_window_opener(&mut self, open: OpenWindow) {
        self.window_opener = Some(WindowOpener(open));
    }

    /// The group marked "start when Parley starts", if any (R-UI-32).
    pub fn startup_group(&self) -> Option<usize> {
        self.settings
            .session_groups
            .iter()
            .position(|g| g.start_on_launch)
    }

    /// Starts every session of group `index` (R-UI-32).
    pub fn start_group(&mut self, ctx: &egui::Context, index: usize) {
        let openings = self.group_openings(index);
        self.open_more(ctx, openings);
    }

    /// Group `index`'s saved sessions, to open.
    pub(crate) fn group_openings(&self, index: usize) -> Vec<NewWindow> {
        self.settings
            .session_groups
            .get(index)
            .map(|g| g.profiles(&self.profiles))
            .unwrap_or_default()
            .into_iter()
            .map(|p| NewWindow {
                profile: p.clone(),
                saved: true,
                duplicate: false,
            })
            .collect()
    }

    /// The session in use again, without its device name (R-UI-30, D53); none while not connected.
    /// `connected` comes from the caller: during a frame the connection is out of its field.
    pub(crate) fn duplicate_of(&self, connected: bool) -> Option<NewWindow> {
        if !connected {
            return None;
        }
        let mut profile = self.current.clone()?;
        profile.config.device_name.clear();
        Some(NewWindow {
            saved: self.current_index().is_some(),
            profile,
            duplicate: true,
        })
    }

    /// Opens `openings` where the settings say (R-UI-34); the first takes this window's front tab while it has
    /// no session open.
    pub(crate) fn open_more(&mut self, ctx: &egui::Context, openings: Vec<NewWindow>) {
        for (n, opening) in openings.into_iter().enumerate() {
            let idle = self.connection.is_none() && self.reconnect.is_none();
            if n == 0 && idle {
                self.connect(ctx, opening.profile);
                continue;
            }
            match self.settings.more_sessions {
                MoreSessions::Tab => self.open_tab(ctx, opening.profile),
                MoreSessions::Window => self.open_window(&opening),
            }
        }
    }

    fn open_window(&mut self, opening: &NewWindow) {
        let outcome = match &self.window_opener {
            Some(opener) => (opener.0)(opening),
            None => Err("This Parley cannot open another window.".to_owned()),
        };
        if let Err(why) = outcome {
            self.warning = Some(why);
        }
    }
}

fn tab_name(current: Option<&Profile>) -> String {
    current.map_or_else(|| "New session".to_owned(), |p| p.name.clone())
}

/// One tab per session: a dot for a message or a lost connection, the name, and a close button.
fn tab_bar(ui: &mut Ui, labels: &[(String, Mark)], active: usize) -> Option<TabAction> {
    let mut action = None;
    ui.horizontal(|ui| {
        let p = theme::palette(ui);
        for (i, (name, mark)) in labels.iter().enumerate() {
            let (color, about) = match mark {
                Mark::Message => (Some(p.warning_text), "A message is waiting"),
                Mark::Lost => (Some(p.fail), "Not connected"),
                Mark::None => (None, ""),
            };
            if let Some(color) = color {
                let (dot, _) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), Sense::hover());
                ui.painter().circle_filled(dot.center(), 3.5, color);
            }
            let text = if i == active {
                theme::semibold(name)
            } else {
                RichText::new(name).color(p.muted)
            };
            let mut tab = ui.selectable_label(i == active, text);
            if !about.is_empty() {
                tab = tab.on_hover_text(about);
            }
            if tab.clicked() {
                action = Some(TabAction::Show(i));
            }
            if tab.middle_clicked() {
                action = Some(TabAction::Close(i));
            }
            let close = ui
                .add(theme::icon_button(ui, "🗙").small())
                .on_hover_text("Close tab");
            close.widget_info(|| {
                egui::WidgetInfo::labeled(
                    egui::accesskit::Role::Button,
                    true,
                    format!("Close {name}"),
                )
            });
            if close.clicked() {
                action = Some(TabAction::Close(i));
            }
            ui.add_space(10.0);
        }
    });
    action
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::net::{TcpListener, TcpStream};
    use std::rc::Rc;

    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;
    use parley_core::net::LinkState;

    use super::*;
    use crate::groups::SessionGroup;
    use crate::test_support::profile;

    /// A session on a local listener, asking nothing before it closes.
    fn local(name: &str, listener: &TcpListener) -> Profile {
        let mut p = profile(name, "127.0.0.1");
        p.config.port = listener.local_addr().expect("its address").port();
        p.config.tls = false;
        p.config.device_name = "DSP01".into();
        p.confirm_exit = false;
        p
    }

    fn app(profiles: Vec<Profile>, more: MoreSessions) -> Harness<'static, ParleyUi> {
        let mut app = ParleyUi::new(profiles);
        let mut settings = app.settings.clone();
        settings.more_sessions = more;
        app.set_settings(settings);
        Harness::builder()
            .with_size(egui::vec2(1000.0, 800.0))
            .with_max_steps(16)
            .build_ui_state(|ui, app: &mut ParleyUi| app.ui(ui), app)
    }

    /// Runs frames until `done`, or fails.
    fn until(harness: &mut Harness<'static, ParleyUi>, done: impl Fn(&ParleyUi) -> bool) {
        for _ in 0..300 {
            if done(harness.state()) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
            harness.run();
        }
        panic!("never happened");
    }

    fn online(ui: &ParleyUi) -> bool {
        ui.connection
            .as_ref()
            .is_some_and(|c| matches!(c.state(), LinkState::Connected))
    }

    fn connect_first(harness: &mut Harness<'static, ParleyUi>) {
        harness.get_by_label("Connect").click();
        harness.run();
        until(harness, online);
    }

    /// Whatever the windows were asked to open.
    fn opened(harness: &mut Harness<'static, ParleyUi>) -> Rc<RefCell<Vec<NewWindow>>> {
        let opened = Rc::new(RefCell::new(Vec::new()));
        let kept = Rc::clone(&opened);
        harness
            .state_mut()
            .set_window_opener(Box::new(move |new: &NewWindow| {
                kept.borrow_mut().push(new.clone());
                Ok(())
            }));
        opened
    }

    fn duplicate(harness: &mut Harness<'static, ParleyUi>) {
        harness.get_by_label("File").click();
        harness.run();
        harness.get_by_label("Duplicate session").click();
        harness.run();
    }

    #[test]
    fn a_duplicate_opens_in_a_tab_without_the_device_name_r_ui_30_r_ui_33() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a local port");
        let mut harness = app(vec![local("Prod æøå", &listener)], MoreSessions::Tab);
        assert!(
            harness.query_by_label("Close Prod æøå").is_none(),
            "one session: no tab bar"
        );
        connect_first(&mut harness);
        duplicate(&mut harness);
        until(&mut harness, online);
        let ui = harness.state();
        assert_eq!((ui.tabs.len(), ui.active), (2, 1));
        let front = ui.current.as_ref().expect("the duplicate");
        assert_eq!(front.name, "Prod æøå");
        assert_eq!(front.config.device_name, "", "the IBM i names the device");
        assert_eq!(
            ui.tabs[0]
                .current
                .as_ref()
                .map(|p| p.config.device_name.as_str()),
            Some("DSP01")
        );
        assert_eq!(harness.query_all_by_label("Close Prod æøå").count(), 2);

        harness.key_press_modifiers(egui::Modifiers::CTRL, egui::Key::Tab);
        harness.run();
        assert_eq!(
            harness.state().active,
            0,
            "Ctrl+Tab goes round to the first"
        );
        harness.key_press_modifiers(
            egui::Modifiers::CTRL | egui::Modifiers::SHIFT,
            egui::Key::Tab,
        );
        harness.run();
        assert_eq!(harness.state().active, 1, "Ctrl+Shift+Tab goes back");

        harness
            .query_all_by_label("Close Prod æøå")
            .next()
            .expect("a tab")
            .click();
        harness.run();
        let ui = harness.state();
        assert_eq!((ui.tabs.len(), ui.active), (1, 0));
        assert!(ui.connection.is_some(), "the other session stays");
        assert_eq!(
            ui.current.as_ref().map(|p| p.config.device_name.as_str()),
            Some("")
        );
    }

    #[test]
    fn a_duplicate_opens_in_another_window_by_default_r_ui_30_r_ui_34() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a local port");
        let mut harness = app(vec![local("Prod", &listener)], MoreSessions::Window);
        let opened = opened(&mut harness);
        connect_first(&mut harness);
        duplicate(&mut harness);
        assert_eq!(harness.state().tabs.len(), 1);
        let opened = opened.borrow();
        let [new] = opened.as_slice() else {
            panic!("one window: {opened:?}");
        };
        assert!(new.saved && new.duplicate);
        assert_eq!(new.profile.name, "Prod");
        assert_eq!(new.profile.config.device_name, "");
    }

    #[test]
    fn a_window_that_cannot_open_says_why_r_ui_34() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a local port");
        let mut harness = app(vec![local("Prod", &listener)], MoreSessions::Window);
        connect_first(&mut harness);
        duplicate(&mut harness);
        assert_eq!(
            harness.state().warning.as_deref(),
            Some("This Parley cannot open another window.")
        );
    }

    fn with_group(harness: &mut Harness<'static, ParleyUi>, names: &[&str]) {
        let ui = harness.state_mut();
        let mut settings = ui.settings.clone();
        settings.session_groups = vec![SessionGroup {
            name: "Morgen".into(),
            sessions: names.iter().map(|&n| n.to_owned()).collect(),
            start_on_launch: false,
        }];
        ui.set_settings(settings);
        harness.run();
    }

    #[test]
    fn a_group_starts_here_first_then_in_windows_r_ui_32() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a local port");
        let profiles = vec![
            local("A", &listener),
            local("B", &listener),
            local("C", &listener),
        ];
        let mut harness = app(profiles, MoreSessions::Window);
        let opened = opened(&mut harness);
        with_group(&mut harness, &["C", "A", "Gone"]);
        harness.get_by_label("Start").click();
        harness.run();
        assert_eq!(
            harness.state().current.as_ref().map(|p| p.name.as_str()),
            Some("C")
        );
        assert!(harness.state().connection.is_some());
        let names: Vec<String> = opened
            .borrow()
            .iter()
            .map(|n| n.profile.name.clone())
            .collect();
        assert_eq!(names, ["A"], "a session no longer saved is left out");
        assert!(opened.borrow().iter().all(|n| n.saved && !n.duplicate));
    }

    #[test]
    fn a_group_starts_in_tabs_from_the_file_menu_r_ui_32_r_ui_33() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a local port");
        let mut harness = app(
            vec![local("A", &listener), local("B", &listener)],
            MoreSessions::Tab,
        );
        with_group(&mut harness, &["A", "B"]);
        harness.get_by_label("File").click();
        harness.run();
        harness.get_by_label("Start group").hover();
        harness.run();
        // The start screen's card has the name too; the menu's comes first.
        harness
            .query_all_by_label("Morgen")
            .next()
            .expect("in the menu")
            .click();
        harness.run();
        let ui = harness.state();
        assert_eq!((ui.tabs.len(), ui.active), (2, 1));
        assert_eq!(ui.current.as_ref().map(|p| p.name.as_str()), Some("B"));
        assert_eq!(
            ui.tabs[0].current.as_ref().map(|p| p.name.as_str()),
            Some("A")
        );
    }

    #[test]
    fn a_tab_behind_whose_host_ends_it_closes_and_the_window_stays_r_ui_33() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a local port");
        let mut ended = local("A", &listener);
        ended.exit_on_close = true;
        let mut harness = app(vec![ended], MoreSessions::Tab);
        harness.get_by_label("Connect").click();
        harness.run();
        let (first, _): (TcpStream, _) = listener.accept().expect("the first session");
        until(&mut harness, online);
        duplicate(&mut harness);
        let (_second, _) = listener.accept().expect("the duplicate");
        until(&mut harness, online);
        until(&mut harness, |ui| {
            ui.tabs[0]
                .connection
                .as_ref()
                .is_some_and(|c| matches!(c.state(), LinkState::Connected))
                || ui.tabs[0].online
        });
        drop(first);
        until(&mut harness, |ui| ui.tabs.len() == 1);
        let ui = harness.state();
        assert!(!ui.exit_confirmed, "the window stays open");
        assert!(ui.connection.is_some(), "with the duplicate");
    }

    #[test]
    fn closing_the_window_asks_once_for_every_connected_tab_r_ui_33() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a local port");
        let mut asking = local("A", &listener);
        asking.confirm_exit = true;
        let mut harness = app(vec![asking], MoreSessions::Tab);
        connect_first(&mut harness);
        duplicate(&mut harness);
        until(&mut harness, online);
        assert_eq!(harness.state().sessions_asking_to_exit(), ["A", "A"]);
        harness.state_mut().asking_to_exit = true;
        harness.run();
        assert!(
            harness
                .query_by_label("2 sessions are still connected.")
                .is_some()
        );
    }

    #[test]
    fn a_connected_tab_that_asks_is_asked_about_before_it_closes_r_ui_33() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a local port");
        let mut asking = local("A", &listener);
        asking.confirm_exit = true;
        let mut harness = app(vec![asking], MoreSessions::Tab);
        connect_first(&mut harness);
        duplicate(&mut harness);
        harness
            .query_all_by_label("Close A")
            .next()
            .expect("a tab")
            .click();
        harness.run();
        assert_eq!(harness.state().tabs.len(), 2, "not yet");
        harness.get_by_label("Close session").click();
        harness.run();
        assert_eq!(harness.state().tabs.len(), 1);
    }
}
