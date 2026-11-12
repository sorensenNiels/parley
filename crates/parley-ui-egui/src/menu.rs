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
    /// Help ▸ Check for updates…: Settings on the About page, checking (R-UI-11).
    CheckForUpdates,
    Key(Key),
}

/// The app's icon and name, then the menus; Help on the right.
pub fn menu_bar(
    ui: &mut Ui,
    connected: bool,
    shortcuts_shown: bool,
    profiles: &[Profile],
    logo: Option<&egui::TextureHandle>,
    newer: Option<&parley_core::update::Release>,
) -> Option<Action> {
    let mut action = None;
    MenuBar::new().ui(ui, |ui| {
        brand(ui, logo);
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
        theme::button_row(ui, |ui| {
            ui.menu_button("Help", |ui| {
                if ui.button("Keyboard shortcuts").clicked() {
                    action = Some(Action::ToggleShortcuts);
                }
                if ui.button("Check for updates…").clicked() {
                    action = Some(Action::CheckForUpdates);
                }
                ui.separator();
                ui.add_enabled(
                    false,
                    Button::new(format!("Parley {}", env!("CARGO_PKG_VERSION"))),
                );
            });
            // A quiet link, never a pop-up; the download command is on the About page.
            if let Some(release) = newer {
                ui.hyperlink_to(
                    format!("Parley {} is available", release.version),
                    &release.page,
                )
                .on_hover_text("Opens the release page, with its notes and installers.");
                ui.add_space(8.0);
            }
        });
    });
    action
}

/// The app icon for the menu bar: the decoded image until the first frame makes it a texture.
#[derive(Default)]
pub struct Logo {
    image: Option<egui::ColorImage>,
    texture: Option<egui::TextureHandle>,
}

impl std::fmt::Debug for Logo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(if self.texture.is_some() || self.image.is_some() {
            "Logo"
        } else {
            "Logo(none)"
        })
    }
}

impl Logo {
    pub fn new(image: egui::ColorImage) -> Self {
        Self {
            image: Some(image),
            texture: None,
        }
    }

    /// The icon as a texture, made on first use.
    pub fn texture(&mut self, ctx: &egui::Context) -> Option<&egui::TextureHandle> {
        if let Some(image) = self.image.take() {
            self.texture = Some(ctx.load_texture("logo", image, egui::TextureOptions::LINEAR));
        }
        self.texture.as_ref()
    }
}

/// The app's icon and name, as Tortuga shows its own at the start of its top bar.
fn brand(ui: &mut Ui, logo: Option<&egui::TextureHandle>) {
    if let Some(logo) = logo {
        ui.add(
            egui::Image::new(logo)
                .fit_to_exact_size(egui::vec2(18.0, 18.0))
                .alt_text("Parley"),
        );
    }
    ui.label(theme::semibold("Parley").size(15.0));
    ui.add_space(8.0);
    ui.separator();
    ui.add_space(2.0);
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
