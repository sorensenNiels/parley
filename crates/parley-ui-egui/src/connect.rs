//! The start screen while not connected: the saved sessions, to connect to or edit (R-UI-5).

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

pub fn show(ui: &mut Ui, profiles: &[Profile], last_error: Option<&str>) -> Option<Choice> {
    let mut choice = None;
    ui.vertical_centered(|ui| {
        ui.add_space(ui.available_height() * 0.12);
        ui.heading("Connect to IBM i");
        ui.add_space(12.0);
        if let Some(error) = last_error {
            theme::error(ui, error);
            ui.add_space(8.0);
        }
        if profiles.is_empty() {
            theme::hint(ui, "No sessions yet.");
            ui.add_space(8.0);
        }
    });
    let width = 460.0_f32.min(ui.available_width());
    let margin = (ui.available_width() - width) / 2.0;
    ui.horizontal(|ui| {
        ui.add_space(margin);
        ui.vertical(|ui| {
            ui.set_width(width);
            for (i, profile) in profiles.iter().enumerate() {
                if let Some(chosen) = card(ui, profile) {
                    choice = Some(match chosen {
                        Card::Connect => Choice::Connect(i),
                        Card::Edit => Choice::Edit(i),
                    });
                }
                ui.add_space(4.0);
            }
            ui.add_space(12.0);
            if ui
                .add(Button::new("New session…").min_size(egui::vec2(width, 34.0)))
                .clicked()
            {
                choice = Some(Choice::New);
            }
        });
    });
    choice
}

enum Card {
    Connect,
    Edit,
}

/// One saved session: its name and address, with Edit… and Connect.
fn card(ui: &mut Ui, profile: &Profile) -> Option<Card> {
    let muted = theme::palette(ui).muted;
    theme::card(ui)
        .inner_margin(egui::Margin::symmetric(14, 10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(theme::semibold(&profile.name).size(14.5));
                    let config = &profile.config;
                    let tls = if config.tls { "  ·  TLS" } else { "" };
                    let address = format!("{}:{}{tls}", config.host, config.port);
                    ui.label(RichText::new(address).small().color(muted));
                });
                let connect = theme::primary_button(ui, "Connect");
                theme::button_row(ui, |ui| {
                    if ui.add(connect).clicked() {
                        Some(Card::Connect)
                    } else if ui.button("Edit…").clicked() {
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
