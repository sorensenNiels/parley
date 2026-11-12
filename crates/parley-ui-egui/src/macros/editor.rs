//! Editing a macro's text in Parley (R-KEY-5): Tools ▸ Macros… ▸ Edit, or New macro… for one
//! written by hand. The text is checked as it is typed, with the same reader that runs macros,
//! and can be saved only when it reads; comments and blank lines are kept as typed.

use egui::{ScrollArea, TextEdit};
use parley_core::macros::Macro;

use super::{Macros, check_name, scope};
use crate::{dialog, theme};

/// The text area's height: fixed, so the dialog keeps its size (CLAUDE.md "Layout traps").
const TEXT_HEIGHT: f32 = 300.0;

/// What a new macro starts with.
const TEMPLATE: &str = "# Parley macro\n";

const SYNTAX: &str = "One step per line: text \"…\", a key (enter, f1–f24, tab, field-exit, reset, …), \
wait, password, move ROW COLUMN. session \"NAME\" first keeps it to one session; # starts a comment.";

/// The open editor.
#[derive(Debug)]
pub struct Editor {
    /// The macro being edited; `None` for a new one, named in the dialog.
    original: Option<String>,
    name: String,
    text: String,
    message: Option<String>,
}

impl Editor {
    /// Opens the saved macro `name`, as its file is.
    pub fn open(macros: &Macros, name: &str) -> Result<Self, String> {
        Ok(Self {
            original: Some(name.to_owned()),
            name: name.to_owned(),
            text: macros.store.read_text(name)?,
            message: None,
        })
    }

    /// A new macro, to write by hand.
    pub fn new_macro() -> Self {
        Self {
            original: None,
            name: String::new(),
            text: TEMPLATE.to_owned(),
            message: None,
        }
    }

    /// Draws the dialog; true once it is closed (saved or not).
    pub fn show(&mut self, ctx: &egui::Context, macros: &mut Macros) -> bool {
        let title = if self.original.is_some() {
            "Edit macro"
        } else {
            "New macro"
        };
        let (close, (cancel, save)) = dialog::dialog(ctx, "macro_editor", title, 640.0, |ui| {
            ui.horizontal(|ui| {
                let label = ui.label("Name");
                if self.original.is_some() {
                    ui.label(theme::semibold(&self.name));
                } else {
                    theme::sized_edit(ui, 260.0, TextEdit::singleline(&mut self.name))
                        .labelled_by(label.id);
                }
            });
            ui.add_space(8.0);
            ScrollArea::vertical()
                .id_salt("macro_text")
                .auto_shrink([false, false])
                .max_height(TEXT_HEIGHT)
                .min_scrolled_height(TEXT_HEIGHT)
                .show(ui, |ui| {
                    let edit = TextEdit::multiline(&mut self.text)
                        .code_editor()
                        .desired_width(f32::INFINITY)
                        .min_size(egui::vec2(0.0, TEXT_HEIGHT));
                    ui.add(edit).on_hover_text("The macro's steps");
                });
            ui.add_space(6.0);
            let checked = Macro::parse(&self.text);
            match &checked {
                Ok(m) => theme::hint(ui, scope(m)),
                Err(e) => theme::error(ui, e.to_string()),
            }
            theme::hint(ui, SYNTAX);
            if let Some(message) = &self.message {
                theme::error(ui, message);
            }
            let save = theme::primary_button(ui, "Save");
            dialog::actions(ui, save, checked.is_ok())
        });
        if close || cancel {
            return true;
        }
        save && self.save(macros)
    }

    /// Writes the text; true when it was saved. A new macro needs a name no macro has.
    fn save(&mut self, macros: &mut Macros) -> bool {
        let name = match &self.original {
            Some(name) => name.clone(),
            None => match check_name(&self.name) {
                Ok(name) if macros.find(&name).is_some() => {
                    self.message = Some(format!("There is already a macro called \"{name}\"."));
                    return false;
                }
                Ok(name) => name,
                Err(e) => {
                    self.message = Some(e);
                    return false;
                }
            },
        };
        let saved = macros.store.save_text(&name, &self.text);
        macros.reload();
        match saved {
            Ok(()) => true,
            Err(e) => {
                self.message = Some(format!("Could not save \"{name}\": {e}"));
                false
            }
        }
    }
}
