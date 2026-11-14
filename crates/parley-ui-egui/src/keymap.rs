//! PC keys to 5250 keys and Parley actions: keyboard maps (R-KEY-1, R-KEY-2). The built-in
//! **Default** map follows IBM's own emulators where a PC can tell the keys apart: Esc is
//! Attention, a Ctrl key pressed alone is Reset, Page Up and Page Down roll. The operator's
//! own maps are copies of it, changed in Settings ▸ Keyboard and kept with the settings.

use std::sync::LazyLock;

use egui::{Event, Key as PcKey, Modifiers};
use parley_core::macros::{key_name_of, key_named};
use parley_core::{Aid, Key};
use serde::{Deserialize, Serialize};

/// The built-in map's name; no map of the operator's can have it.
pub const DEFAULT_MAP: &str = "Default";

/// A PC key and the modifiers held with it: what a binding binds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Chord {
    Press {
        key: PcKey,
        ctrl: bool,
        alt: bool,
        shift: bool,
    },
    /// A Ctrl key pressed and released on its own.
    CtrlAlone,
}

impl Chord {
    /// `key` pressed with `modifiers`. Shift is left out for a key that types a character:
    /// egui already names the shifted character (Shift+= is Plus on a US keyboard).
    pub fn press(key: PcKey, modifiers: Modifiers) -> Self {
        Self::Press {
            key,
            ctrl: modifiers.ctrl,
            alt: modifiers.alt,
            shift: modifiers.shift && !types_text(key),
        }
    }

    /// `key` pressed with `modifiers`, Shift kept also for a key that types a character: the
    /// SQL window tells Ctrl+F from Ctrl+Shift+F (R-SQL-60).
    pub fn exact(key: PcKey, modifiers: Modifiers) -> Self {
        Self::Press {
            key,
            ctrl: modifiers.ctrl || modifiers.command,
            alt: modifiers.alt,
            shift: modifiers.shift,
        }
    }

    /// Reads [`Chord::text`]: `"Ctrl+Shift+F1"`, `"Ctrl+Plus"`, `"Ctrl"`.
    pub fn parse(text: &str) -> Option<Self> {
        Self::parse_with(text, Self::press)
    }

    /// Reads [`Chord::text`] as [`Chord::exact`] makes it.
    pub fn parse_exact(text: &str) -> Option<Self> {
        Self::parse_with(text, Self::exact)
    }

    fn parse_with(text: &str, make: fn(PcKey, Modifiers) -> Self) -> Option<Self> {
        if text == "Ctrl" {
            return Some(Self::CtrlAlone);
        }
        let (modifiers, key) = match text.strip_suffix("++") {
            Some(modifiers) => (modifiers, "+"),
            None => text.rsplit_once('+').unwrap_or(("", text)),
        };
        let mut m = Modifiers::NONE;
        for part in modifiers.split('+').filter(|p| !p.is_empty()) {
            m = m.plus(match part {
                "Ctrl" => Modifiers::CTRL,
                "Alt" => Modifiers::ALT,
                "Shift" => Modifiers::SHIFT,
                _ => return None,
            });
        }
        Some(make(PcKey::from_name(key)?, m))
    }

    /// How the settings keep it, with egui's key names.
    pub fn text(self) -> String {
        self.with_key_name(PcKey::name)
    }

    /// How Parley shows it: "Ctrl+Page Up", "Shift+Esc".
    pub fn label(self) -> String {
        self.with_key_name(key_label)
    }

    fn with_key_name(self, name: impl Fn(PcKey) -> &'static str) -> String {
        let Self::Press {
            key,
            ctrl,
            alt,
            shift,
        } = self
        else {
            return "Ctrl".to_owned();
        };
        let mut text = String::new();
        for (on, modifier) in [(ctrl, "Ctrl+"), (alt, "Alt+"), (shift, "Shift+")] {
            if on {
                text.push_str(modifier);
            }
        }
        text.push_str(name(key));
        text
    }

    /// Why this key cannot be bound, or `None` when it can (R-KEY-2).
    pub fn refusal(self) -> Option<String> {
        let Self::Press { key, ctrl, alt, .. } = self else {
            return None;
        };
        if !types_text(key) {
            return None;
        }
        if !ctrl {
            return Some(format!(
                "{} types a character. Hold Ctrl with it.",
                self.label()
            ));
        }
        alt.then(|| {
            format!(
                "{} types a character on many keyboards (AltGr). Choose another key.",
                self.label()
            )
        })
    }
}

/// Whether `key` types a character: a letter, digit, punctuation or Space.
fn types_text(key: PcKey) -> bool {
    use PcKey as K;
    let function_key = key
        .name()
        .strip_prefix('F')
        .is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()));
    !function_key
        && !matches!(
            key,
            K::ArrowUp
                | K::ArrowDown
                | K::ArrowLeft
                | K::ArrowRight
                | K::Escape
                | K::Tab
                | K::Backspace
                | K::Enter
                | K::Insert
                | K::Delete
                | K::Home
                | K::End
                | K::PageUp
                | K::PageDown
                | K::Copy
                | K::Cut
                | K::Paste
                | K::BrowserBack
        )
}

/// A key's name as Parley shows it.
fn key_label(key: PcKey) -> &'static str {
    match key {
        PcKey::ArrowUp => "Up",
        PcKey::ArrowDown => "Down",
        PcKey::ArrowLeft => "Left",
        PcKey::ArrowRight => "Right",
        PcKey::Escape => "Esc",
        PcKey::PageUp => "Page Up",
        PcKey::PageDown => "Page Down",
        PcKey::Plus => "+",
        PcKey::Minus => "-",
        PcKey::Equals => "=",
        PcKey::Slash => "/",
        key => key.name(),
    }
}

/// What a key does: a 5250 key, or something Parley does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Function {
    Key(Key),
    CopyScreen,
    Paste,
    /// Start recording a macro, or stop the recording (R-KEY-3).
    RecordMacro,
    /// Run the macro of this name (R-KEY-4).
    RunMacro(String),
    /// Open the keyboard shortcut sheet (R-UI-17).
    Shortcuts,
    /// Open the SQL window (R-SQL-6), while the SQL add-on is on.
    Sql,
}

/// The 5250 keys that go to the host, beside F1–F24, in the order Settings lists them.
const HOST_KEYS: [(Key, &str); 9] = [
    (Key::Aid(Aid::ENTER), "Enter"),
    (Key::Aid(Aid::ROLL_UP), "Roll Up"),
    (Key::Aid(Aid::ROLL_DOWN), "Roll Down"),
    (Key::Aid(Aid::HELP), "Help"),
    (Key::Aid(Aid::PRINT), "Print"),
    (Key::Aid(Aid::CLEAR), "Clear"),
    (Key::Aid(Aid::RECORD_BACKSPACE), "Record Backspace"),
    (Key::Attention, "Attention"),
    (Key::SystemRequest, "System Request"),
];

/// The 5250 keys for fields and the cursor.
const FIELD_KEYS: [(Key, &str); 16] = [
    (Key::FieldExit, "Field Exit"),
    (Key::FieldPlus, "Field+"),
    (Key::FieldMinus, "Field-"),
    (Key::Tab, "Next field"),
    (Key::BackTab, "Previous field"),
    (Key::Up, "Up"),
    (Key::Down, "Down"),
    (Key::Left, "Left"),
    (Key::Right, "Right"),
    (Key::Home, "Home"),
    (Key::End, "End of field"),
    (Key::Backspace, "Backspace"),
    (Key::Delete, "Delete"),
    (Key::ToggleInsert, "Insert mode"),
    (Key::EraseEof, "Erase to end of field"),
    (Key::Reset, "Reset"),
];

const RUN_MACRO: &str = "macro ";

impl Function {
    /// How a keyboard map keeps it: a 5250 key by its macro-file name (`enter`, `f13`), the
    /// others `copy`, `paste`, `record-macro`, `macro NAME`.
    pub fn id(&self) -> String {
        match self {
            Self::Key(key) => key_name_of(*key).unwrap_or_default(),
            Self::CopyScreen => "copy".to_owned(),
            Self::Paste => "paste".to_owned(),
            Self::RecordMacro => "record-macro".to_owned(),
            Self::RunMacro(name) => format!("{RUN_MACRO}{name}"),
            Self::Shortcuts => "shortcuts".to_owned(),
            Self::Sql => "sql".to_owned(),
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Some(match id {
            "copy" => Self::CopyScreen,
            "paste" => Self::Paste,
            "record-macro" => Self::RecordMacro,
            "shortcuts" => Self::Shortcuts,
            "sql" => Self::Sql,
            _ => match id.strip_prefix(RUN_MACRO) {
                Some(name) => Self::RunMacro(name.to_owned()),
                None => Self::Key(key_named(id)?),
            },
        })
    }

    /// What Settings and the shortcuts panel call it.
    pub fn name(&self) -> String {
        match self {
            Self::Key(key) => {
                if let Some(n) = (1..=24).find(|&n| Aid::function(n).map(Key::Aid) == Some(*key)) {
                    return format!("F{n}");
                }
                HOST_KEYS
                    .iter()
                    .chain(&FIELD_KEYS)
                    .find(|(k, _)| k == key)
                    .map_or_else(String::new, |(_, name)| (*name).to_owned())
            }
            Self::CopyScreen => "Copy the screen".to_owned(),
            Self::Paste => "Paste into fields".to_owned(),
            Self::RecordMacro => "Record macro, stop recording".to_owned(),
            Self::RunMacro(name) => format!("Run macro {name}"),
            Self::Shortcuts => "Keyboard shortcuts".to_owned(),
            Self::Sql => "SQL window".to_owned(),
        }
    }

    /// The system's own key for it, which always works and cannot be bound (egui hands it
    /// over as a copy or paste, not as a key).
    pub fn system_key(&self, mac: bool) -> Option<&'static str> {
        match (self, mac) {
            (Self::CopyScreen, false) => Some("Ctrl+C"),
            (Self::CopyScreen, true) => Some("Cmd+C"),
            (Self::Paste, false) => Some("Ctrl+V"),
            (Self::Paste, true) => Some("Cmd+V"),
            _ => None,
        }
    }
}

/// Every function under its heading, in the order Settings lists them. The Macros group
/// holds Record macro and a Run macro for each of `macros`; `None` leaves it out (the
/// Macros add-on is off). `sql` adds the SQL window's function (the SQL add-on is on).
pub fn groups(macros: Option<&[String]>, sql: bool) -> Vec<(&'static str, Vec<Function>)> {
    let keys = |list: &[(Key, &str)]| list.iter().map(|(k, _)| Function::Key(*k)).collect();
    let mut host: Vec<Function> = keys(&HOST_KEYS[..1]);
    host.extend(
        (1..=24)
            .filter_map(Aid::function)
            .map(|a| Function::Key(Key::Aid(a))),
    );
    host.extend(keys(&HOST_KEYS[1..]));
    let mut groups = vec![
        ("Send to the host", host),
        ("Fields and cursor", keys(&FIELD_KEYS)),
        (
            "Parley",
            vec![Function::CopyScreen, Function::Paste, Function::Shortcuts],
        ),
    ];
    if sql && let Some((_, parley)) = groups.last_mut() {
        parley.push(Function::Sql);
    }
    if let Some(macros) = macros {
        let mut list = vec![Function::RecordMacro];
        list.extend(macros.iter().cloned().map(Function::RunMacro));
        groups.push(("Macros", list));
    }
    groups
}

/// A named keyboard map (R-KEY-2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct KeyMap {
    pub name: String,
    /// Each binding as text, the key then what it does (`"Ctrl+R"`, `"reset"`). One this
    /// version cannot read (a later version's) is kept and ignored.
    pub bindings: Vec<(String, String)>,
}

impl Default for KeyMap {
    fn default() -> Self {
        BUILT_IN.clone()
    }
}

/// The Default map (R-KEY-1).
static BUILT_IN: LazyLock<KeyMap> = LazyLock::new(|| {
    let mut bindings: Vec<(String, String)> = [
        ("Enter", "enter"),
        ("Shift+Enter", "field-exit"),
        ("Ctrl+Enter", "field-exit"),
        ("Ctrl+Plus", "field-plus"),
        ("Ctrl+Equals", "field-plus"),
        ("Ctrl+Minus", "field-minus"),
        ("Tab", "tab"),
        ("Shift+Tab", "back-tab"),
        ("PageUp", "roll-down"),
        ("PageDown", "roll-up"),
        ("Ctrl", "reset"),
        ("Ctrl+R", "reset"),
        ("Escape", "attention"),
        ("Shift+Escape", "system-request"),
        ("Insert", "insert"),
        ("Delete", "delete"),
        ("Backspace", "backspace"),
        ("Home", "home"),
        ("End", "end"),
        ("Ctrl+End", "erase-eof"),
        ("Ctrl+Slash", "shortcuts"),
        ("ArrowUp", "up"),
        ("ArrowDown", "down"),
        ("ArrowLeft", "left"),
        ("ArrowRight", "right"),
    ]
    .iter()
    .map(|(k, f)| ((*k).to_owned(), (*f).to_owned()))
    .collect();
    for n in 1..=24 {
        bindings.push((format!("F{n}"), format!("f{n}")));
    }
    for n in 1..=12 {
        bindings.push((format!("Shift+F{n}"), format!("f{}", n + 12)));
    }
    KeyMap {
        name: DEFAULT_MAP.to_owned(),
        bindings,
    }
});

impl KeyMap {
    /// The built-in Default map, which cannot be changed.
    pub fn built_in() -> &'static Self {
        &BUILT_IN
    }

    /// The map called `name` among `maps`; the Default map when there is none of that name
    /// (it was deleted) or `name` is empty.
    pub fn find<'a>(maps: &'a [Self], name: &str) -> &'a Self {
        maps.iter()
            .find(|m| m.name == name)
            .unwrap_or_else(|| Self::built_in())
    }

    pub fn is_built_in(&self) -> bool {
        self.name == DEFAULT_MAP
    }

    /// The bindings this version can read.
    fn parsed(&self) -> impl Iterator<Item = (Chord, Function)> + '_ {
        self.bindings
            .iter()
            .filter_map(|(k, f)| Some((Chord::parse(k)?, Function::from_id(f)?)))
    }

    /// What `chord` does in this map.
    pub fn function_for(&self, chord: Chord) -> Option<Function> {
        self.parsed().find(|(c, _)| *c == chord).map(|(_, f)| f)
    }

    /// The keys bound to `function`, in the order they were added.
    pub fn chords_for(&self, function: &Function) -> Vec<Chord> {
        self.parsed()
            .filter(|(_, f)| f == function)
            .map(|(c, _)| c)
            .collect()
    }

    /// Binds `chord` to `function`, taking it from whatever it did before.
    pub fn bind(&mut self, chord: Chord, function: &Function) {
        self.unbind(chord);
        self.bindings.push((chord.text(), function.id()));
    }

    pub fn unbind(&mut self, chord: Chord) {
        self.bindings
            .retain(|(k, _)| Chord::parse(k).is_none_or(|c| c != chord));
    }

    /// Takes away every key of `function`.
    pub fn unbind_all(&mut self, function: &Function) {
        let id = function.id();
        self.bindings.retain(|(_, f)| *f != id);
    }

    /// Bindings to run the macro `old` now run `new` (R-KEY-4: renaming a macro).
    pub fn rename_macro(&mut self, old: &str, new: &str) {
        let (old, new) = (
            Function::RunMacro(old.to_owned()).id(),
            Function::RunMacro(new.to_owned()).id(),
        );
        for (_, f) in &mut self.bindings {
            if *f == old {
                f.clone_from(&new);
            }
        }
    }

    /// The keys of every bound function, for the shortcuts panel: (keys, function) in the
    /// order of [`groups`]. `mac` names the system's copy and paste keys as macOS does.
    pub fn shortcuts(&self, macros: Option<&[String]>, mac: bool) -> Vec<(String, String)> {
        groups(macros, false)
            .into_iter()
            .flat_map(|(_, functions)| functions)
            .filter_map(|f| {
                let keys = self.keys_for(&f, mac);
                (!keys.is_empty()).then(|| (keys.join(", "), f.name()))
            })
            .collect()
    }

    /// Every key that does `function`, as Parley writes them: the system's own first.
    pub fn keys_for(&self, function: &Function, mac: bool) -> Vec<String> {
        let mut keys: Vec<String> = function
            .system_key(mac)
            .map(str::to_owned)
            .into_iter()
            .collect();
        keys.extend(self.chords_for(function).into_iter().map(Chord::label));
        keys
    }
}

/// Notices a Ctrl key pressed and released on its own, from the modifiers frame by frame.
#[derive(Debug, Default)]
pub struct CtrlAlone {
    previous: Modifiers,
    pending: bool,
}

impl CtrlAlone {
    /// True on the frame Ctrl is let go, if nothing else was pressed while it was held.
    /// `other_input`: a key, text or clipboard event came this frame.
    pub fn released(&mut self, other_input: bool, modifiers: Modifiers) -> bool {
        let ctrl_only = modifiers.ctrl && !modifiers.shift && !modifiers.alt;
        let was_ctrl_only = self.previous.ctrl && !self.previous.shift && !self.previous.alt;
        self.previous = modifiers;
        if other_input || (modifiers.ctrl && !ctrl_only) {
            self.pending = false;
        } else if ctrl_only && !was_ctrl_only {
            self.pending = true;
        } else if !modifiers.ctrl && self.pending {
            self.pending = false;
            return true;
        }
        false
    }
}

/// Turns egui input into what the keys do under a map, frame by frame.
#[derive(Debug, Default)]
pub struct KeyInput {
    ctrl_alone: CtrlAlone,
}

impl KeyInput {
    pub fn read(&mut self, events: &[Event], modifiers: Modifiers, map: &KeyMap) -> Vec<Function> {
        let mut functions = Vec::new();
        let mut other_input = false;
        for event in events {
            match event {
                Event::Text(text) => {
                    other_input = true;
                    let typed = text.chars().filter(|c| !c.is_control()).map(Key::Char);
                    functions.extend(typed.map(Function::Key));
                }
                Event::Paste(text) => {
                    other_input = true;
                    functions.extend(pasted(text).map(Function::Key));
                }
                Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } => {
                    other_input = true;
                    functions.extend(map.function_for(Chord::press(*key, *modifiers)));
                }
                Event::Copy => {
                    other_input = true;
                    functions.push(Function::CopyScreen);
                }
                Event::Cut => other_input = true,
                _ => {}
            }
        }
        if self.ctrl_alone.released(other_input, modifiers) {
            functions.extend(map.function_for(Chord::CtrlAlone));
        }
        functions
    }
}

/// Pasted text goes into fields as typed; a line break or tab moves to the next field.
fn pasted(text: &str) -> impl Iterator<Item = Key> + '_ {
    let mut previous = None;
    text.chars().filter_map(move |c| {
        let after_cr = previous == Some('\r');
        previous = Some(c);
        match c {
            '\n' if after_cr => None,
            '\r' | '\t' | '\n' => Some(Key::FieldExit),
            c if c.is_control() => None,
            c => Some(Key::Char(c)),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(key: PcKey, modifiers: Modifiers) -> Event {
        Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }
    }

    fn keys_for(events: &[Event]) -> Vec<Function> {
        KeyInput::default().read(events, Modifiers::NONE, KeyMap::built_in())
    }

    fn key(key: Key) -> Vec<Function> {
        vec![Function::Key(key)]
    }

    fn f(n: u8) -> Key {
        Key::Aid(Aid::function(n).expect("F1 to F24"))
    }

    #[test]
    fn enter_function_and_roll_keys_are_aids() {
        // R-KEY-1
        assert_eq!(
            keys_for(&[press(PcKey::Enter, Modifiers::NONE)]),
            key(Key::Aid(Aid::ENTER))
        );
        assert_eq!(keys_for(&[press(PcKey::F3, Modifiers::NONE)]), key(f(3)));
        assert_eq!(keys_for(&[press(PcKey::F1, Modifiers::SHIFT)]), key(f(13)));
        assert_eq!(keys_for(&[press(PcKey::F24, Modifiers::NONE)]), key(f(24)));
        assert_eq!(
            keys_for(&[press(PcKey::PageDown, Modifiers::NONE)]),
            key(Key::Aid(Aid::ROLL_UP))
        );
    }

    #[test]
    fn field_keys_use_modifiers() {
        // R-KEY-1
        assert_eq!(
            keys_for(&[press(PcKey::Enter, Modifiers::SHIFT)]),
            key(Key::FieldExit)
        );
        assert_eq!(
            keys_for(&[press(PcKey::Minus, Modifiers::CTRL)]),
            key(Key::FieldMinus)
        );
        assert_eq!(
            keys_for(&[press(PcKey::Tab, Modifiers::SHIFT)]),
            key(Key::BackTab)
        );
        assert_eq!(
            keys_for(&[press(PcKey::End, Modifiers::CTRL)]),
            key(Key::EraseEof)
        );
        assert_eq!(
            keys_for(&[press(PcKey::Escape, Modifiers::NONE)]),
            key(Key::Attention)
        );
    }

    #[test]
    fn ctrl_plus_works_with_or_without_shift_r_key_1() {
        // On a US keyboard + is Shift+=, which egui reports as Plus with Shift held.
        let shifted = Modifiers::CTRL.plus(Modifiers::SHIFT);
        assert_eq!(
            keys_for(&[press(PcKey::Plus, shifted)]),
            key(Key::FieldPlus)
        );
        assert_eq!(
            keys_for(&[press(PcKey::Equals, Modifiers::CTRL)]),
            key(Key::FieldPlus)
        );
    }

    #[test]
    fn typed_and_pasted_text_become_characters() {
        let keys = keys_for(&[Event::Text("æ1".into()), Event::Paste("ab\r\nc\td".into())]);
        let expected: Vec<Function> = [
            Key::Char('æ'),
            Key::Char('1'),
            Key::Char('a'),
            Key::Char('b'),
            Key::FieldExit,
            Key::Char('c'),
            Key::FieldExit,
            Key::Char('d'),
        ]
        .into_iter()
        .map(Function::Key)
        .collect();
        assert_eq!(keys, expected);
    }

    #[test]
    fn a_ctrl_key_pressed_and_released_alone_is_reset() {
        let mut input = KeyInput::default();
        let map = KeyMap::built_in();
        assert_eq!(input.read(&[], Modifiers::CTRL, map), []);
        assert_eq!(input.read(&[], Modifiers::NONE, map), key(Key::Reset));
    }

    #[test]
    fn ctrl_with_another_key_is_not_reset() {
        let mut input = KeyInput::default();
        let map = KeyMap::built_in();
        input.read(&[], Modifiers::CTRL, map);
        let minus = [press(PcKey::Minus, Modifiers::CTRL)];
        assert_eq!(
            input.read(&minus, Modifiers::CTRL, map),
            key(Key::FieldMinus)
        );
        assert_eq!(input.read(&[], Modifiers::NONE, map), []);
        input.read(&[], Modifiers::CTRL, map);
        input.read(&[Event::Copy], Modifiers::CTRL, map);
        assert_eq!(input.read(&[], Modifiers::NONE, map), []);
    }

    #[test]
    fn the_system_copy_key_copies_the_screen() {
        assert_eq!(keys_for(&[Event::Copy]), [Function::CopyScreen]);
    }

    #[test]
    fn a_map_of_ones_own_changes_what_keys_do_r_key_2() {
        let mut map = KeyMap {
            name: "Mine".to_owned(),
            ..KeyMap::default()
        };
        let ctrl_r = Chord::parse("Ctrl+R").expect("a key");
        map.bind(ctrl_r, &Function::Key(Key::Aid(Aid::HELP)));
        map.bind(
            Chord::parse("Alt+F1").expect("a key"),
            &Function::RunMacro("WRKACTJOB".into()),
        );
        map.unbind(Chord::CtrlAlone);
        let mut input = KeyInput::default();
        let read = |input: &mut KeyInput, events: &[Event], m| input.read(events, m, &map);
        assert_eq!(
            read(
                &mut input,
                &[press(PcKey::R, Modifiers::CTRL)],
                Modifiers::CTRL
            ),
            key(Key::Aid(Aid::HELP))
        );
        assert_eq!(
            read(
                &mut input,
                &[press(PcKey::F1, Modifiers::ALT)],
                Modifiers::NONE
            ),
            [Function::RunMacro("WRKACTJOB".into())]
        );
        read(&mut input, &[], Modifiers::CTRL);
        assert_eq!(
            read(&mut input, &[], Modifiers::NONE),
            [],
            "Ctrl alone unbound"
        );
        assert_eq!(map.chords_for(&Function::Key(Key::Reset)), []);
        assert_eq!(
            KeyMap::built_in()
                .chords_for(&Function::Key(Key::Reset))
                .len(),
            2
        );
    }

    #[test]
    fn keys_read_back_as_they_are_written_r_key_2() {
        for (text, label) in [
            ("Ctrl+Shift+F1", "Ctrl+Shift+F1"),
            ("Ctrl+Plus", "Ctrl++"),
            ("Shift+Escape", "Shift+Esc"),
            ("Alt+PageUp", "Alt+Page Up"),
            ("Ctrl", "Ctrl"),
        ] {
            let chord = Chord::parse(text).expect(text);
            assert_eq!(chord.text(), text);
            assert_eq!(chord.label(), label);
        }
        assert_eq!(Chord::parse("Ctrl++"), Chord::parse("Ctrl+Plus"));
        assert_eq!(Chord::parse("Hyper+A"), None);
        assert_eq!(Chord::parse("Ctrl+Nope"), None);
    }

    #[test]
    fn keys_that_type_characters_cannot_be_bound_r_key_2() {
        let refused = |text| Chord::parse(text).expect(text).refusal();
        assert!(refused("A").is_some());
        assert!(refused("Shift+Space").is_some());
        assert!(refused("Alt+7").is_some());
        assert!(
            refused("Ctrl+Alt+2").is_some(),
            "AltGr+2 is @ on many keyboards"
        );
        assert_eq!(refused("Ctrl+A"), None);
        assert_eq!(refused("F13"), None);
        assert_eq!(refused("Alt+ArrowUp"), None);
        assert_eq!(refused("Ctrl"), None);
    }

    #[test]
    fn every_function_has_a_name_and_reads_back_r_key_2() {
        let macros = ["æøå".to_owned()];
        for (_, functions) in groups(Some(&macros), true) {
            for function in functions {
                assert!(!function.name().is_empty(), "{function:?}");
                assert_eq!(Function::from_id(&function.id()), Some(function));
            }
        }
    }

    #[test]
    fn unknown_bindings_are_kept_but_ignored_r_key_2() {
        let map = KeyMap {
            name: "Later".to_owned(),
            bindings: vec![
                ("Hyper+Q".to_owned(), "enter".to_owned()),
                ("F2".to_owned(), "teleport".to_owned()),
            ],
        };
        assert_eq!(map.function_for(Chord::parse("F2").expect("F2")), None);
        assert_eq!(map.shortcuts(None, false).len(), 2, "only copy and paste");
    }

    #[test]
    fn a_deleted_map_falls_back_to_default_r_key_2() {
        let maps = [KeyMap {
            name: "Mine".to_owned(),
            bindings: Vec::new(),
        }];
        assert_eq!(KeyMap::find(&maps, "Mine").name, "Mine");
        assert!(KeyMap::find(&maps, "Gone").is_built_in());
        assert!(KeyMap::find(&maps, "").is_built_in());
    }

    #[test]
    fn renaming_a_macro_moves_its_keys_r_key_2() {
        let mut map = KeyMap::default();
        let alt_f1 = Chord::parse("Alt+F1").expect("a key");
        map.bind(alt_f1, &Function::RunMacro("old".into()));
        map.rename_macro("old", "new");
        assert_eq!(
            map.function_for(alt_f1),
            Some(Function::RunMacro("new".into()))
        );
    }
}
