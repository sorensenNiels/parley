//! The completion list under the cursor (R-SQL-36): what is offered comes from the window, which
//! asks the schema browser; the editor shows it, moves the choice and puts the chosen name in.

use egui::{Area, Key, Modifiers, Order, Pos2, RichText, ScrollArea, Ui};
use parley_core::sql::complete::Typed;

use crate::theme;

/// Something offered: shown as `label` with `detail` beside it, put in as `insert`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub label: String,
    pub detail: String,
    pub insert: String,
}

/// At most this many are offered.
pub const MAX_ITEMS: usize = 50;

/// The list while a name is typed.
#[derive(Debug, Clone)]
pub struct Popup {
    pub typed: Typed,
    pub items: Vec<Item>,
    pub chosen: usize,
    /// Where the list goes: under the cursor, in screen points.
    pub at: Option<Pos2>,
}

/// What a key did to the list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyAction {
    None,
    Accept,
    Close,
}

impl Popup {
    pub fn new(typed: Typed) -> Self {
        Self {
            typed,
            items: Vec::new(),
            chosen: 0,
            at: None,
        }
    }

    /// Up and Down move the choice; Tab or Enter accept; Esc closes. Taken from the input
    /// before the text field sees them, and only while something is offered.
    pub fn keys(&mut self, ui: &Ui) -> KeyAction {
        if self.items.is_empty() {
            return KeyAction::None;
        }
        let none = Modifiers::NONE;
        let (down, up, tab, enter, escape) = ui.input_mut(|i| {
            (
                i.consume_key(none, Key::ArrowDown),
                i.consume_key(none, Key::ArrowUp),
                i.consume_key(none, Key::Tab),
                i.consume_key(none, Key::Enter),
                i.consume_key(none, Key::Escape),
            )
        });
        let last = self.items.len() - 1;
        if down {
            self.chosen = if self.chosen >= last {
                0
            } else {
                self.chosen + 1
            };
        }
        if up {
            self.chosen = if self.chosen == 0 {
                last
            } else {
                self.chosen - 1
            };
        }
        if tab || enter {
            KeyAction::Accept
        } else if escape {
            KeyAction::Close
        } else {
            KeyAction::None
        }
    }

    /// Offers `items`, keeping the choice on the same name when it is still there.
    pub fn offer(&mut self, items: Vec<Item>) {
        let kept = self
            .items
            .get(self.chosen)
            .and_then(|old| items.iter().position(|i| i.label == old.label));
        self.items = items;
        self.chosen = kept.unwrap_or(0);
    }

    /// Draws the list; the index clicked, if one was.
    pub fn show(&self, ui: &Ui) -> Option<usize> {
        let at = self.at?;
        if self.items.is_empty() {
            return None;
        }
        let mut clicked = None;
        Area::new(egui::Id::unique("sql-completion"))
            .order(Order::Foreground)
            .fixed_pos(at)
            .show(ui.ctx(), |ui| {
                theme::card(ui).show(ui, |ui| {
                    ui.set_min_width(220.0);
                    ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                        let muted = theme::palette(ui).muted;
                        for (i, item) in self.items.iter().enumerate() {
                            let response = ui
                                .horizontal(|ui| {
                                    let label =
                                        ui.selectable_label(i == self.chosen, item.label.as_str());
                                    ui.label(RichText::new(&item.detail).color(muted).small());
                                    label
                                })
                                .inner;
                            if i == self.chosen {
                                response.scroll_to_me(None);
                            }
                            if response.clicked() {
                                clicked = Some(i);
                            }
                        }
                    });
                });
            });
        clicked
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(label: &str) -> Item {
        Item {
            label: label.to_owned(),
            detail: String::new(),
            insert: label.to_owned(),
        }
    }

    #[test]
    fn a_new_offer_keeps_the_choice_on_the_same_name_r_sql_36() {
        let typed = Typed {
            start: 0,
            end: 2,
            prefix: "KU".to_owned(),
            qualifier: None,
        };
        let mut popup = Popup::new(typed);
        popup.offer(vec![item("KUNDE"), item("KUNDER")]);
        popup.chosen = 1;
        popup.offer(vec![item("KUNDEGRP"), item("KUNDER")]);
        assert_eq!(popup.chosen, 1);
        popup.offer(vec![item("ÆØÅ")]);
        assert_eq!(popup.chosen, 0);
    }
}
