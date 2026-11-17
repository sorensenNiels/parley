//! Find in the scrollback (R-SSH-24): every match of a text in the scrollback and the screen, line by line, with
//! or without matching case. A match is cells, so wide characters and combining marks count as what is shown.

use super::{Point, Terminal, Width};

/// One match: from its first cell to its last, both included.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Found {
    pub from: Point,
    pub to: Point,
}

/// A character as compared: lower case unless case matters.
fn folded(c: char, match_case: bool) -> char {
    if match_case {
        c
    } else {
        c.to_lowercase().next().unwrap_or(c)
    }
}

impl Terminal {
    /// Each match of `text` in the scrollback and on the screen, oldest first; none for empty text. Matches do not
    /// overlap and do not run across lines.
    pub fn find(&self, text: &str, match_case: bool) -> Vec<Found> {
        let needle: Vec<char> = text.chars().map(|c| folded(c, match_case)).collect();
        if needle.is_empty() {
            return Vec::new();
        }
        let mut found = Vec::new();
        for line in 0..self.history.len() + self.rows {
            let Some(row) = self.line(line) else { break };
            // Each character shown, with the column of the cell it is in.
            let shown: Vec<(char, usize)> = row
                .cells
                .iter()
                .enumerate()
                .filter(|(_, cell)| cell.width != Width::Spacer)
                .flat_map(|(column, cell)| {
                    std::iter::once(cell.ch)
                        .chain(cell.combining.iter().copied())
                        .map(move |c| (folded(c, match_case), column))
                })
                .collect();
            let mut at = 0;
            while at + needle.len() <= shown.len() {
                let here = shown[at..at + needle.len()].iter().map(|(c, _)| *c);
                if here.eq(needle.iter().copied()) {
                    let last = shown[at + needle.len() - 1].1;
                    let last = match row.cells.get(last) {
                        Some(cell) if cell.width == Width::Wide => last + 1,
                        _ => last,
                    };
                    found.push(Found {
                        from: Point {
                            line,
                            column: shown[at].1,
                        },
                        to: Point { line, column: last },
                    });
                    at += needle.len();
                } else {
                    at += 1;
                }
            }
        }
        found
    }

    /// Scrolls the view so that `line` shows, near the middle when it is in the scrollback.
    pub fn show_line(&mut self, line: usize) {
        let history = self.history.len();
        self.display_offset = if line >= history {
            0
        } else {
            (history + self.rows / 2).saturating_sub(line).min(history)
        };
    }

    /// Scrolls the view so that `line` is its top row, as far as the scrollback allows (R-SSH-45's jumps).
    pub fn show_line_at_top(&mut self, line: usize) {
        self.display_offset = self.history.len().saturating_sub(line);
    }

    /// Grows each time what is shown may have changed, so a search is made again only then.
    pub const fn changes(&self) -> u64 {
        self.changes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn term(text: &[u8]) -> Terminal {
        let mut t = Terminal::new(12, 3);
        t.feed(text);
        t
    }

    fn at(line: usize, from: usize, to: usize) -> Found {
        Found {
            from: Point { line, column: from },
            to: Point { line, column: to },
        }
    }

    #[test]
    fn matches_are_found_in_the_scrollback_and_on_the_screen_r_ssh_24() {
        let t = term(b"Error one\r\nok\r\nerror two\r\nthird\r\nERROR");
        assert_eq!(t.history_len(), 2);
        assert_eq!(
            t.find("error", false),
            [at(0, 0, 4), at(2, 0, 4), at(4, 0, 4)]
        );
        assert_eq!(t.find("error", true), [at(2, 0, 4)]);
        assert_eq!(t.find("", false), []);
        assert_eq!(t.find("nothing", false), []);
    }

    #[test]
    fn matches_do_not_overlap_and_count_wide_characters_as_cells_r_ssh_24() {
        let t = term("aaaa\r\n日本語 æøå".as_bytes());
        assert_eq!(t.find("aa", false), [at(0, 0, 1), at(0, 2, 3)]);
        assert_eq!(t.find("本", false), [at(1, 2, 3)]);
        assert_eq!(t.find("ØÅ", false), [at(1, 8, 9)]);
    }

    #[test]
    fn a_match_in_the_scrollback_is_scrolled_into_view_r_ssh_24() {
        let mut t = term(b"1\r\n2\r\n3\r\n4\r\n5\r\n6\r\n7\r\n8");
        assert_eq!(t.history_len(), 5);
        t.show_line(0);
        assert_eq!(t.display_offset(), 5);
        assert_eq!(
            t.visible_row(0).map(super::super::Row::text),
            Some("1".to_owned())
        );
        t.show_line(3);
        assert_eq!(t.display_offset(), 3);
        t.show_line(6);
        assert_eq!(t.display_offset(), 0, "on the screen");
        t.show_line_at_top(2);
        assert_eq!(
            t.visible_row(0).map(super::super::Row::text),
            Some("3".to_owned())
        );
        t.show_line_at_top(7);
        assert_eq!(t.display_offset(), 0);
        let before = t.changes();
        t.feed(b"!");
        assert!(t.changes() > before);
    }
}
