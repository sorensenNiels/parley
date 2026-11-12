//! The menu bar and the toolbar (R-UI-4). Both only report what the operator chose, as an
//! [`Action`] for [`crate::ParleyUi`] to carry out. The menus hold what Parley can do today and
//! grow as features arrive.

use egui::{Button, MenuBar, RichText, Ui};
use parley_core::{Aid, Key, SessionConfig};

use crate::profile::Profile;
use crate::theme;

/// Something the operator chose from the menu bar or the toolbar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    NewSession,
    /// The session dialog, on the session in use or the first one.
    EditSessions,
    /// Connect to the saved session at this index.
    Connect(usize),
    Disconnect,
    Exit,
    CopyScreen,
    Paste,
    ToggleShortcuts,
    Settings,
    Key(Key),
}

pub fn menu_bar(
    ui: &mut Ui,
    connected: bool,
    shortcuts_shown: bool,
    profiles: &[Profile],
) -> Option<Action> {
    let mut action = None;
    MenuBar::new().ui(ui, |ui| {
        ui.menu_button("File", |ui| {
            if ui.button("New session…").clicked() {
                action = Some(Action::NewSession);
            }
            if ui.button("Edit sessions…").clicked() {
                action = Some(Action::EditSessions);
            }
            ui.add_enabled_ui(!connected && !profiles.is_empty(), |ui| {
                ui.menu_button("Connect to", |ui| {
                    for (i, profile) in profiles.iter().enumerate() {
                        if ui.button(&profile.name).clicked() {
                            action = Some(Action::Connect(i));
                        }
                    }
                });
            });
            ui.separator();
            if ui
                .add_enabled(connected, Button::new("Disconnect"))
                .clicked()
            {
                action = Some(Action::Disconnect);
            }
            ui.separator();
            if ui.button("Exit").clicked() {
                action = Some(Action::Exit);
            }
        });
        ui.menu_button("Edit", |ui| {
            let copy = Button::new("Copy screen").shortcut_text("Ctrl+C");
            if ui.add_enabled(connected, copy).clicked() {
                action = Some(Action::CopyScreen);
            }
            let paste = Button::new("Paste").shortcut_text("Ctrl+V");
            if ui.add_enabled(connected, paste).clicked() {
                action = Some(Action::Paste);
            }
        });
        ui.menu_button("View", |ui| {
            if ui
                .add(Button::new("Keyboard shortcuts").selected(shortcuts_shown))
                .clicked()
            {
                action = Some(Action::ToggleShortcuts);
            }
        });
        ui.menu_button("Tools", |ui| {
            if ui.button("Settings…").clicked() {
                action = Some(Action::Settings);
            }
        });
        ui.menu_button("Help", |ui| {
            if ui.button("Keyboard shortcuts").clicked() {
                action = Some(Action::ToggleShortcuts);
            }
            ui.separator();
            ui.add_enabled(
                false,
                Button::new(format!("Parley {}", env!("CARGO_PKG_VERSION"))),
            );
        });
    });
    action
}

/// 5250 keys a PC keyboard lacks, and the common actions, as buttons.
/// `config` is the open session's, `None` while not connected.
pub fn toolbar(ui: &mut Ui, config: Option<&SessionConfig>) -> Option<Action> {
    let connected = config.is_some();
    let mut action = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        let mut button = |ui: &mut Ui, button: Button<'_>, chosen: Action| {
            if ui.add_enabled(connected, button).clicked() {
                action = Some(chosen);
            }
        };
        button(ui, Button::new("Disconnect"), Action::Disconnect);
        ui.separator();
        button(ui, Button::new("Copy"), Action::CopyScreen);
        button(ui, Button::new("Paste"), Action::Paste);
        ui.separator();
        button(ui, Button::new("Clear"), Action::Key(Key::Aid(Aid::CLEAR)));
        button(ui, Button::new("Attn"), Action::Key(Key::Attention));
        button(ui, Button::new("Sysreq"), Action::Key(Key::SystemRequest));
        button(ui, Button::new("Reset"), Action::Key(Key::Reset));
        button(ui, Button::new("Help"), Action::Key(Key::Aid(Aid::HELP)));
        let enter = theme::primary_button(ui, "⏵ Enter");
        button(ui, enter, Action::Key(Key::Aid(Aid::ENTER)));

        if let Some(config) = config {
            let muted = theme::palette(ui).muted;
            theme::button_row(ui, |ui| {
                ui.label(RichText::new(format!("{}:{}", config.host, config.port)).color(muted));
            });
        }
    });
    action
}
