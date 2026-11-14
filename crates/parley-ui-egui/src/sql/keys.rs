//! The SQL window's shortcuts (R-SQL-60): every action that has keys, its own keys, and the
//! keys in use after the user's changes, which Parley SQL keeps in its settings. The window
//! takes the keys pressed from the input once a frame, the ones with the most modifiers first,
//! so Ctrl+Shift+F is never taken as Ctrl+F.

use std::collections::BTreeMap;
use std::sync::LazyLock;

use egui::{Key, Modifiers, Ui};

use crate::keymap::Chord;

/// An action of the SQL window that has keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SqlAction {
    Run,
    RunAll,
    Cancel,
    NewScript,
    CloseScript,
    NextScript,
    Open,
    Save,
    Find,
    Replace,
    GoToLine,
    Format,
    UpperCase,
    LowerCase,
    Comment,
    DuplicateLines,
    MoveLinesUp,
    MoveLinesDown,
    Complete,
    SelectNext,
    ZoomIn,
    ZoomOut,
    ZoomReset,
}

impl SqlAction {
    /// Every action, in the order the settings list them.
    pub const ALL: [Self; 23] = [
        Self::Run,
        Self::RunAll,
        Self::Cancel,
        Self::NewScript,
        Self::CloseScript,
        Self::NextScript,
        Self::Open,
        Self::Save,
        Self::Find,
        Self::Replace,
        Self::GoToLine,
        Self::Format,
        Self::UpperCase,
        Self::LowerCase,
        Self::Comment,
        Self::DuplicateLines,
        Self::MoveLinesUp,
        Self::MoveLinesDown,
        Self::Complete,
        Self::SelectNext,
        Self::ZoomIn,
        Self::ZoomOut,
        Self::ZoomReset,
    ];

    /// How the settings keep it; never changes.
    pub const fn id(self) -> &'static str {
        match self {
            Self::Run => "run",
            Self::RunAll => "run-all",
            Self::Cancel => "cancel",
            Self::NewScript => "new-script",
            Self::CloseScript => "close-script",
            Self::NextScript => "next-script",
            Self::Open => "open",
            Self::Save => "save",
            Self::Find => "find",
            Self::Replace => "replace",
            Self::GoToLine => "go-to-line",
            Self::Format => "format",
            Self::UpperCase => "upper-case",
            Self::LowerCase => "lower-case",
            Self::Comment => "comment",
            Self::DuplicateLines => "duplicate-lines",
            Self::MoveLinesUp => "move-lines-up",
            Self::MoveLinesDown => "move-lines-down",
            Self::Complete => "complete",
            Self::SelectNext => "select-next",
            Self::ZoomIn => "zoom-in",
            Self::ZoomOut => "zoom-out",
            Self::ZoomReset => "zoom-reset",
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Run => "Run the statement",
            Self::RunAll => "Run all",
            Self::Cancel => "Cancel the running statement",
            Self::NewScript => "New script",
            Self::CloseScript => "Close the script",
            Self::NextScript => "Next script",
            Self::Open => "Open a file",
            Self::Save => "Save the script",
            Self::Find => "Find",
            Self::Replace => "Find and replace",
            Self::GoToLine => "Go to a line",
            Self::Format => "Format",
            Self::UpperCase => "Keywords in upper case",
            Self::LowerCase => "Keywords in lower case",
            Self::Comment => "Comment lines",
            Self::DuplicateLines => "Duplicate lines",
            Self::MoveLinesUp => "Move lines up",
            Self::MoveLinesDown => "Move lines down",
            Self::Complete => "Complete the name",
            Self::SelectNext => "Select the next one",
            Self::ZoomIn => "Larger text",
            Self::ZoomOut => "Smaller text",
            Self::ZoomReset => "Text at 100 %",
        }
    }

    /// The heading it is listed under.
    pub const fn group(self) -> &'static str {
        match self {
            Self::Run | Self::RunAll | Self::Cancel => "Run",
            Self::NewScript | Self::CloseScript | Self::NextScript | Self::Open | Self::Save => {
                "Scripts"
            }
            Self::Find | Self::Replace | Self::GoToLine => "Find",
            Self::Format
            | Self::UpperCase
            | Self::LowerCase
            | Self::Comment
            | Self::DuplicateLines
            | Self::MoveLinesUp
            | Self::MoveLinesDown
            | Self::Complete
            | Self::SelectNext => "Editing",
            Self::ZoomIn | Self::ZoomOut | Self::ZoomReset => "Zoom",
        }
    }

    /// Only while the editor's text has the focus: these act on the text or its size.
    const fn in_editor(self) -> bool {
        match self {
            Self::Comment
            | Self::DuplicateLines
            | Self::MoveLinesUp
            | Self::MoveLinesDown
            | Self::Complete
            | Self::SelectNext
            | Self::ZoomIn
            | Self::ZoomOut
            | Self::ZoomReset => true,
            Self::Run
            | Self::RunAll
            | Self::Cancel
            | Self::NewScript
            | Self::CloseScript
            | Self::NextScript
            | Self::Open
            | Self::Save
            | Self::Find
            | Self::Replace
            | Self::GoToLine
            | Self::Format
            | Self::UpperCase
            | Self::LowerCase => false,
        }
    }

    /// Its own keys, as `Chord::text` writes them.
    const fn defaults(self) -> &'static [&'static str] {
        match self {
            Self::Run => &["Ctrl+Enter"],
            Self::RunAll => &["Ctrl+Shift+Enter"],
            Self::Cancel => &["Escape"],
            Self::NewScript => &["Ctrl+T"],
            Self::CloseScript => &["Ctrl+W"],
            Self::NextScript => &["Ctrl+Tab"],
            Self::Open => &["Ctrl+O"],
            Self::Save => &["Ctrl+S"],
            Self::Find => &["Ctrl+F"],
            Self::Replace => &["Ctrl+H"],
            Self::GoToLine => &["Ctrl+G"],
            Self::Format => &["Ctrl+Shift+F"],
            Self::UpperCase => &["Ctrl+Shift+U"],
            Self::LowerCase => &["Ctrl+Shift+L"],
            // Ctrl+Shift+7 is Ctrl+/ where `/` needs Shift (a Danish keyboard).
            Self::Comment => &["Ctrl+Slash", "Ctrl+Shift+7"],
            Self::DuplicateLines => &["Ctrl+Shift+D"],
            Self::MoveLinesUp => &["Alt+ArrowUp"],
            Self::MoveLinesDown => &["Alt+ArrowDown"],
            Self::Complete => &["Ctrl+Space"],
            Self::SelectNext => &["Ctrl+D"],
            Self::ZoomIn => &["Ctrl+Plus", "Ctrl+Equals"],
            Self::ZoomOut => &["Ctrl+Minus"],
            Self::ZoomReset => &["Ctrl+0"],
        }
    }

    /// Its own keys.
    pub fn default_chords(self) -> Vec<Chord> {
        self.defaults()
            .iter()
            .filter_map(|t| Chord::parse_exact(t))
            .collect()
    }
}

/// The keys that cannot change, and what they do, for the settings to list.
pub const FIXED: [(&str, &str); 8] = [
    ("Tab, Shift+Tab", "Indent and outdent lines"),
    (
        "Esc",
        "Close the find bar, the Go to line bar or the completion list",
    ),
    ("Up, Down", "Choose in the completion list"),
    ("Tab, Enter", "Take the name chosen in the completion list"),
    ("Ctrl+Z, Ctrl+Y", "Undo, redo"),
    ("Ctrl+C, Ctrl+X, Ctrl+V", "Copy, cut, paste"),
    ("Ctrl+A", "Select all"),
    ("Ctrl and the wheel", "Larger or smaller text"),
];

/// What the user changed, as the settings keep it: an action's id and its keys, for the
/// actions whose keys differ from their own.
pub type Overrides = BTreeMap<String, Vec<String>>;

/// The keys in use: each action's own keys with the user's changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SqlKeys {
    bindings: Vec<(Chord, SqlAction)>,
}

static DEFAULTS: LazyLock<SqlKeys> = LazyLock::new(|| SqlKeys::new(&Overrides::new()));

impl Default for SqlKeys {
    fn default() -> Self {
        DEFAULTS.clone()
    }
}

impl SqlKeys {
    /// The keys with `overrides`; an unknown action or key in them is ignored.
    pub fn new(overrides: &Overrides) -> Self {
        let mut bindings = Vec::new();
        for action in SqlAction::ALL {
            let chords = match overrides.get(action.id()) {
                Some(keys) => keys.iter().filter_map(|k| Chord::parse_exact(k)).collect(),
                None => action.default_chords(),
            };
            for chord in chords {
                if !bindings.iter().any(|(c, _)| *c == chord) {
                    bindings.push((chord, action));
                }
            }
        }
        Self { bindings }
    }

    pub fn chords_for(&self, action: SqlAction) -> Vec<Chord> {
        self.bindings
            .iter()
            .filter(|(_, a)| *a == action)
            .map(|(c, _)| *c)
            .collect()
    }

    pub fn action_for(&self, chord: Chord) -> Option<SqlAction> {
        self.bindings
            .iter()
            .find(|(c, _)| *c == chord)
            .map(|(_, a)| *a)
    }

    /// The first key of `action` as the window shows it, or "" when it has none.
    pub fn label(&self, action: SqlAction) -> String {
        self.chords_for(action)
            .first()
            .map(|c| c.label())
            .unwrap_or_default()
    }

    /// `text` with the first key of `action` after it in brackets, when it has one.
    pub fn hint(&self, text: &str, action: SqlAction) -> String {
        match self.label(action) {
            key if key.is_empty() => text.to_owned(),
            key => format!("{text} ({key})"),
        }
    }

    /// The actions whose keys were pressed this frame, the keys taken from the input. Keys of
    /// the editor's actions count only while `in_editor`; plain Esc is left alone while
    /// `escape_taken` (a bar or list of the editor closes on it).
    pub fn take(&self, ui: &Ui, in_editor: bool, escape_taken: bool) -> Vec<SqlAction> {
        let mut wanted: Vec<&(Chord, SqlAction)> = self
            .bindings
            .iter()
            .filter(|(_, a)| in_editor || !a.in_editor())
            .filter(|(c, _)| !(escape_taken && is_plain_escape(*c)))
            .collect();
        // The most modifiers first: egui's matching lets Ctrl+F match Ctrl+Shift+F.
        wanted.sort_by_key(|(c, _)| std::cmp::Reverse(modifier_count(*c)));
        let mut pressed = Vec::new();
        ui.input_mut(|i| {
            for (chord, action) in wanted {
                if let Some((modifiers, key)) = pattern(*chord)
                    && i.consume_key(modifiers, key)
                    && !pressed.contains(action)
                {
                    pressed.push(*action);
                }
            }
        });
        pressed
    }
}

fn is_plain_escape(chord: Chord) -> bool {
    matches!(
        chord,
        Chord::Press {
            key: Key::Escape,
            ctrl: false,
            alt: false,
            shift: false
        }
    )
}

fn modifier_count(chord: Chord) -> usize {
    match chord {
        Chord::Press {
            ctrl, alt, shift, ..
        } => usize::from(ctrl) + usize::from(alt) + usize::from(shift),
        Chord::CtrlAlone => 0,
    }
}

/// The modifiers and key egui is asked for; Ctrl is Cmd on a Mac.
fn pattern(chord: Chord) -> Option<(Modifiers, Key)> {
    let Chord::Press {
        key,
        ctrl,
        alt,
        shift,
    } = chord
    else {
        return None;
    };
    let mut modifiers = Modifiers::NONE;
    if ctrl {
        modifiers = modifiers.plus(Modifiers::COMMAND);
    }
    if alt {
        modifiers = modifiers.plus(Modifiers::ALT);
    }
    if shift {
        modifiers = modifiers.plus(Modifiers::SHIFT);
    }
    Some((modifiers, key))
}

/// Why `chord` cannot be a shortcut of the SQL window, or `None` when it can (R-SQL-60): the
/// text field and the system need the rest.
pub fn refusal(chord: Chord) -> Option<String> {
    let Chord::Press { key, ctrl, alt, .. } = chord else {
        return Some("Ctrl on its own is not a shortcut here. Choose another key.".to_owned());
    };
    let function_key = key
        .name()
        .strip_prefix('F')
        .is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()));
    if !ctrl && !alt && !function_key && key != Key::Escape {
        return Some(format!(
            "{} is for typing and moving in the text. Hold Ctrl or Alt with it.",
            chord.label()
        ));
    }
    let editing = matches!(
        key,
        Key::C
            | Key::V
            | Key::X
            | Key::A
            | Key::Z
            | Key::Y
            | Key::ArrowLeft
            | Key::ArrowRight
            | Key::ArrowUp
            | Key::ArrowDown
            | Key::Home
            | Key::End
            | Key::Backspace
            | Key::Delete
    );
    (ctrl && !alt && editing).then(|| {
        format!(
            "{} copies, pastes, undoes or moves in the text. Choose another key.",
            chord.label()
        )
    })
}

/// `overrides` with `action` given `chords`; nothing kept when they are its own keys.
pub fn set(overrides: &mut Overrides, action: SqlAction, chords: &[Chord]) {
    if chords == action.default_chords().as_slice() {
        overrides.remove(action.id());
    } else {
        overrides.insert(
            action.id().to_owned(),
            chords.iter().map(|c| c.text()).collect(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chord(text: &str) -> Chord {
        Chord::parse_exact(text).expect(text)
    }

    #[test]
    fn every_action_has_its_own_keys_and_reads_back_by_id_r_sql_60() {
        let keys = SqlKeys::default();
        for action in SqlAction::ALL {
            assert!(!keys.chords_for(action).is_empty(), "{action:?}");
            let same_id = SqlAction::ALL
                .iter()
                .filter(|a| a.id() == action.id())
                .count();
            assert_eq!(same_id, 1, "{action:?}'s id is its own");
            for c in keys.chords_for(action) {
                assert_eq!(refusal(c), None, "{action:?}: {}", c.label());
                assert_eq!(Chord::parse_exact(&c.text()), Some(c));
            }
        }
        assert_eq!(
            keys.action_for(chord("Ctrl+Shift+F")),
            Some(SqlAction::Format)
        );
        assert_eq!(keys.action_for(chord("Ctrl+F")), Some(SqlAction::Find));
    }

    #[test]
    fn only_changed_actions_are_kept_and_unknown_ones_are_ignored_r_sql_60() {
        let mut overrides = Overrides::new();
        set(&mut overrides, SqlAction::Find, &[chord("Ctrl+Shift+K")]);
        set(
            &mut overrides,
            SqlAction::Save,
            &SqlAction::Save.default_chords(),
        );
        set(&mut overrides, SqlAction::Format, &[]);
        assert_eq!(overrides.len(), 2);
        overrides.insert("gone".to_owned(), vec!["Ctrl+Q".to_owned()]);
        let keys = SqlKeys::new(&overrides);
        assert_eq!(keys.chords_for(SqlAction::Find), [chord("Ctrl+Shift+K")]);
        assert_eq!(keys.label(SqlAction::Format), "");
        assert_eq!(keys.hint("Save", SqlAction::Save), "Save (Ctrl+S)");
        assert_eq!(keys.action_for(chord("Ctrl+Q")), None);
    }

    #[test]
    fn keys_for_typing_and_the_text_field_are_refused_r_sql_60() {
        for text in [
            "K",
            "Shift+K",
            "Enter",
            "Tab",
            "Ctrl+C",
            "Ctrl+Z",
            "Ctrl+Shift+Z",
            "Ctrl+ArrowLeft",
        ] {
            assert!(refusal(chord(text)).is_some(), "{text}");
        }
        for text in [
            "F5",
            "Escape",
            "Ctrl+K",
            "Alt+K",
            "Alt+ArrowUp",
            "Ctrl+Shift+K",
        ] {
            assert_eq!(refusal(chord(text)), None, "{text}");
        }
        assert!(refusal(Chord::CtrlAlone).is_some());
    }

    #[test]
    fn ctrl_shift_f_is_taken_as_format_not_find_r_sql_60() {
        let mut harness = egui_kittest::Harness::builder().build_ui_state(
            |ui, pressed: &mut Vec<SqlAction>| {
                pressed.extend(SqlKeys::default().take(ui, false, false));
            },
            Vec::new(),
        );
        harness.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::F);
        harness.run();
        assert_eq!(harness.state(), &[SqlAction::Format]);
    }
}
