//! The session dialog (R-UI-5): adds, edits and deletes saved sessions, and connects to one.
//! It asks for no user or password: Parley never signs on by itself (R-NET-7).

use egui::{Button, ComboBox, DragValue, Grid, TextEdit, Ui};
use parley_core::session::{TELNET_PORT, TLS_PORT};
use parley_core::{CodePage, ScreenSize, SessionConfig};

use crate::keymap::{DEFAULT_MAP, KeyMap};
use crate::macros::Macros;
use crate::profile::Profile;
use crate::{dialog, theme};

const HELP: &str = "\
Name: what the session is called in the File menu and on the start screen.
Host: the IBM i's name or IP address. TLS encrypts the connection (port 992).
Device name: the display device to use; empty lets the IBM i make one up (QPADEVxxxx).

Keyboard map: which PC keys do what; make your own in Settings, Keyboard.

Exit when the session ends: close Parley when the IBM i ends the session (sign-off).
Confirm exit: ask before closing Parley while connected.
Send keep-alive: send a telnet no-operation after a minute without traffic, so firewalls \
keep an idle connection open.
Connect on program start: connect to this session when Parley starts.
Run macro: on connecting, run a macro recorded with Record macro in the Tools menu, from \
the first screen (it may sign on, asking for the password).";

/// What the dialog asks [`crate::ParleyUi`] to do when it closes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Close,
    /// Connect to the saved session at this index.
    Connect(usize),
}

/// The open dialog: a copy of one session being edited.
#[derive(Debug)]
pub struct Editor {
    /// The saved session being edited; `None` for a new one.
    index: Option<usize>,
    draft: Profile,
    show_help: bool,
    message: Option<String>,
    /// The body's height last frame (see [`dialog::fitting_scroll_area`]).
    body_height: f32,
    /// A saved session that Apply or Connect changed, since the last [`Self::take_saved`].
    saved: Option<Saved>,
}

/// A saved session changed by the dialog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Saved {
    /// Its name before the change: the session in use, and its macros, go by it.
    pub old_name: String,
    /// Where it is among the saved sessions.
    pub index: usize,
}

impl Editor {
    /// Opens on the saved session at `index`, or on a new one.
    pub fn open(profiles: &[Profile], index: Option<usize>) -> Self {
        let mut editor = Self {
            index: None,
            draft: Profile::default(),
            show_help: false,
            message: None,
            body_height: 0.0,
            saved: None,
        };
        editor.select(profiles, index);
        editor
    }

    fn select(&mut self, profiles: &[Profile], index: Option<usize>) {
        let profile = index.and_then(|i| profiles.get(i));
        self.index = profile.and(index);
        self.draft = profile.cloned().unwrap_or_default();
        self.message = None;
    }

    /// Draws the dialog; returns what to do once it closes.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        profiles: &mut Vec<Profile>,
        connected: bool,
        macros: Option<&Macros>,
        maps: &[KeyMap],
    ) -> Option<Outcome> {
        let title = if self.index.is_some() {
            "Edit session"
        } else {
            "New session"
        };
        let (close, outcome) = dialog::dialog(ctx, "session_dialog", title, 760.0, |ui| {
            self.name_row(ui, profiles);
            ui.add_space(12.0);
            // Room for the heading, name and buttons; the rest scrolls on a small window.
            let mut body_height = self.body_height;
            dialog::fitting_scroll_area(ui, "session_body", 240.0, &mut body_height, |ui| {
                ui.columns(2, |columns| {
                    let (left, right) = columns.split_at_mut(1);
                    let (left, right) = (&mut left[0], &mut right[0]);
                    theme::section_label(left, "Connection");
                    connection_fields(left, &mut self.draft.config);
                    theme::section_label(right, "Options");
                    self.options(right, macros, maps);
                    if self.show_help {
                        right.add_space(10.0);
                        theme::hint(right, HELP);
                    }
                });
            });
            self.body_height = body_height;
            if let Some(message) = &self.message {
                ui.add_space(6.0);
                theme::warning(ui, message);
            }
            ui.add_space(16.0);
            self.buttons(ui, profiles, connected)
        });
        if close { Some(Outcome::Close) } else { outcome }
    }

    /// The name, and a menu to switch to another saved session or a new one.
    fn name_row(&mut self, ui: &mut Ui, profiles: &[Profile]) {
        ui.horizontal(|ui| {
            let label = ui.label("Name");
            theme::sized_edit(
                ui,
                300.0,
                TextEdit::singleline(&mut self.draft.name).hint_text("e.g. Production"),
            )
            .labelled_by(label.id);
            ui.menu_button("Sessions ⏷", |ui| {
                let mut chosen = None;
                for (i, profile) in profiles.iter().enumerate() {
                    if ui
                        .add(Button::selectable(self.index == Some(i), &profile.name))
                        .clicked()
                    {
                        chosen = Some(Some(i));
                    }
                }
                ui.separator();
                if ui.button("New session").clicked() {
                    chosen = Some(None);
                }
                if let Some(index) = chosen {
                    self.select(profiles, index);
                }
            });
        });
    }

    /// `macros` is `None` while the Macros add-on is off: no Run macro then (R-ADD-2).
    fn options(&mut self, ui: &mut Ui, macros: Option<&Macros>, maps: &[KeyMap]) {
        keyboard_map(ui, &mut self.draft.keyboard_map, maps);
        if let Some(macros) = macros {
            self.run_macro(ui, macros);
        }
        let profile = &mut self.draft;
        theme::switch_row(ui, "Exit when the session ends", &mut profile.exit_on_close);
        theme::switch_row(ui, "Confirm exit", &mut profile.confirm_exit);
        theme::switch_row(ui, "Send keep-alive", &mut profile.config.keep_alive);
        theme::switch_row(
            ui,
            "Connect to this session on program start",
            &mut profile.connect_on_start,
        );
    }

    /// "Run macro" on connecting (R-KEY-4), and which one.
    fn run_macro(&mut self, ui: &mut Ui, macros: &Macros) {
        let names = macros.names_for(Some(self.draft.name.trim()));
        let chosen = &mut self.draft.run_macro;
        ui.horizontal(|ui| {
            let mut on = !chosen.is_empty();
            let label = ui
                .add_enabled_ui(on || !names.is_empty(), |ui| {
                    theme::switch_row(ui, "Run macro", &mut on)
                        .on_disabled_hover_text("No macros yet: Record macro is in the Tools menu.")
                })
                .inner;
            match (on, chosen.is_empty(), names.first()) {
                (true, true, Some(first)) => chosen.clone_from(first),
                (false, false, _) => chosen.clear(),
                _ => {}
            }
            let shown = if chosen.is_empty() || names.contains(chosen) {
                chosen.clone()
            } else {
                format!("{chosen} (missing)")
            };
            ui.add_enabled_ui(on, |ui| {
                ComboBox::from_id_salt("macro")
                    .selected_text(shown)
                    .width(160.0)
                    .show_ui(ui, |ui| {
                        for name in &names {
                            ui.selectable_value(chosen, name.clone(), name);
                        }
                    })
                    .response
                    .labelled_by(label.id);
            });
        });
    }

    /// A saved session the dialog changed since the last call.
    pub fn take_saved(&mut self) -> Option<Saved> {
        self.saved.take()
    }

    /// Delete and Help on the left; Connect (filled), Apply and Cancel on the right.
    fn buttons(
        &mut self,
        ui: &mut Ui,
        profiles: &mut Vec<Profile>,
        connected: bool,
    ) -> Option<Outcome> {
        ui.horizontal(|ui| {
            let delete = theme::danger_button(ui, "Delete");
            if ui.add_enabled(self.index.is_some(), delete).clicked() {
                self.delete(profiles);
            }
            if ui.add(Button::selectable(self.show_help, "Help")).clicked() {
                self.show_help = !self.show_help;
            }
            let connect = theme::primary_button(ui, "Connect");
            theme::button_row(ui, |ui| {
                let mut outcome = None;
                let connect = ui
                    .add_enabled(!connected, connect)
                    .on_disabled_hover_text("Disconnect first: one session per window.");
                if connect.clicked()
                    && let Some(index) = self.apply(profiles)
                {
                    outcome = Some(Outcome::Connect(index));
                }
                if ui.button("Apply").clicked() && self.apply(profiles).is_some() {
                    self.message = Some("Saved.".to_owned());
                }
                if ui.button("Cancel").clicked() {
                    outcome = Some(Outcome::Close);
                }
                outcome
            })
        })
        .inner
    }

    /// Saves the draft; returns its index, or `None` if the draft is not complete.
    fn apply(&mut self, profiles: &mut Vec<Profile>) -> Option<usize> {
        let name = self.draft.name.trim().to_owned();
        let taken = profiles
            .iter()
            .enumerate()
            .any(|(i, p)| p.name == name && Some(i) != self.index);
        self.message = if name.is_empty() {
            Some("Give the session a name.".to_owned())
        } else if taken {
            Some(format!("There is already a session called \"{name}\"."))
        } else if self.draft.config.host.trim().is_empty() {
            Some("Enter the IBM i's host name or address.".to_owned())
        } else {
            None
        };
        if self.message.is_some() {
            return None;
        }
        self.draft.name = name;
        let old_name = self
            .index
            .and_then(|i| profiles.get(i))
            .map(|p| p.name.clone());
        let index = match self.index {
            Some(i) if i < profiles.len() => {
                profiles[i] = self.draft.clone();
                i
            }
            _ => {
                profiles.push(self.draft.clone());
                profiles.len() - 1
            }
        };
        if self.draft.connect_on_start {
            for (i, profile) in profiles.iter_mut().enumerate() {
                profile.connect_on_start = i == index;
            }
        }
        self.index = Some(index);
        if let Some(old_name) = old_name {
            self.saved = Some(Saved { old_name, index });
        }
        Some(index)
    }

    fn delete(&mut self, profiles: &mut Vec<Profile>) {
        let Some(index) = self.index.filter(|&i| i < profiles.len()) else {
            return;
        };
        let removed = profiles.remove(index);
        self.select(profiles, None);
        self.message = Some(format!("Deleted \"{}\".", removed.name));
    }
}

/// Host, port and TLS, the CA certificate, code page, screen size and device name.
fn connection_fields(ui: &mut Ui, config: &mut SessionConfig) {
    Grid::new("connection")
        .num_columns(2)
        .spacing([12.0, 6.0])
        .show(ui, |ui| {
            let label = ui.label("Host");
            theme::sized_edit(
                ui,
                230.0,
                TextEdit::singleline(&mut config.host).hint_text("ibmi.example.com"),
            )
            .labelled_by(label.id);
            ui.end_row();

            let label = ui.label("Port");
            ui.horizontal(|ui| {
                ui.add(DragValue::new(&mut config.port).range(1..=65_535))
                    .labelled_by(label.id);
                ui.add_space(8.0);
                let tls_before = config.tls;
                theme::switch_row(ui, "TLS", &mut config.tls);
                if config.tls != tls_before {
                    // Follow the switch when the port is still the other mode's default.
                    match (config.tls, config.port) {
                        (true, TELNET_PORT) => config.port = TLS_PORT,
                        (false, TLS_PORT) => config.port = TELNET_PORT,
                        _ => {}
                    }
                }
            });
            ui.end_row();

            if config.tls {
                let label = ui.label("CA certificate");
                theme::sized_edit(
                    ui,
                    230.0,
                    TextEdit::singleline(&mut config.ca_file).hint_text("PEM file (optional)"),
                )
                .labelled_by(label.id);
                ui.end_row();
            }

            let label = ui.label("Code page");
            ComboBox::from_id_salt("code_page")
                .selected_text(config.code_page.label())
                .width(230.0)
                .show_ui(ui, |ui| {
                    for page in CodePage::ALL {
                        ui.selectable_value(&mut config.code_page, page, page.label());
                    }
                })
                .response
                .labelled_by(label.id);
            ui.end_row();

            ui.label("Terminal size");
            theme::segmented(
                ui,
                &mut config.screen_size,
                &[
                    (ScreenSize::Wide, "27 × 132"),
                    (ScreenSize::Standard, "24 × 80"),
                ],
            );
            ui.end_row();

            let label = ui.label("Device name");
            theme::sized_edit(
                ui,
                200.0,
                TextEdit::singleline(&mut config.device_name)
                    .char_limit(10)
                    .hint_text("optional"),
            )
            .labelled_by(label.id);
            ui.end_row();
        });
}

/// The keyboard map the session uses (R-KEY-2): Default or one of the operator's. A map
/// since deleted shows as Default, which is what the session gets.
fn keyboard_map(ui: &mut Ui, chosen: &mut String, maps: &[KeyMap]) {
    ui.horizontal(|ui| {
        let label = ui.label("Keyboard map");
        let shown = KeyMap::find(maps, chosen).name.clone();
        ComboBox::from_id_salt("keyboard_map")
            .selected_text(shown)
            .width(180.0)
            .show_ui(ui, |ui| {
                if ui
                    .selectable_label(!maps.iter().any(|m| m.name == *chosen), DEFAULT_MAP)
                    .clicked()
                {
                    chosen.clear();
                }
                for map in maps {
                    ui.selectable_value(chosen, map.name.clone(), &map.name);
                }
            })
            .response
            .labelled_by(label.id);
    });
}
