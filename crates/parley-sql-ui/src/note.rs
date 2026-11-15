//! A line said to the operator: news, or an error.

use egui::Ui;
use parley_ui_egui::theme;

/// What a message line or a note under a control says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Note {
    Info(String),
    Error(String),
}

impl Note {
    /// What it says, for the tests.
    #[cfg(test)]
    pub fn text(&self) -> &str {
        match self {
            Self::Info(text) | Self::Error(text) => text,
        }
    }

    /// As a hint, or in the error colour.
    pub fn show(&self, ui: &mut Ui) {
        match self {
            Self::Info(text) => theme::hint(ui, text),
            Self::Error(text) => theme::error(ui, text),
        }
    }
}
