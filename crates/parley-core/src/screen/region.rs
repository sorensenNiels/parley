//! Parts of the screen as text: matches of a search (R-UI-21) and a rectangle copied as lines
//! or as a table (R-UI-22).

use super::{CodePage, Screen, is_attribute};

/// Positions `start..end` (0-based columns) on `row`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub row: usize,
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub const fn contains(&self, row: usize, column: usize) -> bool {
        row == self.row && column >= self.start && column < self.end
    }
}

/// A rectangle of positions, its corners included (0-based).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rectangle {
    pub top: usize,
    pub left: usize,
    pub bottom: usize,
    pub right: usize,
}

impl Rectangle {
    /// The rectangle with these two corners, in either order.
    pub fn spanning(a: (usize, usize), b: (usize, usize)) -> Self {
        Self {
            top: a.0.min(b.0),
            left: a.1.min(b.1),
            bottom: a.0.max(b.0),
            right: a.1.max(b.1),
        }
    }

    pub const fn contains(&self, row: usize, column: usize) -> bool {
        row >= self.top && row <= self.bottom && column >= self.left && column <= self.right
    }

    /// The whole of `screen`.
    pub fn all(screen: &Screen) -> Self {
        let size = screen.size();
        Self {
            top: 0,
            left: 0,
            bottom: usize::from(size.rows()) - 1,
            right: usize::from(size.columns()) - 1,
        }
    }
}

/// The screen's characters, row by row, as the operator sees them.
fn rows(screen: &Screen, code_page: CodePage) -> Vec<Vec<char>> {
    let columns = usize::from(screen.size().columns());
    screen
        .cells(code_page)
        .chunks(columns)
        .map(|row| row.iter().map(|c| c.ch).collect())
        .collect()
}

/// A character compared without its case: `Æ` finds `æ`, but `a` does not find `æ`.
fn fold(c: char) -> char {
    c.to_lowercase().next().unwrap_or(c)
}

/// Every place `needle` is on the screen, row by row and left to right, without overlaps;
/// none for an empty needle. A match never runs from one row into the next.
pub fn find(screen: &Screen, code_page: CodePage, needle: &str) -> Vec<Span> {
    let needle: Vec<char> = needle.chars().map(fold).collect();
    if needle.is_empty() {
        return Vec::new();
    }
    let mut found = Vec::new();
    for (row, line) in rows(screen, code_page).iter().enumerate() {
        let line: Vec<char> = line.iter().copied().map(fold).collect();
        let mut start = 0;
        while start + needle.len() <= line.len() {
            if line[start..start + needle.len()] == needle[..] {
                found.push(Span {
                    row,
                    start,
                    end: start + needle.len(),
                });
                start += needle.len();
            } else {
                start += 1;
            }
        }
    }
    found
}

/// The rows of `area`, cut to the screen.
fn cut(screen: &Screen, code_page: CodePage, area: Rectangle) -> Vec<Vec<char>> {
    rows(screen, code_page)
        .into_iter()
        .skip(area.top)
        .take(area.bottom.saturating_sub(area.top) + 1)
        .map(|row| {
            row.into_iter()
                .skip(area.left)
                .take(area.right.saturating_sub(area.left) + 1)
                .collect()
        })
        .collect()
}

/// `area` as text: one line per row, trailing blanks trimmed.
pub fn rectangle_text(screen: &Screen, code_page: CodePage, area: Rectangle) -> String {
    cut(screen, code_page, area)
        .iter()
        .map(|row| row.iter().collect::<String>().trim_end().to_owned())
        .collect::<Vec<_>>()
        .join("\n")
}

/// `area` as tab-separated text, so a list's columns paste into a spreadsheet as columns. A
/// column ends at two or more screen columns that are blank on every row, or at one that is
/// blank on every row and holds a field attribute on one of them (the position before a
/// field). Each cell is trimmed; blank rows are left out.
pub fn table_text(screen: &Screen, code_page: CodePage, area: Rectangle) -> String {
    let rows = cut(screen, code_page, area);
    let width = rows.first().map_or(0, Vec::len);
    let columns = usize::from(screen.size().columns());
    let blank = |c: usize| rows.iter().all(|r| r.get(c).is_none_or(|&ch| ch == ' '));
    let attribute = |c: usize| {
        (area.top..area.top + rows.len())
            .any(|r| is_attribute(screen.get(r * columns + area.left + c)))
    };
    let gap: Vec<bool> = (0..width)
        .map(|c| {
            blank(c) && (attribute(c) || (c > 0 && blank(c - 1)) || (c + 1 < width && blank(c + 1)))
        })
        .collect();
    // The columns of the table: runs of positions that are not a gap.
    let mut spans = Vec::new();
    let mut c = 0;
    while c < width {
        if gap[c] {
            c += 1;
            continue;
        }
        let start = c;
        while c < width && !gap[c] {
            c += 1;
        }
        spans.push(start..c);
    }
    rows.iter()
        .map(|row| {
            spans
                .iter()
                .map(|s| row[s.clone()].iter().collect::<String>().trim().to_owned())
                .collect::<Vec<_>>()
        })
        .filter(|cells| cells.iter().any(|c| !c.is_empty()))
        .map(|cells| cells.join("\t"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::screen::ScreenSize;

    const PAGE: CodePage = CodePage::Ccsid37;

    fn screen_with(lines: &[&str]) -> Screen {
        let mut screen = Screen::new(ScreenSize::Standard);
        for (r, text) in lines.iter().enumerate() {
            let bytes: Vec<u8> = text
                .chars()
                .map(|c| PAGE.to_byte(c).unwrap_or(0x40))
                .collect();
            screen.set_row_bytes(r, &bytes);
        }
        screen
    }

    fn span(row: usize, start: usize, end: usize) -> Span {
        Span { row, start, end }
    }

    #[test]
    fn find_ignores_case_but_not_accents_r_ui_21() {
        let screen = screen_with(&["Work with Jobs", "WORK æøå ÆØÅ aeo"]);
        assert_eq!(find(&screen, PAGE, "work"), [span(0, 0, 4), span(1, 0, 4)]);
        assert_eq!(find(&screen, PAGE, "ÆØÅ"), [span(1, 5, 8), span(1, 9, 12)]);
        assert_eq!(find(&screen, PAGE, "aeo"), [span(1, 13, 16)]);
    }

    #[test]
    fn find_with_nothing_typed_finds_nothing_and_matches_do_not_overlap_r_ui_21() {
        let screen = screen_with(&["aaaa"]);
        assert_eq!(find(&screen, PAGE, ""), []);
        assert_eq!(find(&screen, PAGE, "aa"), [span(0, 0, 2), span(0, 2, 4)]);
    }

    #[test]
    fn a_match_never_runs_into_the_next_row_r_ui_21() {
        let mut screen = screen_with(&[]);
        let end = |r: usize| r * 80 + 78;
        screen.put(end(0), PAGE.to_byte('a').unwrap_or(0));
        screen.put(end(0) + 1, PAGE.to_byte('b').unwrap_or(0));
        screen.put(80, PAGE.to_byte('c').unwrap_or(0));
        assert_eq!(find(&screen, PAGE, "abc"), []);
        assert_eq!(find(&screen, PAGE, "ab"), [span(0, 78, 80)]);
    }

    #[test]
    fn a_rectangle_copies_its_rows_with_trailing_blanks_trimmed_r_ui_22() {
        let screen = screen_with(&["xx Name   Size", "xx æøå    12  ", "xx"]);
        let area = Rectangle::spanning((1, 13), (0, 3));
        assert_eq!(
            rectangle_text(&screen, PAGE, area),
            "Name   Size\næøå    12"
        );
    }

    #[test]
    fn a_rectangle_past_the_screen_edge_is_cut_there_r_ui_22() {
        let screen = screen_with(&["Hello"]);
        let area = Rectangle::spanning((0, 0), (40, 200));
        let text = rectangle_text(&screen, PAGE, area);
        assert_eq!(text.split('\n').count(), 24);
        assert!(text.starts_with("Hello\n"));
    }

    #[test]
    fn a_list_copies_as_a_table_split_at_blank_columns_r_ui_22() {
        let screen = screen_with(&[
            " Opt  Object      Type     Text",
            "      QGPL        *LIB     General purpose library",
            "                                                 ",
            "      ÆØÅ LIB     *LIB     Danish",
        ]);
        let area = Rectangle::spanning((0, 0), (3, 79));
        assert_eq!(
            table_text(&screen, PAGE, area),
            "Opt\tObject\tType\tText\n\tQGPL\t*LIB\tGeneral purpose library\n\tÆØÅ LIB\t*LIB\tDanish"
        );
    }

    #[test]
    fn a_single_blank_column_splits_only_where_a_field_starts_r_ui_22() {
        let mut screen = screen_with(&["AB CD", "EF GH"]);
        assert_eq!(
            table_text(&screen, PAGE, Rectangle::all(&screen)),
            "AB CD\nEF GH"
        );
        screen.put(82, 0x20); // a field attribute at row 2, column 3
        assert_eq!(
            table_text(&screen, PAGE, Rectangle::all(&screen)),
            "AB\tCD\nEF\tGH"
        );
    }

    #[test]
    fn an_empty_screen_copies_as_an_empty_table() {
        let screen = screen_with(&[]);
        assert_eq!(table_text(&screen, PAGE, Rectangle::all(&screen)), "");
    }
}
