//! Keyboard macros (R-KEY-3, R-KEY-4): what a macro is and its text format (D14).
//!
//! A macro file is UTF-8 text, one step per line; blank lines and lines starting with `#` are
//! ignored:
//!
//! ```text
//! # Parley macro
//! session "PUB400"
//! text "wrkactjob"
//! enter
//! wait
//! password
//! f3
//! ```
//!
//! `session "NAME"` (optional, before the steps) keeps the macro to one saved session.
//! `text "…"` types text (`\"` and `\\` inside the quotes). `wait` waits until the host has
//! answered and unlocked the keyboard. `password` types the session's password. `move ROW
//! COLUMN` (1-based) puts the cursor there. Any other line is a key name: see [`KEY_NAMES`].
//! Recording and playing are on [`crate::Session`].

use crate::error::{Error, Result};
use crate::session::{Aid, Key};

/// One thing a macro does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// Typed characters.
    Text(String),
    /// A key other than a typed character.
    Key(Key),
    /// Wait until the host has answered and the keyboard is unlocked.
    Wait,
    /// Type the session's password; it is never stored in the macro.
    Password,
}

/// A recorded or hand-written macro. Its name is the file's, so it is not part of it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Macro {
    /// The saved session the macro belongs to; `None` for every session.
    pub session: Option<String>,
    pub steps: Vec<Step>,
}

/// The names of the keys a macro line can hold, beside `f1` to `f24`.
pub const KEY_NAMES: [(&str, Key); 25] = [
    ("enter", Key::Aid(Aid::ENTER)),
    ("help", Key::Aid(Aid::HELP)),
    ("roll-down", Key::Aid(Aid::ROLL_DOWN)),
    ("roll-up", Key::Aid(Aid::ROLL_UP)),
    ("print", Key::Aid(Aid::PRINT)),
    ("record-backspace", Key::Aid(Aid::RECORD_BACKSPACE)),
    ("clear", Key::Aid(Aid::CLEAR)),
    ("field-exit", Key::FieldExit),
    ("field-plus", Key::FieldPlus),
    ("field-minus", Key::FieldMinus),
    ("tab", Key::Tab),
    ("back-tab", Key::BackTab),
    ("up", Key::Up),
    ("down", Key::Down),
    ("left", Key::Left),
    ("right", Key::Right),
    ("home", Key::Home),
    ("end", Key::End),
    ("backspace", Key::Backspace),
    ("delete", Key::Delete),
    ("insert", Key::ToggleInsert),
    ("erase-eof", Key::EraseEof),
    ("reset", Key::Reset),
    ("attention", Key::Attention),
    ("system-request", Key::SystemRequest),
];

impl Macro {
    /// Reads a macro file. The error names the line that is wrong.
    pub fn parse(text: &str) -> Result<Self> {
        let mut parsed = Self::default();
        for (number, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let bad = |why: &str| Error::Macro(format!("line {}: {why}", number + 1));
            let (word, rest) = line.split_once(' ').unwrap_or((line, ""));
            let rest = rest.trim();
            let step = match word {
                "session" if parsed.steps.is_empty() && parsed.session.is_none() => {
                    parsed.session = Some(quoted(rest).ok_or_else(|| bad("expected \"name\""))?);
                    continue;
                }
                "text" => Step::Text(quoted(rest).ok_or_else(|| bad("expected \"text\""))?),
                "wait" if rest.is_empty() => Step::Wait,
                "password" if rest.is_empty() => Step::Password,
                "move" => Step::Key(move_to(rest).ok_or_else(|| bad("expected move ROW COLUMN"))?),
                _ if rest.is_empty() => {
                    Step::Key(key_named(word).ok_or_else(|| bad(&format!("unknown key {word}")))?)
                }
                _ => return Err(bad(&format!("unknown step {line}"))),
            };
            parsed.steps.push(step);
        }
        Ok(parsed)
    }

    /// The macro as file text, which [`Self::parse`] reads back.
    pub fn to_text(&self) -> String {
        let mut lines = vec!["# Parley macro: one step per line (see Tools ▸ Macros…).".to_owned()];
        lines.extend(
            self.session
                .as_ref()
                .map(|s| format!("session {}", quote(s))),
        );
        lines.extend(self.steps.iter().map(|step| match step {
            Step::Text(text) => format!("text {}", quote(text)),
            Step::Key(key) => key_name(*key),
            Step::Wait => "wait".to_owned(),
            Step::Password => "password".to_owned(),
        }));
        lines.push(String::new());
        lines.join("\n")
    }
}

/// The macro-file line of a key; typed characters are `text` steps, not keys. An AID key
/// without a name is written `aid-XX` (its code in hex), which [`key_named`] reads back.
fn key_name(key: Key) -> String {
    match key {
        Key::MoveTo { row, column } => format!("move {} {}", row + 1, column + 1),
        Key::Char(c) => format!("text {}", quote(&c.to_string())),
        Key::Aid(aid) => {
            key_name_of(key).unwrap_or_else(|| format!("{AID_PREFIX}{:02x}", aid.code()))
        }
        Key::FieldExit
        | Key::FieldPlus
        | Key::FieldMinus
        | Key::Tab
        | Key::BackTab
        | Key::Up
        | Key::Down
        | Key::Left
        | Key::Right
        | Key::Home
        | Key::End
        | Key::Backspace
        | Key::Delete
        | Key::ToggleInsert
        | Key::EraseEof
        | Key::Reset
        | Key::Attention
        | Key::SystemRequest => key_name_of(key)
            // Every one of these is in KEY_NAMES (tested); a comment line reads back as nothing
            // rather than as a broken line.
            .unwrap_or_else(|| format!("# a key without a name: {key:?}")),
    }
}

/// How a macro file names an AID key that has no name of its own: `aid-` and its code in hex.
const AID_PREFIX: &str = "aid-";

/// The name of a key in a macro file and in a keyboard map (R-KEY-2): `f1` to `f24` or one
/// of [`KEY_NAMES`]; `None` for typed characters, cursor moves and other AIDs.
pub fn key_name_of(key: Key) -> Option<String> {
    if let Key::Aid(aid) = key
        && let Some(n) = (1..=24).find(|&n| Aid::function(n) == Some(aid))
    {
        return Some(format!("f{n}"));
    }
    KEY_NAMES
        .iter()
        .find(|(_, k)| *k == key)
        .map(|(name, _)| (*name).to_owned())
}

/// The key a name from [`key_name_of`] stands for, in any case.
pub fn key_named(word: &str) -> Option<Key> {
    let lower = word.to_ascii_lowercase();
    if let Some(n) = lower.strip_prefix('f').and_then(|n| n.parse().ok()) {
        return Aid::function(n).map(Key::Aid);
    }
    if let Some(code) = lower.strip_prefix(AID_PREFIX) {
        let hex = code.len() == 2 && code.bytes().all(|b| b.is_ascii_hexdigit());
        return u8::from_str_radix(code, 16)
            .ok()
            .filter(|_| hex)
            .map(|code| Key::Aid(Aid::from_code(code)));
    }
    KEY_NAMES
        .iter()
        .find(|(name, _)| *name == lower)
        .map(|(_, key)| *key)
}

/// `ROW COLUMN`, 1-based, to a 0-based move.
fn move_to(rest: &str) -> Option<Key> {
    let mut numbers = rest.split_whitespace().map(str::parse::<usize>);
    let (Some(Ok(row)), Some(Ok(column)), None) = (numbers.next(), numbers.next(), numbers.next())
    else {
        return None;
    };
    (row > 0 && column > 0).then(|| Key::MoveTo {
        row: row - 1,
        column: column - 1,
    })
}

fn quote(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

/// The text between double quotes, with `\"` and `\\` undone; `None` if it is not quoted.
fn quoted(rest: &str) -> Option<String> {
    let inner = rest.strip_prefix('"')?.strip_suffix('"')?;
    let mut out = String::new();
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => out.push(chars.next()?),
            '"' => return None,
            c => out.push(c),
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_macro_reads_back_what_it_writes_r_key_3() {
        let m = Macro {
            session: Some("PUB \"400\"".into()),
            steps: vec![
                Step::Text("wrkactjob æøå \\".into()),
                Step::Key(Key::Aid(Aid::ENTER)),
                Step::Wait,
                Step::Password,
                Step::Key(Key::Aid(Aid::function(24).expect("F24"))),
                Step::Key(Key::FieldExit),
                Step::Key(Key::MoveTo { row: 4, column: 19 }),
            ],
        };
        let text = m.to_text();
        assert!(
            text.contains("\nenter\nwait\npassword\nf24\nfield-exit\nmove 5 20\n"),
            "{text}"
        );
        assert_eq!(Macro::parse(&text).expect("parses"), m);
    }

    #[test]
    fn every_named_key_reads_back_r_key_3() {
        for (name, key) in &KEY_NAMES {
            assert_eq!(key_named(name), Some(*key), "{name}");
            assert_eq!(key_name(*key), *name);
        }
    }

    #[test]
    fn every_key_but_text_and_moves_has_a_name_r_key_3() {
        for key in [
            Key::FieldExit,
            Key::FieldPlus,
            Key::FieldMinus,
            Key::Tab,
            Key::BackTab,
            Key::Up,
            Key::Down,
            Key::Left,
            Key::Right,
            Key::Home,
            Key::End,
            Key::Backspace,
            Key::Delete,
            Key::ToggleInsert,
            Key::EraseEof,
            Key::Reset,
            Key::Attention,
            Key::SystemRequest,
        ] {
            assert!(key_name_of(key).is_some(), "{key:?}");
        }
    }

    #[test]
    fn an_aid_key_without_a_name_reads_back_as_aid_and_its_code_r_key_3() {
        let key = Key::Aid(Aid::from_code(0x70));
        assert_eq!(key_name(key), "aid-70");
        assert_eq!(key_named("AID-70"), Some(key));
        assert_eq!(key_named("aid-7"), None);
        assert_eq!(key_named("aid-zz"), None);
        assert_eq!(key_named("aid-+7"), None);
        assert_eq!(key_named("aid-æø"), None);
    }

    #[test]
    fn comments_blank_lines_and_upper_case_keys_are_fine_r_key_4() {
        let m = Macro::parse("# hi\n\n  ENTER \nF3\n").expect("parses");
        assert_eq!(
            m.steps,
            [
                Step::Key(Key::Aid(Aid::ENTER)),
                Step::Key(Key::Aid(Aid::function(3).expect("F3")))
            ]
        );
        assert_eq!(Macro::parse("").expect("empty is fine"), Macro::default());
    }

    #[test]
    fn a_bad_line_is_an_error_that_names_it_r_key_4() {
        for (text, line) in [
            ("enter\nf25", "line 2"),
            ("text wrkactjob", "line 1"),
            ("text \"open", "line 1"),
            ("text \"a\"b\"", "line 1"),
            ("move 0 1", "line 1"),
            ("move 1", "line 1"),
            ("enter\nsession \"late\"", "line 2"),
            ("wait 5", "line 1"),
            ("hello", "line 1"),
        ] {
            let error = Macro::parse(text).expect_err(text).to_string();
            assert!(error.contains(line), "{text}: {error}");
        }
    }
}
