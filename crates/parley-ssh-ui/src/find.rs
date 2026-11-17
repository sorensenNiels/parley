//! Find in the scrollback (R-SSH-24): Parley 5250's find bar over a pane, its matches searched in the scrollback
//! and on the screen again only when the text, Match case or the terminal changed, the current one scrolled into
//! view and the ones in view highlighted.

use parley_ssh_core::term::{Found, Terminal};
use parley_ui_egui::find::{FindAction, FindBar, bar_with_match_case};

/// The open find bar of a pane.
#[derive(Debug)]
pub struct Find {
    pub bar: FindBar,
    matches: Vec<Found>,
    /// What the matches were found for: the text, Match case and the terminal's changes.
    searched: Option<(String, bool, u64)>,
    /// The match last scrolled to.
    shown: Option<Found>,
}

impl Find {
    pub fn opened() -> Self {
        Self {
            bar: FindBar::opened(),
            matches: Vec::new(),
            searched: None,
            shown: None,
        }
    }

    /// Searches again when needed; scrolls the current match into view when it changed. Returns the matches in
    /// the lines `view` and the current one, for the screen.
    pub fn update(&mut self, t: &mut Terminal) -> (Vec<Found>, Option<Found>) {
        let key = (self.bar.text.clone(), self.bar.match_case, t.changes());
        if self.searched.as_ref() != Some(&key) {
            self.matches = t.find(&self.bar.text, self.bar.match_case);
            self.searched = Some(key);
        }
        let current = self
            .bar
            .current(self.matches.len())
            .and_then(|i| self.matches.get(i))
            .copied();
        if current.is_some() && current != self.shown {
            if let Some(m) = current {
                t.show_line(m.from.line);
            }
            self.shown = current;
        }
        let first = t.point_in_view(0, 0).line;
        let last = first + t.rows();
        let start = self.matches.partition_point(|m| m.from.line < first);
        let end = self.matches.partition_point(|m| m.from.line < last);
        (self.matches[start..end].to_vec(), current)
    }

    /// Draws the bar; `Close` when Escape or its close button was pressed.
    pub fn bar(&mut self, ui: &mut egui::Ui) -> Option<FindAction> {
        bar_with_match_case(ui, &mut self.bar, self.matches.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_current_match_is_scrolled_into_view_and_only_those_in_view_are_given_r_ssh_24() {
        let mut t = Terminal::new(10, 2);
        t.feed(b"hit 1\r\nx\r\nhit 2\r\ny\r\nz");
        let mut find = Find::opened();
        find.bar.text = "hit".into();
        let (shown, current) = find.update(&mut t);
        assert_eq!(current.map(|m| m.from.line), Some(0), "the first match");
        assert_eq!(t.display_offset(), 3, "scrolled back to it");
        assert_eq!(shown.iter().map(|m| m.from.line).collect::<Vec<_>>(), [0]);
        find.bar.text = "HIT".into();
        find.bar.match_case = true;
        let (shown, current) = find.update(&mut t);
        assert_eq!((shown.len(), current), (0, None));
    }
}
