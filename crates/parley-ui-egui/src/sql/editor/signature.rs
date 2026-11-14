//! A routine's parameters while it is called (R-SQL-65): the window finds the call at the
//! cursor and its signatures in the catalog; the editor shows them in a box above the cursor's
//! line, the parameter the cursor is at in bold.

use egui::{Align2, Area, Order, RichText, Ui};
use parley_core::sql::catalog::Signature;
use parley_core::sql::complete::{self, CallAt};
use parley_core::sql::text;

use super::Scripts;
use crate::theme;

/// At most this many signatures are shown.
const MAX_SIGNATURES: usize = 3;

/// The box's state.
#[derive(Debug, Default)]
pub(super) struct Help {
    /// The call at the cursor and what the catalog has for it.
    shown: Option<(CallAt, Vec<Signature>)>,
    /// Esc hid the box for this call, until the cursor leaves its brackets.
    hidden: Option<CallAt>,
}

impl Scripts {
    /// The routine called where the cursor is, while the text has the keyboard and nothing is
    /// selected.
    pub fn calling(&self, ctx: &egui::Context) -> Option<CallAt> {
        let (start, end) = self.selection;
        if start != end || !self.has_focus(ctx) {
            return None;
        }
        let script = self.text();
        // From the statement's start to the cursor: blanks after it count as inside it.
        let range = text::statement_at(script, start)?;
        let before = script.get(range.start..start.max(range.start))?;
        complete::call_at(before, before.len())
    }

    /// What to show for the call at the cursor; `None` for nothing.
    pub fn set_signatures(&mut self, found: Option<(CallAt, Vec<Signature>)>) {
        let same = |a: &CallAt, b: &CallAt| a.qualifier == b.qualifier && a.name == b.name;
        let still = match (&self.help.hidden, &found) {
            (Some(hidden), Some((call, _))) => same(hidden, call),
            (Some(_) | None, None) | (None, Some(_)) => false,
        };
        if !still {
            self.help.hidden = None;
        }
        self.help.shown = found;
    }

    /// Whether the box is shown, so Esc hides it first.
    pub(super) fn help_shown(&self) -> bool {
        self.help.shown.is_some() && self.help.hidden.is_none()
    }

    /// Esc: hides the box until the cursor leaves the call.
    pub(super) fn hide_help(&mut self) {
        self.help.hidden = self.help.shown.as_ref().map(|(c, _)| c.clone());
    }

    /// The box above the cursor's line.
    pub(super) fn help_box(&self, ui: &Ui) {
        if !self.help_shown() {
            return;
        }
        let (Some((call, signatures)), Some(below)) = (&self.help.shown, self.cursor_at) else {
            return;
        };
        let line = ui.text_style_height(&egui::TextStyle::Monospace) * f32::from(self.zoom) / 100.0;
        Area::new(egui::Id::unique("sql-signature-help"))
            .order(Order::Foreground)
            .pivot(Align2::LEFT_BOTTOM)
            .fixed_pos(egui::pos2(below.x, below.y - line - 4.0))
            .interactable(false)
            .show(ui.ctx(), |ui| {
                theme::card(ui).show(ui, |ui| {
                    for signature in signatures.iter().take(MAX_SIGNATURES) {
                        signature_line(ui, call, signature, signatures.len() > 1);
                    }
                });
            });
    }
}

/// One signature: `NAME(IN A INTEGER, OUT B VARCHAR(50))`, the argument at the cursor bold.
fn signature_line(ui: &mut Ui, call: &CallAt, signature: &Signature, several: bool) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        let mono = |text: String| RichText::new(text).monospace();
        ui.label(mono(format!("{}(", call.name)));
        for (i, parameter) in signature.parameters.iter().enumerate() {
            if i > 0 {
                ui.label(mono(", ".to_owned()));
            }
            let words: Vec<&str> = [
                parameter.mode.as_str(),
                parameter.name.as_str(),
                parameter.data_type.as_str(),
            ]
            .into_iter()
            .filter(|w| !w.is_empty())
            .collect();
            let text = words.join(" ");
            if i == call.argument {
                ui.label(theme::semibold(text));
            } else {
                ui.label(mono(text).color(theme::palette(ui).muted));
            }
        }
        ui.label(mono(")".to_owned()));
        if several {
            let count = signature.parameters.len();
            let plural = if count == 1 { "" } else { "s" };
            ui.add_space(8.0);
            theme::hint(ui, format!("{count} parameter{plural}"));
        }
    });
}
