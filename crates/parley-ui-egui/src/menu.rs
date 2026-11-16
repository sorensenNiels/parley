//! The header (R-UI-15, R-UI-10, R-UI-18): the app's icon and name, the menus, and on the right
//! the gear for Settings, the session in use and a newer version. It only reports what the operator chose, as an
//! [`Action`] for [`crate::ParleyUi`] to carry out.

use egui::{Button, MenuBar, RichText, Sense, Ui};
use parley_core::net::LinkState;
use parley_core::{Key, MacroState};

use crate::icons;
use crate::keymap::{Function, KeyMap};
use crate::profile::Profile;
use crate::theme;

/// Which program the UI is: Parley, Parley SQL, the SQL window on its own (R-SQL-45), or Parley SSH (D41).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Program {
    Parley,
    ParleySql,
    ParleySsh,
}

impl Program {
    /// The name the header, the title bar and the About page show.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Parley => "Parley 5250",
            Self::ParleySql => "Parley SQL",
            Self::ParleySsh => "Parley SSH",
        }
    }
}

/// Something the operator chose from the header or the status bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    NewSession,
    /// The session dialog, on the session in use or the first one.
    EditSessions,
    /// Connect to the saved session at this index, closing the open one first.
    Connect(usize),
    Disconnect,
    Exit,
    /// Copy the selection, or the screen (R-UI-22).
    CopyScreen,
    /// Copy the selection, or the screen, as a table (R-UI-22).
    CopyTable,
    Paste,
    /// Edit ▸ Find… (R-UI-21).
    Find,
    /// Tools ▸ SQL… (R-SQL-6).
    Sql,
    /// Tools ▸ SSH… (R-SSH-59).
    Ssh,
    /// The gear (R-UI-18).
    Settings,
    Key(Key),
    /// Tools ▸ Record macro (R-KEY-3).
    RecordMacro,
    StopRecording,
    /// Tools ▸ Run macro ▸: the macro at this index of the menu's list (R-KEY-4).
    RunMacro(usize),
    StopMacro,
    /// Tools ▸ Macros…
    Macros,
}

/// What the Tools menu shows about macros: the session's macro state (`None` when not
/// connected) and the names of the macros it can run.
#[derive(Debug)]
pub struct MacroMenu<'a> {
    pub state: Option<MacroState>,
    pub names: &'a [String],
}

/// What the header shows.
pub struct Header<'a> {
    /// The state of the open connection; `None` when there is none.
    pub link: Option<&'a LinkState>,
    pub profiles: &'a [Profile],
    /// The session in use.
    pub current: Option<&'a Profile>,
    pub logo: Option<&'a egui::TextureHandle>,
    pub newer: Option<&'a parley_core::update::Release>,
    /// `None` while the Macros add-on is off (R-ADD-2).
    pub macros: Option<&'a MacroMenu<'a>>,
    /// The SQL add-on is on (R-SQL-10).
    pub sql: bool,
    /// The SSH add-on is on (R-SSH-59).
    pub ssh: bool,
    /// The keyboard map in use, for the keys the menus show.
    pub map: &'a KeyMap,
    pub mac: bool,
}

impl std::fmt::Debug for Header<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Header")
            .field("link", &self.link)
            .field("current", &self.current.map(|p| &p.name))
            .finish_non_exhaustive()
    }
}

impl Header<'_> {
    const fn connected(&self) -> bool {
        self.link.is_some()
    }

    /// The first key that does `function`, as the menus show it.
    fn key(&self, function: &Function) -> Option<String> {
        self.map.keys_for(function, self.mac).into_iter().next()
    }
}

/// The header: brand, menus (Tools only while it has something); the gear, the session pill
/// and a newer version on the right (R-UI-18).
pub fn menu_bar(ui: &mut Ui, header: &Header<'_>) -> Option<Action> {
    let mut action = None;
    MenuBar::new().ui(ui, |ui| {
        brand(ui, header.logo, Program::Parley);
        let tools = header.macros.is_some() || header.sql || header.ssh;
        // Tools, last, only while it has items.
        let menus = [file_menu, edit_menu, tools_menu];
        for menu in menus.into_iter().take(menus.len() - usize::from(!tools)) {
            if let Some(chosen) = menu(ui, header) {
                action = Some(chosen);
            }
        }
        theme::button_row(ui, |ui| {
            if gear(ui) {
                action = Some(Action::Settings);
            }
            if let Some(chosen) = session_pill(ui, header) {
                action = Some(chosen);
            }
            // A quiet link, never a pop-up; the download command is on the About page.
            if let Some(release) = header.newer {
                ui.hyperlink_to(
                    format!("Parley {} is available", release.version),
                    &release.page,
                )
                .on_hover_text("Opens the release page, with its notes and installers.");
                ui.add_space(8.0);
            }
        });
    });
    action
}

/// One menu item: `icon`, `text` and the key that does `function`, if any.
fn item(
    ui: &mut Ui,
    header: &Header<'_>,
    enabled: bool,
    (icon, text): (&str, &str),
    function: Option<&Function>,
) -> bool {
    let mut button = icons::button(icon, text);
    if let Some(key) = function.and_then(|f| header.key(f)) {
        button = button.shortcut_text(key);
    }
    icons::add_named(ui, enabled, button, text).clicked()
}

fn file_menu(ui: &mut Ui, header: &Header<'_>) -> Option<Action> {
    let mut action = None;
    ui.menu_button("File", |ui| {
        if item(ui, header, true, (icons::PLUS, "New session…"), None) {
            action = Some(Action::NewSession);
        }
        if item(ui, header, true, (icons::PENCIL, "Edit sessions…"), None) {
            action = Some(Action::EditSessions);
        }
        let can_connect = !header.connected() && !header.profiles.is_empty();
        let names = header.profiles.iter().map(|p| p.name.as_str());
        if let Some(i) = connect_menu(ui, can_connect, names, None) {
            action = Some(Action::Connect(i));
        }
        ui.separator();
        if item(
            ui,
            header,
            header.connected(),
            (icons::POWER, "Disconnect"),
            None,
        ) {
            action = Some(Action::Disconnect);
        }
        ui.separator();
        if item(ui, header, true, (icons::SIGN_OUT, "Exit"), None) {
            action = Some(Action::Exit);
        }
    });
    action
}

/// "Connect to ▸" with every saved session (or connection, R-SQL-46) named in `names` but
/// `except`; the index of the one chosen.
pub fn connect_menu<'a>(
    ui: &mut Ui,
    enabled: bool,
    names: impl Iterator<Item = &'a str>,
    except: Option<&str>,
) -> Option<usize> {
    let mut chosen = None;
    ui.add_enabled_ui(enabled, |ui| {
        let text = (icons::icon(icons::PLUGS_CONNECTED), "Connect to");
        ui.menu_button(text, |ui| {
            for (i, name) in names.enumerate() {
                if Some(name) != except && ui.button(name).clicked() {
                    chosen = Some(i);
                }
            }
        })
        .response
        .widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, "Connect to"));
    });
    chosen
}

fn edit_menu(ui: &mut Ui, header: &Header<'_>) -> Option<Action> {
    let mut action = None;
    ui.menu_button("Edit", |ui| {
        let on = header.connected();
        let copy = (icons::COPY, "Copy");
        if item(ui, header, on, copy, Some(&Function::CopyScreen)) {
            action = Some(Action::CopyScreen);
        }
        let table = (icons::LIST, "Copy as table");
        if item(ui, header, on, table, Some(&Function::CopyTable)) {
            action = Some(Action::CopyTable);
        }
        if item(
            ui,
            header,
            on,
            (icons::CLIPBOARD, "Paste"),
            Some(&Function::Paste),
        ) {
            action = Some(Action::Paste);
        }
        ui.separator();
        let find = (icons::MAGNIFYING_GLASS, "Find…");
        if item(ui, header, on, find, Some(&Function::Find)) {
            action = Some(Action::Find);
        }
    });
    action
}

fn tools_menu(ui: &mut Ui, header: &Header<'_>) -> Option<Action> {
    let mut action = None;
    ui.menu_button("Tools", |ui| {
        if let Some(macros) = header.macros {
            action = macro_items(ui, header, macros);
        }
        // R-SQL-6, R-SSH-59: for the connected session's host.
        if (header.sql || header.ssh) && header.macros.is_some() {
            ui.separator();
        }
        if header.sql {
            let sql = (icons::DATABASE, "SQL…");
            if item(ui, header, header.connected(), sql, Some(&Function::Sql)) {
                action = Some(Action::Sql);
            }
        }
        if header.ssh {
            let ssh = (icons::TERMINAL, "SSH…");
            if item(ui, header, header.connected(), ssh, Some(&Function::Ssh)) {
                action = Some(Action::Ssh);
            }
        }
    });
    action
}

/// How much larger than the menus' text the gear is drawn.
const GEAR_SCALE: f32 = 1.45;

/// The gear at the right end of the header, for Settings (R-UI-18; Parley SQL's, R-SQL-45);
/// true when clicked. Larger than the menus' text (R-UI-19), and growing with the text size.
pub fn gear(ui: &mut Ui) -> bool {
    let muted = theme::palette(ui).muted;
    let size = egui::TextStyle::Button.resolve(ui.style()).size * GEAR_SCALE;
    let icon = icons::icon(icons::GEAR).size(size).color(muted);
    let button = Button::new(icon).frame_when_inactive(false);
    icons::add_named(ui, true, button, "Settings")
        .on_hover_text("Settings")
        .clicked()
}

/// Record / Stop recording, Run macro ▸, Stop macro and Macros…
fn macro_items(ui: &mut Ui, header: &Header<'_>, macros: &MacroMenu<'_>) -> Option<Action> {
    let mut action = None;
    let idle = macros.state == Some(MacroState::Off);
    let record = Some(&Function::RecordMacro);
    if macros.state == Some(MacroState::Recording) {
        if item(ui, header, true, (icons::STOP, "Stop recording"), record) {
            action = Some(Action::StopRecording);
        }
    } else if item(ui, header, idle, (icons::RECORD, "Record macro"), record) {
        action = Some(Action::RecordMacro);
    }
    let can_run = idle && !macros.names.is_empty();
    ui.add_enabled_ui(can_run, |ui| {
        ui.menu_button((icons::icon(icons::PLAY), "Run macro"), |ui| {
            for (i, name) in macros.names.iter().enumerate() {
                let key = header.key(&Function::RunMacro(name.clone()));
                let button = Button::new(name).shortcut_text(key.unwrap_or_default());
                if ui.add(button).clicked() {
                    action = Some(Action::RunMacro(i));
                }
            }
        })
        .response
        .widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, can_run, "Run macro"));
    });
    let playing = matches!(
        macros.state,
        Some(MacroState::Playing | MacroState::NeedsPassword)
    );
    if playing {
        let stop = icons::button(icons::STOP, "Stop macro").shortcut_text("Reset");
        if icons::add_named(ui, true, stop, "Stop macro").clicked() {
            action = Some(Action::StopMacro);
        }
    }
    if item(ui, header, true, (icons::LIST, "Macros…"), None) {
        action = Some(Action::Macros);
    }
    action
}

/// The session in use while connected (R-UI-15): a status dot, its name, host:port and a lock
/// for TLS; a menu to switch, edit or disconnect. Laid out right to left.
fn session_pill(ui: &mut Ui, header: &Header<'_>) -> Option<Action> {
    let (Some(link), Some(profile)) = (header.link, header.current) else {
        return None;
    };
    let p = theme::palette(ui);
    let config = &profile.config;
    let lock = if config.tls { icons::LOCK } else { "" };
    let atoms = (
        theme::semibold(&profile.name),
        RichText::new(format!("{}:{}", config.host, config.port)).color(p.muted),
        icons::icon(lock).color(p.muted),
        icons::icon(icons::CARET_DOWN).color(p.muted),
    );
    let mut action = None;
    let response = ui.menu_button(atoms, |ui| {
        theme::section_label(ui, &profile.name);
        let others = header.profiles.len() > 1;
        let names = header.profiles.iter().map(|p| p.name.as_str());
        if let Some(i) = connect_menu(ui, others, names, Some(&profile.name)) {
            action = Some(Action::Connect(i));
        }
        if item(ui, header, true, (icons::PENCIL, "Edit session…"), None) {
            action = Some(Action::EditSessions);
        }
        ui.separator();
        if item(ui, header, true, (icons::POWER, "Disconnect"), None) {
            action = Some(Action::Disconnect);
        }
    });
    let state = match link {
        LinkState::Connected => "online",
        LinkState::Connecting => "connecting",
        LinkState::Closed(_) | LinkState::Dropped(_) => "offline",
    };
    response
        .response
        .on_hover_text(format!("{} is {state}", profile.name));
    let color = match link {
        LinkState::Connected => p.ok,
        LinkState::Connecting => p.warning_text,
        LinkState::Closed(_) | LinkState::Dropped(_) => p.muted,
    };
    let (rect, _) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), Sense::hover());
    ui.painter().circle_filled(rect.center(), 4.0, color);
    ui.add_space(4.0);
    action
}

/// The app icon for the menu bar: the decoded image until the first frame makes it a texture.
#[derive(Default)]
pub struct Logo {
    image: Option<egui::ColorImage>,
    texture: Option<egui::TextureHandle>,
}

impl std::fmt::Debug for Logo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(if self.texture.is_some() || self.image.is_some() {
            "Logo"
        } else {
            "Logo(none)"
        })
    }
}

impl Logo {
    pub fn new(image: egui::ColorImage) -> Self {
        Self {
            image: Some(image),
            texture: None,
        }
    }

    /// The icon as a texture, made on first use.
    pub fn texture(&mut self, ctx: &egui::Context) -> Option<&egui::TextureHandle> {
        if let Some(image) = self.image.take() {
            self.texture = Some(ctx.load_texture("logo", image, egui::TextureOptions::LINEAR));
        }
        self.texture.as_ref()
    }
}

/// The app's icon and name, as Tortuga shows its own at the start of its top bar.
pub fn brand(ui: &mut Ui, logo: Option<&egui::TextureHandle>, program: Program) {
    if let Some(logo) = logo {
        ui.add(
            egui::Image::new(logo)
                .fit_to_exact_size(egui::vec2(18.0, 18.0))
                .alt_text(program.name()),
        );
    }
    ui.label(theme::semibold(program.name()).size(15.0));
    ui.add_space(8.0);
    ui.separator();
    ui.add_space(2.0);
}
