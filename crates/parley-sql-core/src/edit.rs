//! The text changes the SQL editor makes from keys (R-SQL-52..58): comment lines, indent and
//! outdent, move and duplicate lines, brackets and quotes that close themselves, the matching
//! bracket, keyword case and where a line starts. Pure functions on the script's text and its
//! selection, as byte offsets on character boundaries; the editor turns keys into them.

use std::ops::Range;

use super::complete::is_name_char;
use super::text::{self, Kind};

/// `width` as a level of indentation: 1 to 8 spaces, as Format SQL keeps it.
fn level(width: usize) -> usize {
    width.clamp(1, 8)
}

/// A script's text after a change, and the selection to give it (start, end), as byte offsets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edit {
    pub text: String,
    pub selection: (usize, usize),
}

/// Up or down, for moving lines (R-SQL-54).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
}

/// Upper or lower case, for keywords (R-SQL-58).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Case {
    Upper,
    Lower,
}

/// The selection, ordered and kept inside `text` on character boundaries.
fn clamp(text: &str, (a, b): (usize, usize)) -> (usize, usize) {
    let fit = |at: usize| {
        let mut at = at.min(text.len());
        while !text.is_char_boundary(at) {
            at -= 1;
        }
        at
    };
    (fit(a.min(b)), fit(a.max(b)))
}

/// The start of the line `at` is on.
fn line_start_at(text: &str, at: usize) -> usize {
    text.get(..at)
        .and_then(|t| t.rfind('\n'))
        .map_or(0, |i| i + 1)
}

/// The end of the line `at` is on, before its line break.
fn line_end_at(text: &str, at: usize) -> usize {
    text.get(at..)
        .and_then(|t| t.find('\n'))
        .map_or(text.len(), |i| at + i)
}

/// The whole lines the selection touches, without the last line break. A selection that ends
/// at the start of a line does not take that line.
fn lines(text: &str, selection: (usize, usize)) -> Range<usize> {
    let (start, end) = clamp(text, selection);
    let end = if end > start && end > 0 && text.as_bytes().get(end - 1) == Some(&b'\n') {
        end - 1
    } else {
        end
    };
    line_start_at(text, start)..line_end_at(text, end.max(start))
}

/// The lines in `range` with their start offsets.
fn each_line(text: &str, range: Range<usize>) -> impl Iterator<Item = (usize, &str)> {
    let block = text.get(range.clone()).unwrap_or_default();
    let mut at = range.start;
    block.split('\n').map(move |line| {
        let start = at;
        at += line.len() + 1;
        (start, line)
    })
}

/// How many bytes of spaces and tabs `line` starts with.
fn indentation(line: &str) -> usize {
    line.len() - line.trim_start_matches([' ', '\t']).len()
}

/// Inserts and removals at offsets of the original text, in order and not overlapping.
struct Changes {
    /// (offset, bytes removed, text inserted), in order of offset.
    list: Vec<(usize, usize, String)>,
}

impl Changes {
    fn new() -> Self {
        Self { list: Vec::new() }
    }

    fn insert(&mut self, at: usize, what: &str) {
        self.list.push((at, 0, what.to_owned()));
    }

    fn remove(&mut self, at: usize, len: usize) {
        if len > 0 {
            self.list.push((at, len, String::new()));
        }
    }

    fn apply(&self, text: &str) -> String {
        let mut out = String::with_capacity(text.len() + 16);
        let mut at = 0;
        for (offset, removed, inserted) in &self.list {
            out.push_str(text.get(at..*offset).unwrap_or_default());
            out.push_str(inserted);
            at = offset + removed;
        }
        out.push_str(text.get(at..).unwrap_or_default());
        out
    }

    /// Where offset `old` is after the changes: an insert at it stays after it, a removed piece
    /// it was inside takes it to the piece's start.
    fn moved(&self, old: usize) -> usize {
        let mut new = old;
        for (offset, removed, inserted) in &self.list {
            if *offset >= old {
                break;
            }
            let gone = (*removed).min(old - offset);
            new = new - gone + inserted.len();
        }
        new
    }
}

/// Ctrl+/ (R-SQL-52): comments out the lines the selection touches with `-- ` at their least
/// indentation, or takes the comment away when every line that is not blank has one. `None`
/// when all the lines are blank.
pub fn toggle_comment(text: &str, selection: (usize, usize)) -> Option<Edit> {
    let range = lines(text, selection);
    let filled: Vec<(usize, &str)> = each_line(text, range.clone())
        .filter(|(_, l)| !l.trim().is_empty())
        .collect();
    if filled.is_empty() {
        return None;
    }
    let commented = filled
        .iter()
        .all(|(_, l)| l.trim_start_matches([' ', '\t']).starts_with("--"));
    let mut changes = Changes::new();
    if commented {
        for (start, line) in &filled {
            let at = start + indentation(line);
            let space = usize::from(
                line.get(indentation(line) + 2..)
                    .is_some_and(|r| r.starts_with(' ')),
            );
            changes.remove(at, 2 + space);
        }
    } else {
        let least = filled
            .iter()
            .map(|(_, l)| indentation(l))
            .min()
            .unwrap_or(0);
        for (start, _) in &filled {
            changes.insert(start + least, "-- ");
        }
    }
    let new = changes.apply(text);
    let end = range.end + new.len() - text.len();
    Some(Edit {
        text: new,
        selection: (range.start, end),
    })
}

/// Tab with a selection (R-SQL-53): `width` spaces, the indentation of Format SQL's style
/// (R-SQL-77), before each line it touches that is not empty; the lines stay selected.
pub fn indent(text: &str, selection: (usize, usize), width: usize) -> Edit {
    let range = lines(text, selection);
    let mut changes = Changes::new();
    for (start, line) in each_line(text, range.clone()) {
        if !line.is_empty() {
            changes.insert(start, &" ".repeat(level(width)));
        }
    }
    let new = changes.apply(text);
    let end = range.end + new.len() - text.len();
    Edit {
        text: new,
        selection: (range.start, end),
    }
}

/// Shift+Tab (R-SQL-53): up to `width` spaces (Format SQL's indentation, R-SQL-77), or one
/// tab, off the start of each line the selection touches. With a selection the lines stay
/// selected; a cursor stays where it was in its line. `None` when nothing was indented.
pub fn outdent(text: &str, selection: (usize, usize), width: usize) -> Option<Edit> {
    let (start, end) = clamp(text, selection);
    let range = lines(text, (start, end));
    let mut changes = Changes::new();
    for (at, line) in each_line(text, range.clone()) {
        let len = if line.starts_with('\t') {
            1
        } else {
            line.bytes()
                .take(level(width))
                .take_while(|&b| b == b' ')
                .count()
        };
        changes.remove(at, len);
    }
    if changes.list.is_empty() {
        return None;
    }
    let new = changes.apply(text);
    let selection = if start == end {
        let cursor = changes.moved(start);
        (cursor, cursor)
    } else {
        (range.start, range.end + new.len() - text.len())
    };
    Some(Edit {
        text: new,
        selection,
    })
}

/// Tab without a selection (R-SQL-53): spaces up to the next multiple of `width` columns,
/// Format SQL's indentation (R-SQL-77).
pub fn tab(text: &str, cursor: usize, width: usize) -> Edit {
    let width = level(width);
    let (cursor, _) = clamp(text, (cursor, cursor));
    let column = text
        .get(line_start_at(text, cursor)..cursor)
        .map_or(0, |t| t.chars().count());
    let spaces = " ".repeat(width - column % width);
    let mut new = text.to_owned();
    new.insert_str(cursor, &spaces);
    let after = cursor + spaces.len();
    Edit {
        text: new,
        selection: (after, after),
    }
}

/// Alt+↑ and Alt+↓ (R-SQL-54): the lines the selection touches swap with the line above or
/// below; the selection moves with them. `None` at the first or last line.
pub fn move_lines(text: &str, selection: (usize, usize), direction: Direction) -> Option<Edit> {
    let (start, end) = clamp(text, selection);
    let block = lines(text, (start, end));
    let moved = text.get(block.clone())?;
    let (new, shift): (String, isize) = match direction {
        Direction::Up => {
            if block.start == 0 {
                return None;
            }
            let above = line_start_at(text, block.start - 1);
            let other = text.get(above..block.start - 1)?;
            let new = format!(
                "{}{moved}\n{other}{}",
                text.get(..above)?,
                text.get(block.end..)?
            );
            (new, -isize::try_from(other.len() + 1).ok()?)
        }
        Direction::Down => {
            if block.end >= text.len() {
                return None;
            }
            let below = line_end_at(text, block.end + 1);
            let other = text.get(block.end + 1..below)?;
            let new = format!(
                "{}{other}\n{moved}{}",
                text.get(..block.start)?,
                text.get(below..)?
            );
            (new, isize::try_from(other.len() + 1).ok()?)
        }
    };
    let shifted = |at: usize| at.checked_add_signed(shift).unwrap_or(0);
    Some(Edit {
        selection: (shifted(start), shifted(end)),
        text: new,
    })
}

/// Ctrl+Shift+D (R-SQL-54): a copy of the lines the selection touches below them; the
/// selection moves onto the copy.
pub fn duplicate_lines(text: &str, selection: (usize, usize)) -> Edit {
    let (start, end) = clamp(text, selection);
    let block = lines(text, (start, end));
    let copy = text.get(block.clone()).unwrap_or_default();
    let mut new = text.to_owned();
    new.insert_str(block.end, &format!("\n{copy}"));
    let shift = copy.len() + 1;
    Edit {
        text: new,
        selection: (start + shift, end + shift),
    }
}

/// Whether `at` is inside a string, a delimited name or a comment: past its first character
/// and before its end, or at the end of one that is not closed.
fn in_literal(text: &str, at: usize) -> bool {
    text::highlight(text).iter().any(|(range, kind)| {
        let closed = || {
            let piece = text.get(range.clone()).unwrap_or_default();
            match kind {
                Kind::String => piece.len() >= 2 && piece.ends_with('\''),
                Kind::Name => piece.len() >= 2 && piece.ends_with('"'),
                Kind::Comment => {
                    piece.starts_with("/*") && piece.len() >= 4 && piece.ends_with("*/")
                }
                Kind::Keyword | Kind::Number => true,
            }
        };
        matches!(kind, Kind::String | Kind::Name | Kind::Comment)
            && range.start < at
            && (at < range.end || (at == range.end && !closed()))
    })
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// The closing partner of an opening `(`, `'` or `"`.
fn closer(open: char) -> Option<char> {
    match open {
        '(' => Some(')'),
        '\'' => Some('\''),
        '"' => Some('"'),
        _ => None,
    }
}

/// Typing `typed` (R-SQL-55): an opening `(`, `'` or `"` with its partner after it, or around
/// a selection; a closing one over the same character just after the cursor. `None` when the
/// character should go in as typed.
pub fn typed_pair(text: &str, selection: (usize, usize), typed: char) -> Option<Edit> {
    let (start, end) = clamp(text, selection);
    let next = text.get(end..).and_then(|t| t.chars().next());
    let previous = text.get(..start).and_then(|t| t.chars().next_back());
    let quote = typed == '\'' || typed == '"';
    // Over the closing one.
    if start == end && next == Some(typed) && (typed == ')' || quote) {
        let steps = if typed == ')' {
            !in_literal(text, start)
        } else {
            closes_here(text, start, typed)
        };
        if steps {
            let after = start + typed.len_utf8();
            return Some(Edit {
                text: text.to_owned(),
                selection: (after, after),
            });
        }
    }
    let close = closer(typed)?;
    if start < end {
        let inner = text.get(start..end)?;
        let new = format!(
            "{}{typed}{inner}{close}{}",
            text.get(..start)?,
            text.get(end..)?
        );
        return Some(Edit {
            text: new,
            selection: (start + 1, end + 1),
        });
    }
    // Only before a space, the end of a line, or what ends an expression.
    let room = next.is_none_or(|c| c.is_whitespace() || matches!(c, ')' | ',' | ';'));
    if !room || in_literal(text, start) || (quote && previous.is_some_and(is_word)) {
        return None;
    }
    let new = format!("{}{typed}{close}{}", text.get(..start)?, text.get(start..)?);
    Some(Edit {
        text: new,
        selection: (start + 1, start + 1),
    })
}

/// Whether the quote at `at` closes a string or name that is open before it.
fn closes_here(text: &str, at: usize, quote: char) -> bool {
    let kind = if quote == '\'' {
        Kind::String
    } else {
        Kind::Name
    };
    text::highlight(text)
        .iter()
        .any(|(range, k)| *k == kind && range.start < at && range.end == at + 1)
}

/// Backspace between an empty pair (R-SQL-55): both go. `None` otherwise.
pub fn backspace_pair(text: &str, cursor: usize) -> Option<Edit> {
    let (cursor, _) = clamp(text, (cursor, cursor));
    let previous = text.get(..cursor)?.chars().next_back()?;
    let next = text.get(cursor..)?.chars().next()?;
    if closer(previous) != Some(next) {
        return None;
    }
    let mut new = text.to_owned();
    new.replace_range((cursor - 1)..=cursor, "");
    Some(Edit {
        text: new,
        selection: (cursor - 1, cursor - 1),
    })
}

/// The bracket next to the cursor (just before it first, else just after) outside strings,
/// names and comments, and its partner if it has one (R-SQL-56).
pub fn matching_bracket(text: &str, cursor: usize) -> Option<(usize, Option<usize>)> {
    let bytes = text.as_bytes();
    let near = [cursor.checked_sub(1), Some(cursor)];
    let candidate = near
        .into_iter()
        .flatten()
        .find(|&i| matches!(bytes.get(i), Some(b'(' | b')')))?;
    let literals: Vec<Range<usize>> = text::highlight(text)
        .into_iter()
        .filter(|(_, k)| matches!(k, Kind::String | Kind::Name | Kind::Comment))
        .map(|(r, _)| r)
        .collect();
    let outside = |i: usize| !literals.iter().any(|r| r.contains(&i));
    if !outside(candidate) {
        return None;
    }
    let mut depth = 0usize;
    let partner = if bytes.get(candidate) == Some(&b'(') {
        (candidate..bytes.len()).find(|&i| {
            if !outside(i) {
                return false;
            }
            match bytes.get(i) {
                Some(b'(') => depth += 1,
                Some(b')') => depth = depth.saturating_sub(1),
                _ => return false,
            }
            depth == 0
        })
    } else {
        (0..=candidate).rev().find(|&i| {
            if !outside(i) {
                return false;
            }
            match bytes.get(i) {
                Some(b')') => depth += 1,
                Some(b'(') => depth = depth.saturating_sub(1),
                _ => return false,
            }
            depth == 0
        })
    };
    Some((candidate, partner))
}

/// Keywords in `range` of `text` in upper or lower case (R-SQL-58); names, strings and
/// comments as they are. Keywords are ASCII, so offsets do not move.
pub fn keyword_case(text: &str, range: Range<usize>, case: Case) -> String {
    let mut out = text.to_owned();
    for (span, kind) in text::highlight(text) {
        if kind != Kind::Keyword || span.start < range.start || span.end > range.end {
            continue;
        }
        let Some(word) = text.get(span.clone()) else {
            continue;
        };
        let changed = match case {
            Case::Upper => word.to_ascii_uppercase(),
            Case::Lower => word.to_ascii_lowercase(),
        };
        out.replace_range(span, &changed);
    }
    out
}

/// The unquoted name around byte `at` (the cursor may be just after it); `None` off a name or
/// on a number.
fn word_at(text: &str, at: usize) -> Option<Range<usize>> {
    let (at, _) = clamp(text, (at, at));
    let start = text
        .get(..at)?
        .char_indices()
        .rev()
        .take_while(|&(_, c)| is_name_char(c))
        .last()
        .map_or(at, |(i, _)| i);
    let end = text
        .get(at..)?
        .char_indices()
        .find(|&(_, c)| !is_name_char(c))
        .map_or(text.len(), |(i, _)| at + i);
    let word = text.get(start..end)?;
    (!word.is_empty() && !word.starts_with(|c: char| c.is_ascii_digit())).then_some(start..end)
}

/// Whether `a` and `b` are the same text, ignoring case.
fn same_ignoring_case(a: &str, b: &str) -> bool {
    a.chars()
        .flat_map(char::to_lowercase)
        .eq(b.chars().flat_map(char::to_lowercase))
}

/// The strings, delimited names and comments of `text`.
fn literals(text: &str) -> Vec<(Range<usize>, Kind)> {
    text::highlight(text)
        .into_iter()
        .filter(|(_, k)| matches!(k, Kind::String | Kind::Name | Kind::Comment))
        .collect()
}

/// Every place of the name under the cursor at `cursor` (R-SQL-66): unquoted names in any case
/// and only whole, quoted ones exactly; none in strings or comments. Empty when the name is
/// there only once, or the cursor is not on a name.
pub fn occurrences(text: &str, cursor: usize) -> Vec<Range<usize>> {
    let literals = literals(text);
    let inside = |at: usize| literals.iter().find(|(r, _)| r.start < at && at < r.end);
    if let Some((range, kind)) = inside(cursor) {
        if *kind != Kind::Name {
            return Vec::new();
        }
        let name = text.get(range.clone()).unwrap_or_default();
        let found: Vec<Range<usize>> = literals
            .iter()
            .filter(|(r, k)| *k == Kind::Name && text.get(r.clone()) == Some(name))
            .map(|(r, _)| r.clone())
            .collect();
        return if found.len() > 1 { found } else { Vec::new() };
    }
    let Some(word) = word_at(text, cursor) else {
        return Vec::new();
    };
    if literals.iter().any(|(r, _)| r.contains(&word.start)) {
        return Vec::new();
    }
    let name = text.get(word).unwrap_or_default();
    let mut found = Vec::new();
    let mut at = 0;
    while at < text.len() {
        let Some(rest) = text.get(at..) else { break };
        let Some(c) = rest.chars().next() else { break };
        let starts_name = c.is_alphabetic() || matches!(c, '_' | '#' | '$' | '@');
        let fresh = !text
            .get(..at)
            .and_then(|b| b.chars().next_back())
            .is_some_and(is_name_char);
        if starts_name && fresh {
            let end = rest
                .char_indices()
                .find(|&(_, c)| !is_name_char(c))
                .map_or(text.len(), |(i, _)| at + i);
            let outside = !literals.iter().any(|(r, _)| r.contains(&at));
            if outside
                && text
                    .get(at..end)
                    .is_some_and(|w| same_ignoring_case(w, name))
            {
                found.push(at..end);
            }
            at = end;
        } else {
            at += c.len_utf8();
        }
    }
    if found.len() > 1 { found } else { Vec::new() }
}

/// Ctrl+D (R-SQL-67): the name under the cursor without a selection; with one, the next place
/// of the same text after it, from the top again after the last, ignoring case.
pub fn select_next(text: &str, selection: (usize, usize)) -> Option<(usize, usize)> {
    let (start, end) = clamp(text, selection);
    if start == end {
        let word = word_at(text, start)?;
        return Some((word.start, word.end));
    }
    let wanted = text.get(start..end)?;
    let length = wanted.chars().count();
    let matches_at = |at: usize| -> Option<usize> {
        let rest = text.get(at..)?;
        let piece_end = rest
            .char_indices()
            .nth(length)
            .map_or(text.len(), |(i, _)| at + i);
        let piece = text.get(at..piece_end)?;
        (piece.chars().count() == length && same_ignoring_case(piece, wanted)).then_some(piece_end)
    };
    let starts = text.char_indices().map(|(i, _)| i);
    let after = starts.clone().filter(|&i| i >= end);
    let before = starts.filter(|&i| i < end);
    after
        .chain(before)
        .find_map(|i| matches_at(i).map(|e| (i, e)))
}

/// Save (R-SQL-68): `text` without spaces and tabs at the ends of its lines, but in a string
/// that goes on to the next line; and where byte `cursor` is afterwards.
pub fn trim_trailing(text: &str, cursor: usize) -> (String, usize) {
    let strings: Vec<Range<usize>> = literals(text)
        .into_iter()
        .filter(|(_, k)| *k == Kind::String)
        .map(|(r, _)| r)
        .collect();
    let mut changes = Changes::new();
    let mut at = 0;
    for line in text.split('\n') {
        let end = at + line.len();
        let kept = line.trim_end_matches([' ', '\t']).len();
        let in_string = strings.iter().any(|r| r.start < end && end < r.end);
        if kept < line.len() && !in_string {
            changes.remove(at + kept, line.len() - kept);
        }
        at = end + 1;
    }
    let (cursor, _) = clamp(text, (cursor, cursor));
    (changes.apply(text), changes.moved(cursor))
}

/// How many lines `text` has (an empty text has one).
pub fn line_count(text: &str) -> usize {
    text.matches('\n').count() + 1
}

/// Where line `number` (from 1) starts; past the end, the last line (R-SQL-57).
pub fn line_start(text: &str, number: usize) -> usize {
    if number <= 1 {
        return 0;
    }
    text.match_indices('\n')
        .nth(number - 2)
        .map_or_else(|| line_start_at(text, text.len()), |(i, _)| i + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `text` with `|` for the cursor or `[` `]` around the selection, as (text, selection).
    fn marked(marked: &str) -> (String, (usize, usize)) {
        if let Some(at) = marked.find('|') {
            return (marked.replacen('|', "", 1), (at, at));
        }
        let start = marked.find('[').expect("a [ in the test text");
        let end = marked.find(']').expect("a ] in the test text") - 1;
        (
            marked.replacen('[', "", 1).replacen(']', "", 1),
            (start, end),
        )
    }

    /// The edit shown the same way.
    fn shown(edit: &Edit) -> String {
        let (a, b) = edit.selection;
        let mut out = edit.text.clone();
        if a == b {
            out.insert(a, '|');
        } else {
            out.insert(b, ']');
            out.insert(a, '[');
        }
        out
    }

    #[test]
    fn ctrl_slash_comments_the_lines_at_their_least_indentation_r_sql_52() {
        let (text, sel) = marked("SELECT [a,\n    b\n\nFROM] t");
        let edit = toggle_comment(&text, sel).expect("lines to comment");
        assert_eq!(shown(&edit), "[-- SELECT a,\n--     b\n\n-- FROM t]");
    }

    #[test]
    fn ctrl_slash_takes_the_comment_away_when_every_line_has_one_r_sql_52() {
        let (text, sel) = marked("  -- æøå|\n  --x");
        let edit = toggle_comment(&text, sel).expect("a line to uncomment");
        assert_eq!(edit.text, "  æøå\n  --x");
        let (text, sel) = marked("[  -- æøå\n  --x]");
        let edit = toggle_comment(&text, sel).expect("lines to uncomment");
        assert_eq!(shown(&edit), "[  æøå\n  x]");
    }

    #[test]
    fn a_selection_ending_at_a_line_start_leaves_that_line_r_sql_52() {
        let (text, sel) = marked("[a\n]b");
        let edit = toggle_comment(&text, sel).expect("a line");
        assert_eq!(edit.text, "-- a\nb");
    }

    #[test]
    fn blank_lines_alone_are_not_commented_r_sql_52() {
        assert_eq!(toggle_comment("  \n", (0, 0)), None);
        assert_eq!(toggle_comment("", (0, 0)), None);
    }

    #[test]
    fn tab_indents_the_selected_lines_and_shift_tab_takes_it_back_r_sql_53() {
        let (text, sel) = marked("SE[LECT\n\nFROM] t");
        let edit = indent(&text, sel, 4);
        assert_eq!(shown(&edit), "[    SELECT\n\n    FROM t]");
        let back = outdent(&edit.text, edit.selection, 4).expect("indented lines");
        assert_eq!(shown(&back), "[SELECT\n\nFROM t]");
    }

    #[test]
    fn shift_tab_on_a_cursor_keeps_its_place_in_the_line_r_sql_53() {
        let (text, sel) = marked("x\n      ab|c");
        let edit = outdent(&text, sel, 4).expect("an indented line");
        assert_eq!(shown(&edit), "x\n  ab|c");
        let (text, sel) = marked("\tæ|");
        assert_eq!(shown(&outdent(&text, sel, 4).expect("a tab")), "æ|");
        assert_eq!(outdent("abc", (1, 1), 4), None);
    }

    #[test]
    fn tab_without_a_selection_goes_to_the_next_stop_counting_characters_r_sql_53() {
        let (text, sel) = marked("æø|");
        assert_eq!(shown(&tab(&text, sel.0, 4)), "æø  |");
        let (text, sel) = marked("abcd|");
        assert_eq!(shown(&tab(&text, sel.0, 4)), "abcd    |");
    }

    #[test]
    fn tab_and_shift_tab_follow_the_indentation_of_the_format_style_r_sql_77() {
        let (text, sel) = marked("[SELECT\nFROM] t");
        let edit = indent(&text, sel, 2);
        assert_eq!(shown(&edit), "[  SELECT\n  FROM t]");
        let (text, sel) = marked("x\n    æ|");
        assert_eq!(shown(&outdent(&text, sel, 2).expect("indented")), "x\n  æ|");
        let (text, sel) = marked("abc|");
        assert_eq!(shown(&tab(&text, sel.0, 2)), "abc |");
        assert_eq!(shown(&tab(&text, sel.0, 0)), "abc |", "at least one space");
    }

    #[test]
    fn alt_arrows_move_the_lines_and_the_selection_with_them_r_sql_54() {
        let (text, sel) = marked("one\ntw|o\nthree");
        let up = move_lines(&text, sel, Direction::Up).expect("a line above");
        assert_eq!(shown(&up), "tw|o\none\nthree");
        let down = move_lines(&text, sel, Direction::Down).expect("a line below");
        assert_eq!(shown(&down), "one\nthree\ntw|o");
        assert_eq!(move_lines(&up.text, up.selection, Direction::Up), None);
        assert_eq!(
            move_lines(&down.text, down.selection, Direction::Down),
            None
        );
        let (text, sel) = marked("æ\n[b\nc]\nd");
        let up = move_lines(&text, sel, Direction::Up).expect("a line above");
        assert_eq!(shown(&up), "[b\nc]\næ\nd");
    }

    #[test]
    fn ctrl_shift_d_copies_the_lines_below_and_moves_onto_the_copy_r_sql_54() {
        let (text, sel) = marked("a\nb|æ\nc");
        assert_eq!(shown(&duplicate_lines(&text, sel)), "a\nbæ\nb|æ\nc");
        assert_eq!(shown(&duplicate_lines("", (0, 0))), "\n|");
    }

    #[test]
    fn an_opening_bracket_or_quote_brings_its_partner_r_sql_55() {
        let (text, sel) = marked("SELECT |");
        assert_eq!(
            shown(&typed_pair(&text, sel, '(').expect("a pair")),
            "SELECT (|)"
        );
        assert_eq!(
            shown(&typed_pair(&text, sel, '\'').expect("a pair")),
            "SELECT '|'"
        );
        // Not before a word, not after one for quotes, not inside a string or comment.
        assert_eq!(
            typed_pair("|abc".replace('|', "").as_str(), (0, 0), '('),
            None
        );
        assert_eq!(typed_pair("X", (1, 1), '\''), None);
        assert_eq!(typed_pair("'æø ", (6, 6), '('), None);
        assert_eq!(typed_pair("-- x ", (5, 5), '"'), None);
        // Around a selection.
        let (text, sel) = marked("a [b + c] d");
        assert_eq!(
            shown(&typed_pair(&text, sel, '(').expect("a pair")),
            "a ([b + c]) d"
        );
    }

    #[test]
    fn a_closing_one_steps_over_its_partner_r_sql_55() {
        let (text, sel) = marked("(a|)");
        assert_eq!(shown(&typed_pair(&text, sel, ')').expect("a step")), "(a)|");
        let (text, sel) = marked("'æ|'");
        assert_eq!(
            shown(&typed_pair(&text, sel, '\'').expect("a step")),
            "'æ'|"
        );
        // A quote that opens the next string is not stepped over.
        let (text, sel) = marked("x = |'a'");
        assert_eq!(typed_pair(&text, sel, '\''), None);
        assert_eq!(typed_pair("a", (1, 1), ')'), None);
    }

    #[test]
    fn backspace_takes_an_empty_pair_r_sql_55() {
        let (text, sel) = marked("f(|)");
        assert_eq!(shown(&backspace_pair(&text, sel.0).expect("a pair")), "f|");
        assert_eq!(backspace_pair("f(x)", 2), None);
        assert_eq!(backspace_pair("", 0), None);
    }

    #[test]
    fn the_bracket_at_the_cursor_finds_its_partner_outside_strings_r_sql_56() {
        let text = "f(a, ')', (b))";
        assert_eq!(matching_bracket(text, 2), Some((1, Some(13))));
        assert_eq!(matching_bracket(text, 14), Some((13, Some(1))));
        assert_eq!(matching_bracket(text, 11), Some((10, Some(12))));
        assert_eq!(matching_bracket(text, 7), None, "inside a string");
        assert_eq!(matching_bracket("((a)", 1), Some((0, None)));
        assert_eq!(matching_bracket("abc", 1), None);
        assert_eq!(matching_bracket("", 0), None);
    }

    #[test]
    fn keyword_case_changes_keywords_only_and_only_in_the_range_r_sql_58() {
        let text = "select navn from \"from\" where x = 'select'; select 1";
        let first = 0..text.find(';').expect("a semicolon");
        assert_eq!(
            keyword_case(text, first, Case::Upper),
            "SELECT navn FROM \"from\" WHERE x = 'select'; select 1"
        );
        assert_eq!(
            keyword_case("SELECT æ FROM t", 0..16, Case::Lower),
            "select æ from t"
        );
    }

    #[test]
    fn lines_are_found_by_number_and_past_the_end_is_the_last_r_sql_57() {
        let text = "a\næ\nc";
        assert_eq!(line_count(text), 3);
        assert_eq!(line_start(text, 1), 0);
        assert_eq!(line_start(text, 3), 5);
        assert_eq!(line_start(text, 99), 5);
        assert_eq!(line_count(""), 1);
        assert_eq!(line_start("", 4), 0);
    }

    #[test]
    fn every_place_of_the_name_under_the_cursor_is_found_r_sql_66() {
        let text = "SELECT navn, Navn2 FROM kunder k WHERE k.NAVN = 'navn' -- navn\n AND æøå = ÆØÅ";
        let found: Vec<&str> = occurrences(text, 9).into_iter().map(|r| &text[r]).collect();
        assert_eq!(found, ["navn", "NAVN"]);
        let at = text.find("æøå").expect("there") + 2;
        let found: Vec<&str> = occurrences(text, at)
            .into_iter()
            .map(|r| &text[r])
            .collect();
        assert_eq!(found, ["æøå", "ÆØÅ"]);
        assert!(
            occurrences(text, text.find("'navn'").expect("there") + 2).is_empty(),
            "a string"
        );
        assert!(
            occurrences(text, text.find("kunder").expect("there") + 1).is_empty(),
            "once"
        );
        let quoted = "SELECT \"a b\", \"a b\", \"A B\" FROM t";
        assert_eq!(occurrences(quoted, 9).len(), 2);
        assert_eq!(occurrences("", 0), Vec::<Range<usize>>::new());
    }

    #[test]
    fn ctrl_d_takes_the_word_then_the_next_place_and_wraps_r_sql_67() {
        let text = "a navn, b Navn, c NAVN";
        assert_eq!(select_next(text, (4, 4)), Some((2, 6)));
        assert_eq!(select_next(text, (2, 6)), Some((10, 14)));
        assert_eq!(select_next(text, (18, 22)), Some((2, 6)));
        assert_eq!(select_next("æø æø", (0, 4)), Some((5, 9)));
        assert_eq!(select_next("a + b", (2, 2)), None);
    }

    #[test]
    fn trailing_blanks_go_but_not_inside_a_string_r_sql_68() {
        let text = "SELECT 'a  \nb' \t\nFROM t   \næ  ";
        let cursor = text.find("FROM").expect("there") + 6;
        let (trimmed, at) = trim_trailing(text, cursor);
        assert_eq!(trimmed, "SELECT 'a  \nb'\nFROM t\næ");
        assert_eq!(
            &trimmed[at..=at],
            "\n",
            "the cursor stays at its line's end"
        );
        assert_eq!(trim_trailing("", 0), (String::new(), 0));
    }
}
