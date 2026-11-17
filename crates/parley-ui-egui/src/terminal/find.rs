//! The find bar (R-UI-21): a text field above the screen whose matches the screen highlights. Parley SSH's find in
//! the scrollback uses it too, with Match case (R-SSH-24).

use egui::{Key, Role, TextEdit, Ui, WidgetInfo};
use parley_core::Session;
use parley_core::screen::{Span, find};

use crate::{icons, theme};

/// The find bar while it is open.
#[derive(Debug, Default)]
pub struct FindBar {
    pub text: String,
    /// The match shown stronger, as an index into the matches.
    current: usize,
    /// Give the text field the keyboard on the next frame.
    focus: bool,
    /// Upper and lower case must match (only where the bar offers it).
    pub match_case: bool,
}

/// What the operator did in the bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FindAction {
    Close,
}

impl FindBar {
    pub fn opened() -> Self {
        Self {
            focus: true,
            ..Self::default()
        }
    }

    /// Gives the text field the keyboard again (Ctrl+F while the bar is open).
    pub fn focus(&mut self) {
        self.focus = true;
    }

    pub fn matches(&self, session: &Session) -> Vec<Span> {
        find(session.screen(), session.code_page(), &self.text)
    }

    /// The index of the current match among `count`, kept inside them as the screen changes.
    pub const fn current(&self, count: usize) -> Option<usize> {
        if count == 0 {
            None
        } else {
            Some(if self.current < count {
                self.current
            } else {
                count - 1
            })
        }
    }

    fn step(&mut self, count: usize, forward: bool) {
        if let Some(current) = self.current(count) {
            self.current = if forward {
                (current + 1) % count
            } else {
                (current + count - 1) % count
            };
        }
    }
}

/// The bar's text field, for tests and screen readers.
pub const FIELD: &str = "Find on screen";

/// Draws the bar over `count` matches.
pub fn bar(ui: &mut Ui, bar: &mut FindBar, count: usize) -> Option<FindAction> {
    draw(ui, bar, count, false)
}

/// The bar with a Match case switch (Parley SSH, R-SSH-24).
pub fn bar_with_match_case(ui: &mut Ui, bar: &mut FindBar, count: usize) -> Option<FindAction> {
    draw(ui, bar, count, true)
}

fn draw(ui: &mut Ui, bar: &mut FindBar, count: usize, case: bool) -> Option<FindAction> {
    let mut action = None;
    ui.horizontal(|ui| {
        ui.label(icons::icon(icons::MAGNIFYING_GLASS));
        let edit = ui.add(
            TextEdit::singleline(&mut bar.text)
                .hint_text(FIELD)
                .desired_width(240.0),
        );
        edit.widget_info(|| WidgetInfo::labeled(Role::TextInput, true, FIELD));
        if std::mem::take(&mut bar.focus) {
            edit.request_focus();
        }
        if edit.changed() {
            bar.current = 0;
        }
        let (enter, shift, escape) = ui.input(|i| {
            (
                i.key_pressed(Key::Enter),
                i.modifiers.shift,
                i.key_pressed(Key::Escape),
            )
        });
        // A one-line field lets the keyboard go on Enter and Esc.
        if edit.lost_focus() && escape {
            action = Some(FindAction::Close);
        } else if edit.lost_focus() && enter {
            bar.step(count, !shift);
            edit.request_focus();
        }
        let found = match bar.current(count) {
            _ if bar.text.is_empty() => String::new(),
            None => "No matches".to_owned(),
            Some(i) => format!("{} of {count}", i + 1),
        };
        theme::hint(ui, found);
        let previous = icons::button(icons::ARROW_UP, "").small();
        if icons::add_named(ui, count > 0, previous, "Previous match").clicked() {
            bar.step(count, false);
        }
        let next = icons::button(icons::ARROW_DOWN, "").small();
        if icons::add_named(ui, count > 0, next, "Next match").clicked() {
            bar.step(count, true);
        }
        if case && ui.checkbox(&mut bar.match_case, "Match case").changed() {
            bar.current = 0;
        }
        theme::button_row(ui, |ui| {
            let close = ui.add(theme::icon_button(ui, "🗙")).on_hover_text("Close");
            close.widget_info(|| WidgetInfo::labeled(Role::Button, true, "Close find"));
            if close.clicked() {
                action = Some(FindAction::Close);
            }
        });
    });
    action
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    fn harness(count: usize) -> Harness<'static, (FindBar, Option<FindAction>)> {
        Harness::builder()
            .with_size(egui::vec2(700.0, 60.0))
            .build_ui_state(
                move |ui, (find, action)| {
                    crate::test_support::install_fonts(ui);
                    if let Some(a) = bar(ui, find, count) {
                        *action = Some(a);
                    }
                },
                (FindBar::opened(), None),
            )
    }

    #[test]
    fn the_current_match_stays_inside_the_matches_r_ui_21() {
        let mut find = FindBar::default();
        assert_eq!(find.current(0), None);
        find.step(3, false);
        assert_eq!(find.current(3), Some(2), "back from the first wraps");
        assert_eq!(
            find.current(2),
            Some(1),
            "fewer matches after a screen change"
        );
        find.step(3, true);
        assert_eq!(find.current(3), Some(0));
    }

    #[test]
    fn enter_and_shift_enter_move_between_matches_and_keep_the_keyboard_r_ui_21() {
        let mut harness = harness(3);
        harness.run();
        harness.get_by_label(FIELD).type_text("ab");
        harness.run();
        harness.get_by_label("1 of 3");
        harness.key_press(egui::Key::Enter);
        harness.run();
        harness.get_by_label("2 of 3");
        harness.key_press_modifiers(egui::Modifiers::SHIFT, egui::Key::Enter);
        harness.run();
        harness.get_by_label("1 of 3");
        assert!(
            harness.get_by_label(FIELD).is_focused(),
            "the field keeps the keyboard"
        );
    }

    #[test]
    fn esc_closes_the_bar_r_ui_21() {
        let mut harness = harness(0);
        harness.run();
        harness.key_press(egui::Key::Escape);
        harness.run();
        assert_eq!(harness.state().1, Some(FindAction::Close));
    }

    #[test]
    fn the_close_button_closes_the_bar_and_no_matches_is_said_r_ui_21() {
        let mut harness = harness(0);
        harness.run();
        harness.get_by_label(FIELD).type_text("zz");
        harness.run();
        harness.get_by_label("No matches");
        harness.get_by_label("Close find").click();
        harness.run();
        assert_eq!(harness.state().1, Some(FindAction::Close));
    }
}
