//! Hotspots (R-UI-20): text outside input fields that names something to do, such as
//! `F3=Exit`, `2=Change`, `More...` or a URL, found so a click can do it.

use super::{CodePage, Screen};
use crate::session::{Aid, Key};

/// What a hotspot does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// Press this AID key: `F3=Exit` (F3), `More...`, `Bottom` and a lone `+` (Roll Up).
    Aid(Aid),
    /// Type this option number into the option field: `2=Change`.
    Option(String),
    /// Open this URL in the browser.
    Url(String),
}

/// One hotspot: positions `start..end` (0-based columns) on `row`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hotspot {
    pub row: usize,
    pub start: usize,
    pub end: usize,
    pub target: Target,
}

impl Hotspot {
    pub const fn contains(&self, row: usize, column: usize) -> bool {
        row == self.row && column >= self.start && column < self.end
    }

    /// The keys that do what the hotspot says on `screen`; none for a URL, which the UI opens.
    /// An option is typed into the input field on the cursor's row, else into the first input
    /// field after the hotspot, replacing what was there; without such a field, nothing.
    pub fn keys(&self, screen: &Screen) -> Vec<Key> {
        match &self.target {
            Target::Aid(aid) => vec![Key::Aid(*aid)],
            Target::Url(_) => Vec::new(),
            Target::Option(digits) => {
                let columns = usize::from(screen.size().columns());
                let (cursor_row, _) = screen.row_column(screen.cursor);
                let inputs = || screen.fields().iter().filter(|f| f.is_input());
                let here = self.row * columns + self.start;
                let Some(field) = inputs()
                    .find(|f| screen.row_column(f.start).0 == cursor_row)
                    .or_else(|| inputs().find(|f| f.start > here))
                else {
                    return Vec::new();
                };
                let (row, column) = screen.row_column(field.start);
                [Key::MoveTo { row, column }, Key::EraseEof]
                    .into_iter()
                    .chain(digits.chars().map(Key::Char))
                    .collect()
            }
        }
    }
}

/// Every hotspot on `screen`, row by row. Input fields and hidden text are never hotspots.
pub fn hotspots(screen: &Screen, code_page: CodePage) -> Vec<Hotspot> {
    let columns = usize::from(screen.size().columns());
    let mut chars: Vec<Option<char>> = screen
        .cells(code_page)
        .iter()
        .map(|c| (!c.style.hidden).then_some(c.ch))
        .collect();
    for field in screen.fields().iter().filter(|f| f.is_input()) {
        if let Some(positions) = chars.get_mut(field.start..field.end()) {
            positions.fill(None);
        }
    }
    chars
        .chunks(columns)
        .enumerate()
        .flat_map(|(row, line)| {
            scan_row(line)
                .into_iter()
                .map(move |(start, end, target)| Hotspot {
                    row,
                    start,
                    end,
                    target,
                })
        })
        .collect()
}

/// A row's character at `i`, or `None` past its end or inside an input field.
fn at(row: &[Option<char>], i: usize) -> Option<char> {
    row.get(i).copied().flatten()
}

fn is_gap(row: &[Option<char>], i: usize) -> bool {
    at(row, i).is_none_or(|c| c == ' ')
}

/// The hotspots of one row as (start, end, target), `None` marking input-field positions.
fn scan_row(row: &[Option<char>]) -> Vec<(usize, usize, Target)> {
    let mut found = Vec::new();
    let mut i = 0;
    while i < row.len() {
        if (i == 0 || is_gap(row, i - 1))
            && let Some((end, target)) = hotspot_at(row, i)
        {
            found.push((i, end, target));
            i = end;
        } else {
            i += 1;
        }
    }
    found
}

/// The hotspot that starts at `i`, with its end.
fn hotspot_at(row: &[Option<char>], i: usize) -> Option<(usize, Target)> {
    url_at(row, i)
        .or_else(|| legend_at(row, i))
        .or_else(|| paging_at(row, i))
}

fn starts_with(row: &[Option<char>], i: usize, text: &str) -> bool {
    text.chars()
        .enumerate()
        .all(|(k, c)| at(row, i + k) == Some(c))
}

fn url_at(row: &[Option<char>], i: usize) -> Option<(usize, Target)> {
    if !starts_with(row, i, "http://") && !starts_with(row, i, "https://") {
        return None;
    }
    let end = (i..row.len())
        .find(|&k| is_gap(row, k))
        .unwrap_or(row.len());
    let url: String = (i..end).filter_map(|k| at(row, k)).collect();
    Some((end, Target::Url(url)))
}

/// Where a legend's label starts: after `F`? digits, a blank or none, `=`, a blank or none, at a
/// letter.
/// Returns the digits, whether an `F` came first, and the label's start.
fn legend_head(row: &[Option<char>], i: usize) -> Option<(String, bool, usize)> {
    let key = at(row, i) == Some('F');
    let mut k = i + usize::from(key);
    let digits: String = (k..k + 2)
        .map_while(|j| at(row, j).filter(char::is_ascii_digit))
        .collect();
    if digits.is_empty() {
        return None;
    }
    k += digits.len();
    k += usize::from(at(row, k) == Some(' '));
    if at(row, k) != Some('=') {
        return None;
    }
    k += 1;
    k += usize::from(at(row, k) == Some(' '));
    at(row, k)
        .is_some_and(char::is_alphabetic)
        .then_some((digits, key, k))
}

/// `F3=Exit` or `2=Change`: the label runs to two blanks, an input field, the row's end or the
/// next legend after one blank (`F3=Exit F5=Refresh`).
fn legend_at(row: &[Option<char>], i: usize) -> Option<(usize, Target)> {
    let (digits, key, label) = legend_head(row, i)?;
    let target = if key {
        let n = digits.parse().ok()?;
        Target::Aid(Aid::function(n)?)
    } else {
        Target::Option(digits)
    };
    let mut end = label;
    while let Some(c) = at(row, end) {
        if c == ' '
            && (at(row, end + 1).is_none_or(|n| n == ' ') || legend_head(row, end + 1).is_some())
        {
            break;
        }
        end += 1;
    }
    Some((end, target))
}

/// `More...`, `More…`, `Bottom` or a lone `+`, each the last text on its row: Roll Up.
fn paging_at(row: &[Option<char>], i: usize) -> Option<(usize, Target)> {
    let len = ["More...", "More…", "Bottom", "+"]
        .iter()
        .find(|w| starts_with(row, i, w))?
        .chars()
        .count();
    let end = i + len;
    (end..row.len())
        .all(|k| is_gap(row, k))
        .then_some((end, Target::Aid(Aid::ROLL_UP)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::screen::{Field, FieldFormat, ScreenSize};

    fn row(text: &str) -> Vec<Option<char>> {
        text.chars().map(Some).collect()
    }

    fn spans(text: &str) -> Vec<(String, Target)> {
        let line = row(text);
        scan_row(&line)
            .into_iter()
            .map(|(s, e, t)| (text.chars().skip(s).take(e - s).collect(), t))
            .collect()
    }

    fn f(n: u8) -> Target {
        Target::Aid(Aid::function(n).expect("F1-F24"))
    }

    #[test]
    fn function_key_legends_are_hotspots_with_their_labels_r_ui_20() {
        assert_eq!(
            spans(" F3=Exit   F5=Refresh   F12=Cancel"),
            [
                ("F3=Exit".to_owned(), f(3)),
                ("F5=Refresh".to_owned(), f(5)),
                ("F12=Cancel".to_owned(), f(12)),
            ]
        );
    }

    #[test]
    fn a_label_of_several_words_ends_at_two_blanks_or_the_next_legend_r_ui_20() {
        assert_eq!(
            spans("F11=Display names only F24=More keys  next"),
            [
                ("F11=Display names only".to_owned(), f(11)),
                ("F24=More keys".to_owned(), f(24)),
            ]
        );
    }

    #[test]
    fn blanks_around_the_equals_sign_are_allowed_r_ui_20() {
        assert_eq!(spans("F3 = Exit"), [("F3 = Exit".to_owned(), f(3))]);
    }

    #[test]
    fn f25_and_text_inside_words_are_not_hotspots_r_ui_20() {
        assert_eq!(spans("F25=Nothing  XF3=Exit  F3=  a=b  10=20"), []);
    }

    #[test]
    fn option_legends_type_their_number_r_ui_20() {
        assert_eq!(
            spans("  2=Change  4=Delete  12=Work with"),
            [
                ("2=Change".to_owned(), Target::Option("2".into())),
                ("4=Delete".to_owned(), Target::Option("4".into())),
                ("12=Work with".to_owned(), Target::Option("12".into())),
            ]
        );
    }

    #[test]
    fn more_bottom_and_a_lone_plus_at_the_end_of_a_row_roll_up_r_ui_20() {
        let roll = Target::Aid(Aid::ROLL_UP);
        assert_eq!(
            spans("  list        More..."),
            [("More...".to_owned(), roll.clone())]
        );
        assert_eq!(
            spans("  list        More…  "),
            [("More…".to_owned(), roll.clone())]
        );
        assert_eq!(
            spans("              Bottom"),
            [("Bottom".to_owned(), roll.clone())]
        );
        assert_eq!(spans("  1  ITEM            +"), [("+".to_owned(), roll)]);
        assert_eq!(spans("Bottom of the list  1 + 2"), []);
    }

    #[test]
    fn a_url_runs_to_the_next_blank_r_ui_20() {
        assert_eq!(
            spans("See https://pub400.com/æøå?x=1 for more"),
            [(
                "https://pub400.com/æøå?x=1".to_owned(),
                Target::Url("https://pub400.com/æøå?x=1".into())
            )]
        );
    }

    #[test]
    fn an_input_field_ends_a_label_and_is_never_a_hotspot_r_ui_20() {
        let mut line = row("F3=Exit  ");
        line.extend([None, None]);
        assert_eq!(scan_row(&line), [(0, 7, f(3))]);
        let mut cut = row("F3=Ex");
        cut.extend([None, Some('i'), Some('t')]);
        assert_eq!(scan_row(&cut), [(0, 5, f(3))]);
    }

    #[test]
    fn an_empty_row_has_none() {
        assert_eq!(scan_row(&[]), []);
    }

    fn screen_with(lines: &[(usize, &str)], inputs: &[(usize, usize, usize)]) -> Screen {
        let page = CodePage::Ccsid37;
        let mut screen = Screen::new(ScreenSize::Standard);
        for (r, text) in lines {
            let bytes: Vec<u8> = text
                .chars()
                .map(|c| page.to_byte(c).unwrap_or(0x40))
                .collect();
            screen.set_row_bytes(*r, &bytes);
        }
        for &(r, c, len) in inputs {
            screen.add_field(Field {
                start: r * 80 + c,
                len,
                format: FieldFormat(0x4000),
                modified: false,
                self_check: None,
            });
        }
        screen
    }

    #[test]
    fn hotspots_on_a_screen_skip_text_typed_in_input_fields_r_ui_20() {
        let screen = screen_with(&[(2, " F3=Exit"), (5, " F12=Cancel")], &[(5, 0, 20)]);
        let found = hotspots(&screen, CodePage::Ccsid37);
        assert_eq!(found.len(), 1);
        assert_eq!((found[0].row, found[0].start, found[0].end), (2, 1, 8));
        assert!(found[0].contains(2, 1) && found[0].contains(2, 7) && !found[0].contains(2, 8));
    }

    #[test]
    fn an_option_goes_into_the_field_on_the_cursor_s_row_r_ui_20() {
        let mut screen = screen_with(&[(3, " 2=Change")], &[(6, 2, 2), (7, 2, 2)]);
        screen.cursor = 7 * 80 + 10;
        let option = &hotspots(&screen, CodePage::Ccsid37)[0];
        assert_eq!(
            option.keys(&screen),
            [
                Key::MoveTo { row: 7, column: 2 },
                Key::EraseEof,
                Key::Char('2')
            ]
        );
    }

    #[test]
    fn without_a_field_on_the_cursor_s_row_an_option_goes_into_the_first_after_the_legend_r_ui_20()
    {
        let mut screen = screen_with(&[(3, " 12=Work with")], &[(1, 2, 2), (6, 2, 2)]);
        screen.cursor = 20 * 80;
        let option = &hotspots(&screen, CodePage::Ccsid37)[0];
        assert_eq!(
            option.keys(&screen),
            [
                Key::MoveTo { row: 6, column: 2 },
                Key::EraseEof,
                Key::Char('1'),
                Key::Char('2')
            ]
        );
    }

    #[test]
    fn an_option_with_no_field_to_go_to_does_nothing_r_ui_20() {
        let screen = screen_with(&[(3, " 2=Change")], &[]);
        assert_eq!(hotspots(&screen, CodePage::Ccsid37)[0].keys(&screen), []);
    }
}
