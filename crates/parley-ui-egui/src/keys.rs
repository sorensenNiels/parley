//! The Keys menu in the status bar (R-UI-16): the 5250 keys a PC keyboard lacks or hides,
//! each with the keys of the map in use that press it, and F1–F24 in a grid. Out of the way,
//! since they are seldom needed, but always one click away.

use egui::{Button, RichText, Ui};
use parley_core::{Aid, Key};

use crate::icons;
use crate::keymap::{Function, KeyMap};
use crate::theme;

/// The keys before the function keys, with their icons and names.
const KEYS: [(Key, &str, &str); 8] = [
    (Key::Attention, icons::BELL, "Attention"),
    (Key::SystemRequest, icons::TERMINAL, "System Request"),
    (Key::Reset, icons::RESET, "Reset"),
    (Key::Aid(Aid::CLEAR), icons::ERASER, "Clear"),
    (Key::Aid(Aid::HELP), icons::QUESTION, "Help"),
    (Key::Aid(Aid::PRINT), icons::PRINTER, "Print"),
    (Key::Aid(Aid::ROLL_DOWN), icons::ARROW_UP, "Roll Down"),
    (Key::Aid(Aid::ROLL_UP), icons::ARROW_DOWN, "Roll Up"),
];

/// Function keys per row of the grid.
const PER_ROW: u8 = 6;

/// The "Keys" button and its menu; the key clicked.
pub fn keys_menu(ui: &mut Ui, map: &KeyMap, mac: bool) -> Option<Key> {
    let mut clicked = None;
    let muted = theme::palette(ui).muted;
    let atoms = (
        icons::icon(icons::SQUARES_FOUR).color(muted),
        RichText::new("Keys").color(muted),
        icons::icon(icons::CARET_DOWN).color(muted),
    );
    let response = ui
        .menu_button(atoms, |ui| {
            theme::section_label(ui, "5250 keys");
            for (key, icon, name) in KEYS {
                let keys = map.keys_for(&Function::Key(key), mac).join(", ");
                let button = icons::button(icon, name).shortcut_text(keys);
                if icons::add_named(ui, true, button, name).clicked() {
                    clicked = Some(key);
                }
            }
            ui.separator();
            theme::section_label(ui, "Function keys");
            if let Some(key) = function_keys(ui, map, mac) {
                clicked = Some(key);
            }
        })
        .response;
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, "Keys"));
    response.on_hover_text("5250 keys: Attention, System Request, F1–F24 …");
    clicked
}

/// F1–F24 in rows of [`PER_ROW`], each showing on hover the keys that press it.
fn function_keys(ui: &mut Ui, map: &KeyMap, mac: bool) -> Option<Key> {
    let mut clicked = None;
    for first in (1..=24).step_by(usize::from(PER_ROW)) {
        ui.horizontal(|ui| {
            for n in first..first + PER_ROW {
                let Some(aid) = Aid::function(n) else {
                    continue;
                };
                let key = Key::Aid(aid);
                let name = format!("F{n}");
                let text = RichText::new(&name).monospace();
                let button = Button::new(text).min_size(egui::vec2(38.0, 0.0));
                let keys = map.keys_for(&Function::Key(key), mac);
                let response = icons::add_named(ui, true, button, &name).on_hover_ui(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(&name);
                        for keys in &keys {
                            theme::kbd(ui, keys);
                        }
                    });
                });
                if response.clicked() {
                    clicked = Some(key);
                }
            }
        });
    }
    clicked
}

#[cfg(test)]
mod tests {
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    use super::*;

    /// The Keys button alone with the Default map; the key clicked last.
    fn keys() -> Harness<'static, Option<Key>> {
        Harness::builder()
            .with_size(egui::vec2(600.0, 600.0))
            .build_ui_state(
                |ui, clicked: &mut Option<Key>| {
                    crate::test_support::install_fonts(ui);
                    ui.add_space(500.0);
                    if let Some(key) = keys_menu(ui, KeyMap::built_in(), false) {
                        *clicked = Some(key);
                    }
                },
                None,
            )
    }

    #[test]
    fn the_keys_menu_sends_attention_and_f15_r_ui_16() {
        for (label, key) in [
            ("Attention", Key::Attention),
            ("F15", Key::Aid(Aid::function(15).expect("F15"))),
        ] {
            let mut harness = keys();
            assert!(harness.query_by_label(label).is_none(), "folded away");
            harness.get_by_label("Keys").click();
            harness.run();
            harness.get_by_label(label).click();
            harness.run();
            assert_eq!(*harness.state(), Some(key), "{label}");
        }
    }
}
