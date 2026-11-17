//! Shell integration (Phase 35): links (OSC 8, R-SSH-44, and addresses in the text), the shell's prompts and
//! commands as marks (OSC 133, R-SSH-45, the `FinalTerm` sequences bash, zsh, fish and PowerShell send with their
//! integration), notifications (OSC 9 and OSC 777, R-SSH-46), and the clipboard set by the host (OSC 52,
//! R-SSH-47; never read). Sans-I/O: what the UI must act on comes out of [`Terminal::take_shell_events`].

use super::{Buffer, Point, Terminal, Width};
use crate::term::Found;

/// The most links one terminal names (OSC 8); later ones are text.
const MAX_LINKS: usize = 4096;
/// The longest link kept.
const MAX_LINK: usize = 2048;
/// The most text the host may put on the clipboard at once.
const MAX_CLIPBOARD: usize = 100 * 1024;
/// The longest notification text kept.
const MAX_NOTE: usize = 512;

/// What the UI must act on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellEvent {
    /// A command started (OSC 133;C).
    CommandStarted,
    /// A command ended, with its exit status when the shell said it (OSC 133;D).
    CommandEnded(Option<i32>),
    /// A notification from the host (OSC 9, OSC 777;notify).
    Notify { title: String, body: String },
    /// The host set the clipboard (OSC 52).
    Clipboard(String),
}

/// A mark the shell set, at an absolute line (counting the lines dropped from the scrollback).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Mark {
    line: u64,
    kind: MarkKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MarkKind {
    Prompt,
    Output,
    End(Option<i32>),
}

/// One command as the marks show it, lines counted as [`Point`]s.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandMark {
    /// Where its prompt starts.
    pub prompt: usize,
    /// Where its output starts, once it ran.
    pub output: Option<usize>,
    /// The line it ended on, once it ended.
    pub end: Option<usize>,
    pub exit: Option<i32>,
}

impl CommandMark {
    /// Its output, from the first line to the last, when it has any.
    pub fn output_lines(&self) -> Option<(usize, usize)> {
        let from = self.output?;
        let to = self.end.map_or(from, |end| end.saturating_sub(1).max(from));
        Some((from, to))
    }

    /// Whether it failed: it ended with a status other than 0.
    pub fn failed(&self) -> bool {
        self.exit.is_some_and(|e| e != 0)
    }
}

impl Terminal {
    /// OSC 8, 9, 52, 133 and 777; false for any other.
    pub(super) fn shell_osc(&mut self, params: &[&[u8]]) -> bool {
        let Some((kind, rest)) = params.split_first() else {
            return false;
        };
        match *kind {
            b"8" => self.hyperlink(rest),
            b"9" => {
                // ConEmu's OSC 9;1…;12 are other things (progress, sleep): a notification is text.
                let body = text_of(rest);
                if !body.is_empty() && !body.starts_with(|c: char| c.is_ascii_digit()) {
                    self.shell_events.push(ShellEvent::Notify {
                        title: String::new(),
                        body: short(&body, MAX_NOTE),
                    });
                }
            }
            b"777" => {
                if let [b"notify", title, body @ ..] = rest {
                    self.shell_events.push(ShellEvent::Notify {
                        title: short(&String::from_utf8_lossy(title), MAX_NOTE),
                        body: short(&text_of(body), MAX_NOTE),
                    });
                }
            }
            b"52" => self.clipboard(rest),
            b"133" => self.shell_mark(rest),
            _ => return false,
        }
        true
    }

    /// OSC 8 ; params ; URI: the following text links to URI; an empty URI ends the link.
    fn hyperlink(&mut self, rest: &[&[u8]]) {
        let uri = rest
            .get(1..)
            .map(|parts| String::from_utf8_lossy(&parts.join(&b';')).into_owned());
        self.cursor_link = match uri {
            Some(uri)
                if !uri.is_empty()
                    && uri.len() <= MAX_LINK
                    && !uri.chars().any(char::is_control) =>
            {
                match self.links.iter().position(|l| *l == uri) {
                    Some(i) => u16::try_from(i + 1).unwrap_or(0),
                    None if self.links.len() < MAX_LINKS => {
                        self.links.push(uri);
                        u16::try_from(self.links.len()).unwrap_or(0)
                    }
                    None => 0,
                }
            }
            _ => 0,
        };
    }

    /// OSC 52 ; selection ; base64: the host sets the clipboard. A query (`?`) is never answered.
    fn clipboard(&mut self, rest: &[&[u8]]) {
        let Some(data) = rest.get(1) else { return };
        if *data == b"?" || data.len() > MAX_CLIPBOARD * 4 / 3 + 4 {
            return;
        }
        if let Ok(bytes) = data_encoding::BASE64.decode(data)
            && let Ok(text) = String::from_utf8(bytes)
        {
            self.shell_events.push(ShellEvent::Clipboard(text));
        }
    }

    /// OSC 133 ; A (prompt), B (command typed after it), C (output), D [; status] (ended).
    fn shell_mark(&mut self, rest: &[&[u8]]) {
        if self.buffer != Buffer::Main {
            return;
        }
        let line = self.dropped + (self.history.len() + self.cursor.row) as u64;
        let kind = match rest.first().copied() {
            Some(b"A") => MarkKind::Prompt,
            Some(b"C") => {
                self.shell_events.push(ShellEvent::CommandStarted);
                MarkKind::Output
            }
            Some(b"D") => {
                let exit = rest
                    .get(1)
                    .and_then(|s| std::str::from_utf8(s).ok())
                    .and_then(|s| s.parse().ok());
                self.shell_events.push(ShellEvent::CommandEnded(exit));
                MarkKind::End(exit)
            }
            _ => return,
        };
        self.marks.push(Mark { line, kind });
        // Marks of lines dropped from the scrollback go too.
        let dropped = self.dropped;
        self.marks.retain(|m| m.line >= dropped);
    }

    /// What happened since the last call that the UI must act on.
    pub fn take_shell_events(&mut self) -> Vec<ShellEvent> {
        std::mem::take(&mut self.shell_events)
    }

    /// The commands the shell marked, oldest first, of the lines still kept.
    pub fn commands(&self) -> Vec<CommandMark> {
        let point = |abs: u64| usize::try_from(abs.checked_sub(self.dropped)?).ok();
        let mut commands: Vec<CommandMark> = Vec::new();
        for mark in &self.marks {
            let Some(line) = point(mark.line) else {
                continue;
            };
            match mark.kind {
                MarkKind::Prompt => commands.push(CommandMark {
                    prompt: line,
                    output: None,
                    end: None,
                    exit: None,
                }),
                MarkKind::Output => {
                    if let Some(c) = commands.last_mut().filter(|c| c.output.is_none()) {
                        c.output = Some(line);
                    }
                }
                MarkKind::End(exit) => {
                    if let Some(c) = commands.last_mut().filter(|c| c.end.is_none()) {
                        c.end = Some(line);
                        c.exit = exit;
                    }
                }
            }
        }
        commands
    }

    /// The link at `point`: one the host named (OSC 8), else an address written in the text; with its cells.
    pub fn link_at(&self, point: Point) -> Option<(Found, String)> {
        let row = self.line(point.line)?;
        let cell = row.cells.get(point.column)?;
        if cell.link != 0 {
            let uri = self.links.get(usize::from(cell.link) - 1)?.clone();
            let same = |c: usize| row.cells.get(c).is_some_and(|x| x.link == cell.link);
            let mut from = point.column;
            while from > 0 && same(from - 1) {
                from -= 1;
            }
            let mut to = point.column;
            while same(to + 1) {
                to += 1;
            }
            let found = Found {
                from: Point {
                    line: point.line,
                    column: from,
                },
                to: Point {
                    line: point.line,
                    column: to,
                },
            };
            return Some((found, uri));
        }
        written_link(row, point)
    }
}

/// An address written in the row around `point`: `http://`, `https://`, `ftp://` or `file://`, up to a blank, less
/// the punctuation that ends a sentence and a closing bracket without its opening one.
fn written_link(row: &super::Row, point: Point) -> Option<(Found, String)> {
    let shown: Vec<(char, usize)> = row
        .cells
        .iter()
        .enumerate()
        .filter(|(_, c)| c.width != Width::Spacer)
        .map(|(i, c)| (c.ch, i))
        .collect();
    let at = shown
        .iter()
        .position(|(_, column)| *column >= point.column)?;
    if shown[at].1 != point.column || shown[at].0.is_whitespace() {
        return None;
    }
    let is_part = |c: char| !c.is_whitespace() && !matches!(c, '"' | '\'' | '<' | '>' | '`');
    let mut start = at;
    while start > 0 && is_part(shown[start - 1].0) {
        start -= 1;
    }
    let mut end = at;
    while end + 1 < shown.len() && is_part(shown[end + 1].0) {
        end += 1;
    }
    let word: String = shown[start..=end].iter().map(|(c, _)| *c).collect();
    let scheme = ["https://", "http://", "ftp://", "file://"]
        .iter()
        .filter_map(|s| word.find(s))
        .min()?;
    let mut url: Vec<char> = word.chars().skip(scheme).collect();
    loop {
        match url.last() {
            Some('.' | ',' | ';' | ':' | '!' | '?') => {
                url.pop();
            }
            Some(')')
                if url.iter().filter(|c| **c == '(').count()
                    < url.iter().filter(|c| **c == ')').count() =>
            {
                url.pop();
            }
            Some(']') if !url.contains(&'[') => {
                url.pop();
            }
            _ => break,
        }
    }
    let first = start + word[..scheme].chars().count();
    let last = first + url.len().checked_sub(1)?;
    if url.len() <= "http://".len() || at < first || at > last {
        return None;
    }
    let to_column = shown[last].1;
    let to_column = match row.cells.get(to_column) {
        Some(c) if c.width == Width::Wide => to_column + 1,
        _ => to_column,
    };
    let found = Found {
        from: Point {
            line: point.line,
            column: shown[first].1,
        },
        to: Point {
            line: point.line,
            column: to_column,
        },
    };
    Some((found, url.into_iter().collect()))
}

/// Parameters joined back with `;`, as the text they were.
fn text_of(parts: &[&[u8]]) -> String {
    String::from_utf8_lossy(&parts.join(&b';')).into_owned()
}

/// At most `limit` characters, without control characters.
fn short(text: &str, limit: usize) -> String {
    text.chars()
        .filter(|c| !c.is_control())
        .take(limit)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn term(text: &str) -> Terminal {
        let mut t = Terminal::new(80, 6);
        t.feed(text.as_bytes());
        t
    }

    fn point(line: usize, column: usize) -> Point {
        Point { line, column }
    }

    #[test]
    fn a_named_link_covers_its_text_and_ends_with_an_empty_uri_r_ssh_44() {
        let t = term("see \x1b]8;id=1;https://ex.dk/æ\x1b\\the docs\x1b]8;;\x1b\\ now");
        let (found, uri) = t.link_at(point(0, 6)).expect("a link");
        assert_eq!(uri, "https://ex.dk/æ");
        assert_eq!((found.from.column, found.to.column), (4, 11));
        assert_eq!(t.link_at(point(0, 13)), None, "after it");
        assert_eq!(t.link_at(point(0, 0)), None);
    }

    #[test]
    fn written_addresses_are_links_without_the_punctuation_after_them_r_ssh_44() {
        let t = term("Go to (https://ex.dk/a_(b)). Or http://x.dk/p?q=1, ftp:// no");
        let (found, url) = t.link_at(point(0, 10)).expect("a link");
        assert_eq!(url, "https://ex.dk/a_(b)");
        assert_eq!((found.from.column, found.to.column), (7, 25));
        let (_, url) = t.link_at(point(0, 34)).expect("a link");
        assert_eq!(url, "http://x.dk/p?q=1");
        assert_eq!(t.link_at(point(0, 52)), None, "a scheme alone");
        assert_eq!(t.link_at(point(0, 2)), None, "plain text");
    }

    #[test]
    fn prompts_and_commands_are_marked_with_their_exit_status_r_ssh_45() {
        let mut t = term(
            "\x1b]133;A\x07$ \x1b]133;B\x07ls\r\n\x1b]133;C\x07a\r\nb\r\n\x1b]133;D;0\x07\x1b]133;A\x07$ \x1b]133;B\x07false\r\n\x1b]133;C\x07\x1b]133;D;1\x07",
        );
        let commands = t.commands();
        assert_eq!(commands.len(), 2);
        assert_eq!(commands[0].prompt, 0);
        assert_eq!(commands[0].output_lines(), Some((1, 2)));
        assert!(!commands[0].failed());
        assert_eq!(commands[1].prompt, 3);
        assert!(commands[1].failed());
        let events = t.take_shell_events();
        assert_eq!(
            events,
            [
                ShellEvent::CommandStarted,
                ShellEvent::CommandEnded(Some(0)),
                ShellEvent::CommandStarted,
                ShellEvent::CommandEnded(Some(1)),
            ]
        );
        assert_eq!(t.take_shell_events(), []);
    }

    #[test]
    fn marks_of_dropped_lines_go_and_the_alternate_screen_has_none_r_ssh_45() {
        let mut t = Terminal::new(10, 2);
        t.set_history_limit(1);
        t.feed(b"\x1b]133;A\x07one\r\ntwo\r\nthree\r\n\x1b]133;A\x07four");
        assert_eq!(
            t.commands().iter().map(|c| c.prompt).collect::<Vec<_>>(),
            [2]
        );
        t.feed(b"\x1b[?1049h\x1b]133;A\x07");
        assert_eq!(t.commands().len(), 1);
    }

    #[test]
    fn notifications_come_from_osc_9_and_777_but_not_conemus_numbers_r_ssh_46() {
        let mut t = term(
            "\x1b]9;Build done æøå\x07\x1b]9;4;1;50\x07\x1b]777;notify;Make;all; done\x07\x1b]777;other\x07",
        );
        assert_eq!(
            t.take_shell_events(),
            [
                ShellEvent::Notify {
                    title: String::new(),
                    body: "Build done æøå".into()
                },
                ShellEvent::Notify {
                    title: "Make".into(),
                    body: "all; done".into()
                },
            ]
        );
    }

    #[test]
    fn the_host_may_set_the_clipboard_but_never_read_it_r_ssh_47() {
        let mut t = term("\x1b]52;c;aGVqIMOmw7jDpQ==\x07\x1b]52;c;?\x07\x1b]52;c;!!\x07");
        assert_eq!(
            t.take_shell_events(),
            [ShellEvent::Clipboard("hej æøå".into())]
        );
        assert_eq!(t.take_replies(), b"", "nothing is answered");
    }
}
