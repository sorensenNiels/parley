//! The mouse for remote programs (R-SSH-18): while the host asked for mouse events and the session lets it have
//! them, presses, releases, drags, moves and the wheel over the screen go to the host instead of selecting and
//! scrolling back. Shift held selects locally as always.

use egui::{Event, Modifiers, MouseWheelUnit, PointerButton, Pos2, Response, Ui};
use parley_ssh_core::keys::Mods;
use parley_ssh_core::mouse::{self, Button};
use parley_ssh_core::term::{MouseTracking, Terminal};

use super::Grid;

/// What the mouse did that is still going on.
#[derive(Debug, Default)]
pub(super) struct State {
    /// The button pressed for the host: its moves and its release go to the host wherever the pointer is.
    held: Option<Button>,
    /// The cell last reported, so a move within a cell sends nothing.
    cell: Option<(usize, usize)>,
    /// Wheel movement not yet a whole line.
    wheel: f32,
}

/// Whether the mouse goes to the host this frame: it asked, the session allows it (`allowed`), and Shift is not
/// held; or a button pressed for the host is still down.
pub(super) fn reporting(ui: &Ui, t: &Terminal, state: &State, allowed: bool) -> bool {
    state.held.is_some()
        || (allowed && t.modes().mouse != MouseTracking::Off && !ui.input(|i| i.modifiers.shift))
}

/// What this frame's pointer events over the screen send to the host.
pub(super) fn report(
    ui: &Ui,
    response: &Response,
    grid: &Grid,
    t: &Terminal,
    state: &mut State,
) -> Vec<u8> {
    let modes = *t.modes();
    let (rows, columns) = (t.rows(), t.columns());
    // Only where nothing else lies over the screen (a dialog, a pane's close button).
    let over = response.contains_pointer();
    let (events, modifiers) = ui.input(|i| (i.events.clone(), i.modifiers));
    let mut bytes = Vec::new();
    let mut send = |event, pos: Pos2, modifiers: Modifiers, state: &mut State| {
        let cell = grid.nearest(pos, rows, columns);
        state.cell = Some(cell);
        bytes.extend(mouse::encode(
            event,
            cell.0,
            cell.1,
            mods(modifiers),
            &modes,
        ));
    };
    for event in events {
        match event {
            Event::PointerButton {
                pos,
                button,
                pressed,
                modifiers,
            } => {
                let Some(button) = button_of(button) else {
                    continue;
                };
                if pressed && over && state.held.is_none() {
                    state.held = Some(button);
                    send(mouse::Event::Press(button), pos, modifiers, state);
                } else if !pressed && state.held == Some(button) {
                    state.held = None;
                    send(mouse::Event::Release(button), pos, modifiers, state);
                }
            }
            Event::PointerMoved(pos) => {
                let cell = grid.nearest(pos, rows, columns);
                if state.cell == Some(cell) || (state.held.is_none() && !over) {
                    continue;
                }
                let event = state.held.map_or(mouse::Event::Move, mouse::Event::Drag);
                send(event, pos, modifiers, state);
            }
            Event::MouseWheel {
                unit,
                delta,
                modifiers,
                ..
            } if over => {
                state.wheel += match unit {
                    MouseWheelUnit::Line => delta.y,
                    MouseWheelUnit::Point => delta.y / grid.cell.y,
                    #[expect(clippy::cast_precision_loss, reason = "a screen's rows are few")]
                    MouseWheelUnit::Page => delta.y * rows as f32,
                };
                let Some(pos) = ui.input(|i| i.pointer.hover_pos()) else {
                    continue;
                };
                while state.wheel.abs() >= 1.0 {
                    let up = state.wheel > 0.0;
                    state.wheel -= state.wheel.signum();
                    let event = if up {
                        mouse::Event::WheelUp
                    } else {
                        mouse::Event::WheelDown
                    };
                    send(event, pos, modifiers, state);
                }
            }
            _ => {}
        }
    }
    bytes
}

fn button_of(button: PointerButton) -> Option<Button> {
    match button {
        PointerButton::Primary => Some(Button::Left),
        PointerButton::Middle => Some(Button::Middle),
        PointerButton::Secondary => Some(Button::Right),
        PointerButton::Extra1 | PointerButton::Extra2 => None,
    }
}

fn mods(m: Modifiers) -> Mods {
    Mods {
        shift: m.shift,
        ctrl: m.ctrl,
        alt: m.alt,
    }
}
