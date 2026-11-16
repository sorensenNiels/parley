//! Parley SSH's UI (D41): the start screen, the SSH session in the main window (one per window, D45), under a
//! header and over a status bar, in the look Parley 5250 and Parley SQL share (D12). The engine is
//! `parley-ssh-core`; the launcher (`parley-ssh` in `parley-app`) keeps the settings.

mod bars;
mod dialogs;
pub mod screen;
mod session;
pub mod settings;
mod start;

use egui::{CentralPanel, Margin, Panel, Ui, ViewportCommand};
use parley_ssh_core::known_hosts::KnownHosts;
use parley_ui_egui::menu::{Logo, Program};
use parley_ui_egui::{theme, updates};

use bars::Action;
use session::Session;
pub use settings::Settings;

/// The smallest and largest text size View offers, in percent, and its step.
const TEXT_SIZES: (u16, u16, u16) = (60, 250, 10);

/// Parley SSH's whole UI.
#[derive(Debug)]
pub struct ParleySshUi {
    settings: Settings,
    applied: Option<Settings>,
    settings_dialog: Option<settings::Dialog>,
    logo: Logo,
    updates: updates::Updates,
    known_hosts: KnownHosts,
    form: start::Form,
    session: Option<Session>,
    /// Why the last session closed, for the status bar on the start screen.
    note: Option<String>,
    shortcuts_open: bool,
    /// The window's close button was pressed while connected: ask (R-SSH-10).
    asking_close: bool,
    close_confirmed: bool,
}

impl ParleySshUi {
    /// The UI with `settings`, keeping host keys in `known_hosts` (R-SSH-3).
    pub fn new(settings: Settings, known_hosts: KnownHosts) -> Self {
        Self {
            form: start::Form::from_settings(&settings),
            settings,
            applied: None,
            settings_dialog: None,
            logo: Logo::default(),
            updates: updates::Updates::default(),
            known_hosts,
            session: None,
            note: None,
            shortcuts_open: false,
            asking_close: false,
            close_confirmed: false,
        }
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    pub fn set_logo(&mut self, image: egui::ColorImage) {
        self.logo = Logo::new(image);
    }

    pub fn set_update_checker(&mut self, checker: updates::Checker) {
        self.updates.set_checker(checker);
    }

    /// The title bar: what the host set, or the session, then the program.
    pub fn title(&self) -> String {
        let name = Program::ParleySsh.name();
        let Some(session) = &self.session else {
            return name.to_owned();
        };
        let host_title = session.host_title();
        let shown = if host_title.is_empty() {
            session.target.label()
        } else {
            host_title
        };
        format!("{shown} – {name}")
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        self.apply_settings(ui.ctx());
        self.guard_close(ui.ctx());
        let menu_frame = theme::bar(ui, Margin::symmetric(8, 3));
        let status_frame = theme::bar(ui, Margin::symmetric(8, 5));
        let logo = self.logo.texture(ui.ctx()).cloned();
        let newer = self
            .updates
            .available()
            .map(|r| (r.version.clone(), r.page.clone()));
        let header = bars::Header {
            logo: logo.as_ref(),
            session: self.session.as_ref(),
            newer: newer.as_ref().map(|(v, p)| (v.as_str(), p.as_str())),
        };
        let action = Panel::top("ssh-menu")
            .frame(menu_frame)
            .show(ui, |ui| bars::menu_bar(ui, &header))
            .inner;
        Panel::bottom("ssh-status")
            .frame(status_frame)
            .show(ui, |ui| {
                bars::status_bar(ui, self.session.as_ref(), self.note.as_deref());
            });
        let shortcuts = self.central(ui);
        for shortcut in shortcuts {
            match shortcut {
                screen::Shortcut::Restart => self.carry_out(ui.ctx(), Action::Restart),
            }
        }
        if let Some(action) = action {
            self.carry_out(ui.ctx(), action);
            // A menu's click took the keyboard from the screen: give it back.
            if let Some(session) = &mut self.session {
                session.focus_screen();
            }
        }
        self.show_dialogs(ui.ctx());
    }

    fn apply_settings(&mut self, ctx: &egui::Context) {
        if self.applied.is_none() && self.settings.check_for_updates {
            self.updates.start(ctx, updates::Check::AtStart);
        }
        self.updates.collect();
        if self.applied.as_ref() != Some(&self.settings) {
            let old = self.applied.as_ref().map(Settings::look);
            // Parley SSH uses the bundled default terminal font, which always loads: there is no note.
            let _ = settings::apply(ctx, old.as_ref(), &self.settings.look());
            self.applied = Some(self.settings.clone());
        }
    }

    /// The start screen, or the session's screen.
    fn central(&mut self, ui: &mut Ui) -> Vec<screen::Shortcut> {
        let palette = screen::Palette::for_theme(ui.visuals().dark_mode);
        let confirm_paste = self.settings.confirm_paste;
        let zoom = self.settings.text_size;
        let frame = egui::Frame::NONE.fill(palette.background);
        let mut shortcuts = Vec::new();
        let mut connect = None;
        match &mut self.session {
            Some(session) => {
                session.view.zoom = zoom;
                CentralPanel::default().frame(frame).show(ui, |ui| {
                    shortcuts = session.show(ui, &palette, confirm_paste);
                });
            }
            None => {
                CentralPanel::default().show(ui, |ui| connect = self.form.show(ui));
            }
        }
        if let Some(target) = connect {
            self.connect(ui.ctx(), target);
        }
        shortcuts
    }

    fn connect(&mut self, ctx: &egui::Context, target: parley_ssh_core::net::Target) {
        self.settings.last_host.clone_from(&target.host);
        self.settings.last_port = target.port;
        self.settings.last_user.clone_from(&target.user);
        let ctx = ctx.clone();
        self.session = Some(Session::open(target, self.known_hosts.clone(), move || {
            ctx.request_repaint();
        }));
        self.note = None;
    }

    fn carry_out(&mut self, ctx: &egui::Context, action: Action) {
        match action {
            Action::Restart => {
                if let Some(session) = &mut self.session {
                    session.disconnect();
                    let ctx = ctx.clone();
                    session.restart(move || ctx.request_repaint());
                }
            }
            Action::Disconnect => {
                if let Some(session) = &self.session {
                    session.disconnect();
                }
            }
            Action::CloseSession => {
                if let Some(session) = self.session.take() {
                    session.disconnect();
                    self.note = Some(format!(
                        "Closed the session with {}.",
                        session.target.label()
                    ));
                }
                self.form = start::Form::from_settings(&self.settings);
            }
            Action::Exit => ctx.send_viewport_cmd(ViewportCommand::Close),
            Action::Copy => {
                if let Some(text) = self.session.as_ref().and_then(Session::copy_selection) {
                    ctx.copy_text(text);
                }
            }
            Action::Paste => {
                if let Some(session) = &mut self.session {
                    session.view.request_paste(ctx);
                }
            }
            Action::CopyAll => {
                if let Some(session) = &self.session {
                    ctx.copy_text(session.copy_all());
                }
            }
            Action::ClearScrollback => {
                if let Some(session) = &self.session {
                    session.clear_scrollback();
                }
            }
            Action::ResetTerminal => {
                if let Some(session) = &mut self.session {
                    session.reset_terminal();
                }
            }
            Action::TextLarger => {
                self.settings.text_size =
                    (self.settings.text_size + TEXT_SIZES.2).min(TEXT_SIZES.1);
            }
            Action::TextSmaller => {
                self.settings.text_size = self
                    .settings
                    .text_size
                    .saturating_sub(TEXT_SIZES.2)
                    .max(TEXT_SIZES.0);
            }
            Action::TextActualSize => self.settings.text_size = 100,
            Action::Shortcuts => self.shortcuts_open = true,
            Action::Settings => self.settings_dialog = Some(settings::Dialog::open(&self.settings)),
        }
    }

    /// Closing the window while connected asks first, when the settings say so (R-SSH-10).
    fn guard_close(&mut self, ctx: &egui::Context) {
        if !ctx.input(|i| i.viewport().close_requested()) || self.close_confirmed {
            return;
        }
        let connected = self.session.as_ref().is_some_and(Session::is_connected);
        if connected && self.settings.confirm_close {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            self.asking_close = true;
        }
    }

    fn show_dialogs(&mut self, ctx: &egui::Context) {
        if self.shortcuts_open
            && dialogs::shortcut_sheet(ctx, ctx.os() == egui::os::OperatingSystem::Mac)
        {
            self.shortcuts_open = false;
        }
        if self.asking_close
            && let Some(session) = &self.session
            && let Some(close) = dialogs::confirm_close(ctx, &session.target)
        {
            self.asking_close = false;
            if close {
                self.close_confirmed = true;
                session.disconnect();
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }
        }
        self.show_settings(ctx);
    }

    fn show_settings(&mut self, ctx: &egui::Context) {
        let Some(dialog) = &mut self.settings_dialog else {
            return;
        };
        let closed = dialog.show(ctx, &mut self.settings, self.updates.status());
        if dialog.take_check_request() {
            self.updates.start(ctx, updates::Check::Now);
        }
        match closed {
            Some(settings::Closed::Keep) => self.settings_dialog = None,
            Some(settings::Closed::Revert) => {
                self.settings = dialog.original().clone();
                self.settings_dialog = None;
            }
            None => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    fn app(settings: Settings) -> Harness<'static, ParleySshUi> {
        let known = KnownHosts::new(
            std::env::temp_dir()
                .join("parley-ssh-app-test")
                .join("known_hosts"),
            None,
        );
        Harness::builder()
            .with_size(egui::vec2(900.0, 640.0))
            .build_ui_state(
                |ui, app: &mut ParleySshUi| app.ui(ui),
                ParleySshUi::new(settings, known),
            )
    }

    #[test]
    fn the_start_screen_keeps_the_last_connection_and_says_what_is_missing_r_ssh_2() {
        let settings = Settings {
            last_host: "box.lan".into(),
            last_port: 2222,
            ..Settings::default()
        };
        let mut h = app(settings);
        h.run_steps(3);
        assert_eq!(h.get_by_label("Host").value().as_deref(), Some("box.lan"));
        assert_eq!(h.get_by_label("Port").value().as_deref(), Some("2222"));
        h.get_by_label("Connect").click();
        h.run_steps(3);
        assert!(
            h.query_by_label("Fill in the user name to sign on as.")
                .is_some()
        );
        assert!(h.state().session.is_none(), "nothing is opened");
        assert_eq!(h.state().title(), "Parley SSH");
    }

    #[test]
    fn the_header_has_parley_sshs_menus() {
        let mut h = app(Settings::default());
        h.run_steps(2);
        for menu in ["Session", "Edit", "View", "Help"] {
            assert!(h.query_by_label(menu).is_some(), "{menu}");
        }
        assert!(h.query_by_label("Parley SSH").is_some());
    }

    #[test]
    fn view_changes_the_text_size_within_its_limits() {
        let mut app = ParleySshUi::new(
            Settings::default(),
            KnownHosts::new("known_hosts".into(), None),
        );
        let ctx = egui::Context::default();
        app.carry_out(&ctx, Action::TextLarger);
        assert_eq!(app.settings.text_size, 110);
        app.settings.text_size = TEXT_SIZES.1;
        app.carry_out(&ctx, Action::TextLarger);
        assert_eq!(app.settings.text_size, TEXT_SIZES.1);
        app.carry_out(&ctx, Action::TextActualSize);
        assert_eq!(app.settings.text_size, 100);
    }
}
