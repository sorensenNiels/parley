//! Pressing a key to bind it, as the Keyboard page (R-KEY-2) and Parley SQL's Shortcuts page
//! (R-SQL-60) both do: a button that waits for a key, and taking that key from the input.

use egui::{Event, EventFilter, RichText, Ui};

use crate::keymap::Chord;

/// Where a pressed key goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    /// In place of this key of the function.
    Replace(Chord),
    /// As one more key.
    Add,
}

/// A key, or + for one more (`slot`): click it, then press the key. While `waiting` it says so
/// and keeps the keyboard, so Tab and the arrows can be pressed too. Returns whether it was
/// clicked.
pub fn key_button(ui: &mut Ui, waiting: bool, slot: Slot, text: &str) -> bool {
    let shown = if waiting { "Press a key…" } else { text };
    let button = egui::Button::new(RichText::new(shown).monospace()).selected(waiting);
    let hover = match slot {
        Slot::Replace(_) => "Click, then press the new key",
        Slot::Add => "Add a key: click, then press it",
    };
    let response = ui.add(button).on_hover_text(hover);
    if waiting {
        response.request_focus();
        ui.memory_mut(|m| {
            m.set_focus_lock_filter(
                response.id,
                EventFilter {
                    tab: true,
                    horizontal_arrows: true,
                    vertical_arrows: true,
                    escape: true,
                },
            );
        });
    }
    response.clicked()
}

/// The key pressed this frame as `make` reads it, taken from the input so nothing else acts on
/// it; also whether anything was taken, and the modifiers held.
pub fn take_key(
    ui: &Ui,
    make: fn(egui::Key, egui::Modifiers) -> Chord,
) -> (Option<Pressed>, bool, egui::Modifiers) {
    let (taken, modifiers) = ui.input_mut(|i| {
        let mut taken = Vec::new();
        i.events.retain(|e| match e {
            Event::Key { .. } | Event::Text(_) | Event::Copy | Event::Cut | Event::Paste(_) => {
                taken.push(e.clone());
                false
            }
            _ => true,
        });
        (taken, i.modifiers)
    });
    let pressed = taken.iter().find_map(|e| match e {
        Event::Key {
            key,
            pressed: true,
            modifiers,
            ..
        } => Some(Pressed::Chord(make(*key, *modifiers))),
        Event::Copy | Event::Cut | Event::Paste(_) => Some(Pressed::Clipboard),
        _ => None,
    });
    (pressed, !taken.is_empty(), modifiers)
}

/// A key pressed while a function waits for one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pressed {
    Chord(Chord),
    /// The system's copy, paste or cut key, which egui never shows as a key.
    Clipboard,
}
