//! "Run snippet" (R-SQL-116): find a snippet by typing, then run or insert it from the keyboard.

use egui::{Key, Modifiers, ScrollArea, TextEdit, Ui};
use parley_ui_egui::{dialog, fuzzy, theme};

use super::values::{Go, values_actions};
use super::{SnippetPanel, Use};

/// The Run snippet dialog's state.
#[derive(Debug, Clone, Default)]
pub(super) struct Finder {
    pub(super) query: String,
    /// The row chosen, in the order found.
    pub(super) chosen: usize,
    /// The search field takes the keyboard on the first frame.
    opened: bool,
    /// The chosen row moved by a key: it is scrolled into view.
    reveal: bool,
}

/// A key the dialog acts on, taken before its search field sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pressed {
    Up,
    Down,
    /// Enter.
    Run,
    /// Shift+Enter.
    Insert,
    /// Escape.
    Close,
}

impl Pressed {
    /// The first of the dialog's keys pressed this frame; Shift+Enter before Enter.
    fn take(ctx: &egui::Context) -> Option<Self> {
        let keys = [
            (Modifiers::NONE, Key::ArrowUp, Self::Up),
            (Modifiers::NONE, Key::ArrowDown, Self::Down),
            (Modifiers::SHIFT, Key::Enter, Self::Insert),
            (Modifiers::NONE, Key::Enter, Self::Run),
            (Modifiers::NONE, Key::Escape, Self::Close),
        ];
        ctx.input_mut(|i| {
            keys.into_iter()
                .find(|(modifiers, key, _)| i.consume_key(*modifiers, *key))
                .map(|(_, _, pressed)| pressed)
        })
    }
}

impl SnippetPanel {
    /// Opens Run snippet (R-SQL-116).
    pub fn open_finder(&mut self) {
        self.finding = Some(Finder::default());
    }

    /// Whether Run snippet is open: the window leaves the keys to it.
    pub const fn finding(&self) -> bool {
        self.finding.is_some()
    }

    /// The snippets fitting `query`, best first; all in the panel's order for an empty one. The
    /// title counts most, forgiving a typo; tags and about when they hold the query (D64).
    pub(super) fn found(&self, query: &str) -> Vec<usize> {
        let mut scored: Vec<(u32, usize)> = self
            .entries
            .iter()
            .enumerate()
            .filter_map(|(i, e)| {
                let tags = e.snippet.tags.join(" ");
                let other = [tags.as_str(), e.snippet.about.as_str()];
                fuzzy::fit(query, &e.snippet.title, &other).map(|s| (s, i))
            })
            .collect();
        // Stable: equal fits keep the panel's order.
        scored.sort_by_key(|&(score, _)| std::cmp::Reverse(score));
        scored.into_iter().map(|(_, i)| i).collect()
    }

    /// The dialog; the snippet chosen goes on as from the panel, through the values form.
    pub(super) fn finder_dialog(&mut self, ctx: &egui::Context) -> Option<Use> {
        let mut finder = self.finding.take()?;
        let pressed = Pressed::take(ctx);
        let found = self.found(&finder.query);
        let last = found.len().saturating_sub(1);
        match pressed {
            Some(Pressed::Down) => finder.chosen = (finder.chosen + 1).min(last),
            Some(Pressed::Up) => finder.chosen = finder.chosen.saturating_sub(1),
            Some(Pressed::Run | Pressed::Insert | Pressed::Close) | None => {}
        }
        finder.chosen = finder.chosen.min(last);
        finder.reveal = matches!(pressed, Some(Pressed::Up | Pressed::Down));
        let (close, (cancel, go)) =
            dialog::dialog(ctx, "sql-snippet-finder", "Run snippet", 520.0, |ui| {
                self.finder_content(ui, &mut finder, &found)
            });
        if close || cancel || pressed == Some(Pressed::Close) {
            return None;
        }
        let go = go.or(match pressed {
            Some(Pressed::Run) => Some(true),
            Some(Pressed::Insert) => Some(false),
            Some(Pressed::Up | Pressed::Down | Pressed::Close) | None => None,
        });
        let (Some(run), Some(&i)) = (go, found.get(finder.chosen)) else {
            self.finding = Some(finder);
            return None;
        };
        self.start(i, run)
    }

    /// The search field, the snippets found, and the buttons: `(cancel, Some(run))`.
    fn finder_content(
        &self,
        ui: &mut Ui,
        finder: &mut Finder,
        found: &[usize],
    ) -> (bool, Option<bool>) {
        let field = ui.add(
            TextEdit::singleline(&mut finder.query)
                .hint_text("Search snippets")
                .desired_width(f32::INFINITY),
        );
        if !finder.opened {
            field.request_focus();
            finder.opened = true;
        }
        if field.changed() {
            finder.chosen = 0;
        }
        ui.add_space(8.0);
        let mut double_clicked = false;
        ScrollArea::vertical()
            .id_salt("sql-snippet-finder-list")
            .max_height(280.0)
            .min_scrolled_height(280.0)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if found.is_empty() {
                    theme::hint(ui, "No snippet fits.");
                }
                for (row, &i) in found.iter().enumerate() {
                    let Some(entry) = self.entries.get(i) else {
                        continue;
                    };
                    let chosen = row == finder.chosen;
                    ui.horizontal(|ui| {
                        let label = ui.selectable_label(chosen, entry.snippet.title.as_str());
                        if chosen && finder.reveal {
                            label.scroll_to_me(None);
                        }
                        if label.clicked() {
                            finder.chosen = row;
                        }
                        if label.double_clicked() {
                            finder.chosen = row;
                            double_clicked = true;
                        }
                        theme::hint(ui, entry.snippet.tags.join(", "));
                    });
                }
            });
        theme::hint(ui, "Enter runs, Shift+Enter inserts at the cursor.");
        let (cancel, go) = values_actions(ui, [Go::Run, Go::Insert], !found.is_empty());
        let go = go.map(|go| go != Go::Insert);
        (cancel, go.or(double_clicked.then_some(true)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn starters() -> SnippetPanel {
        let mut panel = SnippetPanel::default();
        panel.load(None, "");
        panel
    }

    fn titles(panel: &SnippetPanel, query: &str) -> Vec<String> {
        panel
            .found(query)
            .into_iter()
            .map(|i| panel.entries[i].snippet.title.clone())
            .collect()
    }

    #[test]
    fn the_best_fit_comes_first_forgiving_a_typo_r_sql_116() {
        let panel = starters();
        assert_eq!(
            titles(&panel, "").len(),
            panel.entries.len(),
            "all at first"
        );
        assert_eq!(titles(&panel, "system status")[0], "System status");
        assert_eq!(titles(&panel, "sytem status")[0], "System status", "a typo");
        assert_eq!(titles(&panel, "locked")[0], "Who has an object locked");
        assert_eq!(titles(&panel, "æøå qqq"), Vec::<String>::new());
    }

    /// Run snippet drawn alone over the starters; what it gave is kept.
    fn drawn() -> egui_kittest::Harness<'static, (SnippetPanel, Option<Use>)> {
        let mut panel = starters();
        panel.open_finder();
        egui_kittest::Harness::builder()
            .with_size(egui::vec2(900.0, 700.0))
            .with_max_steps(16)
            .build_ui_state(
                |ui, (panel, used): &mut (SnippetPanel, Option<Use>)| {
                    parley_ui_egui::test_support::install_fonts(ui);
                    if let Some(u) = panel.dialogs(ui.ctx(), None, "") {
                        *used = Some(u);
                    }
                },
                (panel, None),
            )
    }

    #[test]
    fn typing_and_enter_run_the_best_fit_r_sql_116() {
        use egui_kittest::kittest::Queryable;
        let mut harness = drawn();
        harness.run();
        crate::test_support::keeps_its_place(&mut harness, "Run");
        harness
            .get_by_role(egui::accesskit::Role::TextInput)
            .type_text("system status");
        harness.run();
        harness.key_press(Key::Enter);
        harness.run();
        let (panel, used) = harness.state();
        assert!(!panel.finding(), "closed");
        assert!(
            matches!(used, Some(Use::Run(sql)) if sql.contains("SYSTEM_STATUS_INFO")),
            "{used:?}"
        );
    }

    #[test]
    fn down_and_shift_enter_insert_the_next_and_escape_closes_r_sql_116() {
        use egui_kittest::kittest::Queryable;
        let mut harness = drawn();
        harness.run();
        harness
            .get_by_role(egui::accesskit::Role::TextInput)
            .type_text("system");
        harness.run();
        let second = {
            let panel = &harness.state().0;
            panel.found("system").get(1).copied().expect("two fit")
        };
        harness.key_press(Key::ArrowDown);
        harness.run();
        assert_eq!(
            harness.state().0.finding.as_ref().map(|f| f.chosen),
            Some(1)
        );
        harness.key_press_modifiers(Modifiers::SHIFT, Key::Enter);
        harness.run();
        let (panel, used) = harness.state();
        assert_eq!(panel.entries[second].snippet.title, "System values");
        let expected = panel.entries[second].snippet.sql.clone();
        assert_eq!(used, &Some(Use::Insert(expected)), "it has no placeholders");
        let mut harness = drawn();
        harness.run();
        harness.key_press(Key::Escape);
        harness.run();
        assert!(!harness.state().0.finding());
        assert_eq!(harness.state().1, None);
    }
}
