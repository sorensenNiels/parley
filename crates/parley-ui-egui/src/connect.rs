//! The start screen while not connected: the saved sessions, to connect to or edit (R-UI-5);
//! in Parley SQL, its saved connections (R-SQL-46); in Parley SSH, its saved sessions (R-SSH-11).

use egui::{Button, RichText, Ui};

use crate::profile::Profile;
use crate::theme;

/// What the operator chose on the start screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    Connect(usize),
    Edit(usize),
    New,
}

/// What the start screen calls the things it lists.
#[derive(Debug, Clone, Copy)]
pub struct Words {
    /// The screen's heading: "Connect to IBM i".
    pub heading: &'static str,
    /// Said when there are none: "No sessions yet."
    pub none: &'static str,
    /// The button that adds one: "New session…"
    pub new: &'static str,
}

/// Parley's words: saved sessions.
pub const SESSIONS: Words = Words {
    heading: "Connect to IBM i",
    none: "No sessions yet.",
    new: "New session…",
};

/// One card: a name and the line under it.
#[derive(Debug, Clone)]
pub struct Entry<'a> {
    pub name: &'a str,
    pub address: String,
}

impl<'a> Entry<'a> {
    /// A saved session: its name, host:port and TLS.
    pub fn session(profile: &'a Profile) -> Self {
        let config = &profile.config;
        let tls = if config.tls { "  ·  TLS" } else { "" };
        Self {
            name: &profile.name,
            address: format!("{}:{}{tls}", config.host, config.port),
        }
    }
}

pub fn show(
    ui: &mut Ui,
    words: Words,
    entries: &[Entry<'_>],
    last_error: Option<&str>,
) -> Option<Choice> {
    let mut choice = None;
    ui.vertical_centered(|ui| {
        ui.add_space(ui.available_height() * 0.12);
        ui.heading(words.heading);
        ui.add_space(12.0);
        if let Some(error) = last_error {
            theme::error(ui, error);
            ui.add_space(8.0);
        }
        if entries.is_empty() {
            theme::hint(ui, words.none);
            ui.add_space(8.0);
        }
    });
    let width = 460.0_f32.min(ui.available_width());
    let margin = (ui.available_width() - width) / 2.0;
    ui.horizontal(|ui| {
        ui.add_space(margin);
        ui.vertical(|ui| {
            ui.set_width(width);
            for (i, entry) in entries.iter().enumerate() {
                if let Some(chosen) = card(ui, entry, "Connect", "Edit…") {
                    choice = Some(match chosen {
                        Card::Connect => Choice::Connect(i),
                        Card::Edit => Choice::Edit(i),
                    });
                }
                ui.add_space(4.0);
            }
            ui.add_space(12.0);
            if ui
                .add(Button::new(words.new).min_size(egui::vec2(width, 34.0)))
                .clicked()
            {
                choice = Some(Choice::New);
            }
        });
    });
    choice
}

/// A second list of cards under `show`'s, as wide and centred, below a section label; `primary` names each card's
/// main button (Connect, chosen as [`Choice::Connect`]) and `second` the one beside it, chosen as
/// [`Choice::Edit`]. Parley SSH's hosts from `~/.ssh/config` (R-SSH-42); Parley's session groups (R-UI-32).
pub fn group(
    ui: &mut Ui,
    label: &str,
    entries: &[Entry<'_>],
    primary: &str,
    second: &str,
) -> Option<Choice> {
    let mut choice = None;
    let width = 460.0_f32.min(ui.available_width());
    let margin = (ui.available_width() - width) / 2.0;
    ui.horizontal(|ui| {
        ui.add_space(margin);
        ui.vertical(|ui| {
            ui.set_width(width);
            theme::section_label(ui, label);
            for (i, entry) in entries.iter().enumerate() {
                if let Some(chosen) = card(ui, entry, primary, second) {
                    choice = Some(match chosen {
                        Card::Connect => Choice::Connect(i),
                        Card::Edit => Choice::Edit(i),
                    });
                }
                ui.add_space(4.0);
            }
        });
    });
    choice
}

enum Card {
    Connect,
    Edit,
}

/// One saved session or connection: its name and address, with `second` (Edit…) and `primary` (Connect).
fn card(ui: &mut Ui, entry: &Entry<'_>, primary: &str, second: &str) -> Option<Card> {
    let muted = theme::palette(ui).muted;
    theme::card(ui)
        .inner_margin(egui::Margin::symmetric(14, 10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(theme::semibold(entry.name).size(14.5));
                    ui.label(RichText::new(&entry.address).small().color(muted));
                });
                let connect = theme::primary_button(ui, primary);
                theme::button_row(ui, |ui| {
                    if ui.add(connect).clicked() {
                        Some(Card::Connect)
                    } else if ui.button(second).clicked() {
                        Some(Card::Edit)
                    } else {
                        None
                    }
                })
            })
            .inner
        })
        .inner
}
