//! Saved sessions (R-SSH-11, `PuTTY`'s Session page): a name, where to connect and how to sign on, kept in Parley
//! SSH's settings; "Default Settings" is what a new session starts with. Never a password or passphrase (D16, D44).

use egui::Context;
use parley_ssh_core::net::agent::AgentAddress;
use parley_ssh_core::net::jump;
use parley_ssh_core::net::{Options, Target};
use parley_ssh_core::openssh_config::Config;
use parley_ui_egui::{dialog, theme};
use serde::{Deserialize, Serialize};

/// The name the defaults go by, as in `PuTTY`.
pub const DEFAULT_SETTINGS: &str = "Default Settings";

/// One saved session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SavedSession {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub user: String,
    /// Private key files, tried in this order after the agent's keys (R-SSH-12).
    pub key_files: Vec<String>,
    /// Sign on with the running agent's keys (R-SSH-13).
    pub use_agent: bool,
    /// Only the agent key with this fingerprint (`SHA256:…`); empty: each of its keys.
    pub agent_key: String,
    /// Let the server use the agent while connected; off by default (R-SSH-13).
    pub agent_forwarding: bool,
    /// Programs on the host that ask for the mouse get it (R-SSH-18); Shift still selects.
    pub mouse_reporting: bool,
    /// Jump hosts as OpenSSH's `ProxyJump` writes them, `[user@]host[:port]` separated by commas (R-SSH-43).
    pub proxy_jump: String,
}

impl Default for SavedSession {
    fn default() -> Self {
        Self {
            name: String::new(),
            host: String::new(),
            port: 22,
            user: String::new(),
            key_files: Vec::new(),
            use_agent: true,
            agent_key: String::new(),
            agent_forwarding: false,
            mouse_reporting: true,
            proxy_jump: String::new(),
        }
    }
}

impl SavedSession {
    /// A new session from the defaults, named `name`, to `target`.
    pub fn from_defaults(defaults: &Self, name: &str, target: &Target) -> Self {
        Self {
            name: name.to_owned(),
            host: target.host.clone(),
            port: target.port,
            user: target.user.clone(),
            ..defaults.clone()
        }
    }

    pub fn target(&self) -> Target {
        Target {
            host: self.host.trim().to_owned(),
            port: self.port,
            user: self.user.trim().to_owned(),
        }
    }

    /// How to sign on: the agent the system names, when the session uses one (R-SSH-13), and the jump hosts,
    /// their names looked up in `~/.ssh/config` (R-SSH-43), which is read only when there are any.
    ///
    /// # Errors
    /// Jump hosts that cannot be read, said for the user.
    pub fn options(&self) -> Result<Options, String> {
        let config = if self.proxy_jump.trim().is_empty() {
            None
        } else {
            Config::from_home()
        };
        self.options_with(config.as_ref())
    }

    /// [`Self::options`] with the config given.
    ///
    /// # Errors
    /// Jump hosts that cannot be read, said for the user.
    pub fn options_with(&self, config: Option<&Config>) -> Result<Options, String> {
        let key_files: Vec<std::path::PathBuf> = self
            .key_files
            .iter()
            .filter(|k| !k.trim().is_empty())
            .map(|k| k.trim().into())
            .collect();
        let jumps = jump::resolve(&self.proxy_jump, self.user.trim(), &key_files, config)
            .map_err(|e| format!("The jump hosts cannot be used: {e}."))?;
        Ok(Options {
            key_files,
            agent: if self.use_agent {
                AgentAddress::from_environment()
            } else {
                None
            },
            agent_key: Some(self.agent_key.trim().to_owned()).filter(|k| !k.is_empty()),
            agent_forwarding: self.use_agent && self.agent_forwarding,
            jumps,
        })
    }

    /// The line under the name on the start screen.
    pub fn address(&self) -> String {
        self.target().label()
    }

    /// The command line that opens this session in another window (R-SSH-14, R-SSH-15).
    pub fn command_line(&self, saved: bool) -> Vec<String> {
        if saved {
            return vec!["-load".to_owned(), self.name.clone()];
        }
        let mut args = vec![
            format!("{}@{}", self.user, self.host),
            "-p".to_owned(),
            self.port.to_string(),
        ];
        for key in &self.key_files {
            args.extend(["-i".to_owned(), key.clone()]);
        }
        args.push(if self.agent_forwarding { "-A" } else { "-a" }.to_owned());
        if !self.proxy_jump.trim().is_empty() {
            args.extend(["-J".to_owned(), self.proxy_jump.trim().to_owned()]);
        }
        args
    }
}

/// What the editor closed with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Edited {
    Save(SavedSession),
    Delete,
    Cancel,
}

/// The dialog that edits one saved session, or the defaults.
#[derive(Debug)]
pub struct Editor {
    session: SavedSession,
    port: String,
    key_files: String,
    /// The index edited; `None` for a new one.
    pub index: Option<usize>,
    defaults: bool,
    error: Option<String>,
    confirm_delete: bool,
}

impl Editor {
    pub fn new(session: SavedSession, index: Option<usize>) -> Self {
        let defaults = session.name == DEFAULT_SETTINGS && index.is_none();
        Self {
            port: session.port.to_string(),
            key_files: session.key_files.join("\n"),
            session,
            index,
            defaults,
            error: None,
            confirm_delete: false,
        }
    }

    /// The editor of the defaults.
    pub fn defaults(defaults: &SavedSession) -> Self {
        Self::new(
            SavedSession {
                name: DEFAULT_SETTINGS.to_owned(),
                ..defaults.clone()
            },
            None,
        )
    }

    pub fn is_defaults(&self) -> bool {
        self.defaults
    }

    /// Draws the dialog; `taken` are the other sessions' names. `None` while it is open.
    pub fn show(&mut self, ctx: &Context, taken: &[&str]) -> Option<Edited> {
        let title = if self.defaults {
            DEFAULT_SETTINGS.to_owned()
        } else if self.index.is_some() {
            "Edit session".to_owned()
        } else {
            "New session".to_owned()
        };
        let (close, clicked) = dialog::dialog(ctx, "ssh-session-editor", &title, 540.0, |ui| {
            self.content(ui)
        });
        if close {
            return Some(Edited::Cancel);
        }
        match clicked? {
            Clicked::Cancel => Some(Edited::Cancel),
            Clicked::Delete if self.confirm_delete => Some(Edited::Delete),
            Clicked::Delete => {
                self.confirm_delete = true;
                None
            }
            Clicked::Save => match self.checked(taken) {
                Ok(session) => Some(Edited::Save(session)),
                Err(e) => {
                    self.error = Some(e);
                    None
                }
            },
        }
    }

    fn content(&mut self, ui: &mut egui::Ui) -> Option<Clicked> {
        if self.defaults {
            theme::hint(
                ui,
                "A new session starts with these. No password or passphrase is ever kept.",
            );
            ui.add_space(8.0);
        }
        egui::Grid::new("ssh-session-fields")
            .num_columns(2)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                if !self.defaults {
                    field(ui, "Name", &mut self.session.name, "Production web", 300.0);
                    field(
                        ui,
                        "Host",
                        &mut self.session.host,
                        "server.example.com",
                        300.0,
                    );
                }
                field(ui, "Port", &mut self.port, "22", 80.0);
                field(ui, "User name", &mut self.session.user, "", 300.0);
                let label = ui.label("Key files");
                let edit = egui::TextEdit::multiline(&mut self.key_files)
                    .desired_rows(2)
                    .hint_text("One path per line, e.g. ~/.ssh/id_ed25519 or a PuTTY .ppk");
                theme::sized_edit(ui, 300.0, edit).labelled_by(label.id);
                ui.end_row();
                field(
                    ui,
                    "Jump hosts",
                    &mut self.session.proxy_jump,
                    "bastion, me@gw.example.com:2222",
                    300.0,
                );
            });
        ui.add_space(10.0);
        theme::section_label(ui, "Agent");
        theme::switch_row(
            ui,
            "Sign on with the running agent's keys",
            &mut self.session.use_agent,
        );
        ui.add_enabled_ui(self.session.use_agent, |ui| {
            egui::Grid::new("ssh-session-agent").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                field(ui, "Only the key", &mut self.session.agent_key, "SHA256:… (empty: any)", 300.0);
            });
            theme::switch_row(ui, "Forward the agent to the server", &mut self.session.agent_forwarding);
            if self.session.agent_forwarding {
                theme::warning(ui, "The server's administrators can then sign on elsewhere with your keys while you are connected.");
            }
        });
        ui.add_space(10.0);
        theme::section_label(ui, "Terminal");
        theme::switch_row(
            ui,
            "Programs that ask get the mouse (Shift+drag still selects)",
            &mut self.session.mouse_reporting,
        );
        if let Some(error) = &self.error {
            ui.add_space(6.0);
            theme::error(ui, error);
        }
        if self.confirm_delete {
            ui.add_space(6.0);
            theme::warning(
                ui,
                format!("Delete \"{}\"? Click Delete again.", self.session.name),
            );
        }
        ui.add_space(16.0);
        theme::button_row(ui, |ui| {
            if ui.add(theme::primary_button(ui, "Save")).clicked() {
                return Some(Clicked::Save);
            }
            if ui.button("Cancel").clicked() {
                return Some(Clicked::Cancel);
            }
            if self.index.is_some() && ui.add(theme::danger_button(ui, "Delete")).clicked() {
                return Some(Clicked::Delete);
            }
            None
        })
    }

    /// The session as typed, or what is wrong with it.
    fn checked(&self, taken: &[&str]) -> Result<SavedSession, String> {
        let mut session = SavedSession {
            name: self.session.name.trim().to_owned(),
            host: self.session.host.trim().to_owned(),
            user: self.session.user.trim().to_owned(),
            ..self.session.clone()
        };
        session.port = self
            .port
            .trim()
            .parse::<u16>()
            .ok()
            .filter(|p| *p > 0)
            .ok_or("The port is a number from 1 to 65535.")?;
        session.key_files = self
            .key_files
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(str::to_owned)
            .collect();
        self.session
            .proxy_jump
            .trim()
            .clone_into(&mut session.proxy_jump);
        jump::check(&session.proxy_jump).map_err(|e| format!("Jump hosts: {e}."))?;
        if self.defaults {
            return Ok(session);
        }
        if session.name.is_empty() {
            return Err("Give the session a name.".to_owned());
        }
        if session.name == DEFAULT_SETTINGS || taken.contains(&session.name.as_str()) {
            return Err(format!(
                "There is a session called \"{}\" already.",
                session.name
            ));
        }
        if session.host.is_empty() || session.host.chars().any(char::is_whitespace) {
            return Err("Name the server to connect to, without spaces.".to_owned());
        }
        Ok(session)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Clicked {
    Save,
    Cancel,
    Delete,
}

fn field(ui: &mut egui::Ui, name: &str, value: &mut String, hint: &str, width: f32) {
    let label = ui.label(name);
    theme::sized_edit(ui, width, egui::TextEdit::singleline(value).hint_text(hint))
        .labelled_by(label.id);
    ui.end_row();
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    fn prod() -> SavedSession {
        SavedSession {
            name: "Prod æøå".into(),
            host: "box.lan".into(),
            port: 2222,
            user: "me".into(),
            key_files: vec!["/home/me/.ssh/id_ed25519".into()],
            ..SavedSession::default()
        }
    }

    #[test]
    fn a_session_signs_on_with_its_keys_and_the_agent_r_ssh_11() {
        let session = prod();
        assert_eq!(session.target().label(), "me@box.lan:2222");
        let options = session.options().expect("no jump hosts");
        assert_eq!(options.jumps, []);
        assert_eq!(
            options.key_files,
            [std::path::PathBuf::from("/home/me/.ssh/id_ed25519")]
        );
        assert_eq!(options.agent_key, None);
        assert!(!options.agent_forwarding);
        let off = SavedSession {
            use_agent: false,
            agent_forwarding: true,
            ..prod()
        };
        let off = off.options().expect("no jump hosts");
        assert_eq!(off.agent, None);
        assert!(!off.agent_forwarding, "no agent, nothing to forward");
    }

    #[test]
    fn jump_hosts_sign_on_as_the_session_unless_told_and_with_its_keys_r_ssh_43() {
        let session = SavedSession {
            proxy_jump: "gw, ops@inner:2200".into(),
            ..prod()
        };
        let options = session.options_with(None).expect("readable");
        let jumps: Vec<String> = options.jumps.iter().map(|j| j.target.label()).collect();
        assert_eq!(jumps, ["me@gw", "ops@inner:2200"]);
        assert_eq!(options.jumps[0].key_files, options.key_files);
        let bad = SavedSession {
            proxy_jump: "gw:x".into(),
            ..prod()
        };
        let error = bad.options_with(None).expect_err("not readable");
        assert!(
            error.starts_with("The jump hosts cannot be used"),
            "{error}"
        );
    }

    #[test]
    fn another_window_gets_the_jump_hosts_on_its_command_line_r_ssh_43() {
        let session = SavedSession {
            proxy_jump: " gw ".into(),
            ..prod()
        };
        assert!(
            session
                .command_line(false)
                .ends_with(&["-J".to_owned(), "gw".to_owned()])
        );
    }

    #[test]
    fn another_window_gets_the_session_on_its_command_line_r_ssh_15() {
        assert_eq!(prod().command_line(true), ["-load", "Prod æøå"]);
        assert_eq!(
            prod().command_line(false),
            [
                "me@box.lan",
                "-p",
                "2222",
                "-i",
                "/home/me/.ssh/id_ed25519",
                "-a"
            ]
        );
    }

    #[test]
    fn a_new_session_starts_from_the_defaults_r_ssh_11() {
        let defaults = SavedSession {
            agent_forwarding: true,
            key_files: vec!["k".into()],
            ..SavedSession::default()
        };
        let target = Target {
            host: "h".into(),
            port: 22,
            user: "u".into(),
        };
        let new = SavedSession::from_defaults(&defaults, "N", &target);
        assert_eq!(
            (new.name.as_str(), new.host.as_str(), new.key_files.len()),
            ("N", "h", 1)
        );
        assert!(new.agent_forwarding);
    }

    #[test]
    fn the_editor_says_what_is_missing_or_taken() {
        let mut editor = Editor::new(SavedSession::default(), None);
        assert!(editor.checked(&[]).is_err(), "no name");
        editor.session.name = "Prod".into();
        assert!(editor.checked(&[]).is_err(), "no host");
        editor.session.host = "box".into();
        assert!(editor.checked(&["Prod"]).is_err(), "taken");
        editor.session.proxy_jump = "@gw".into();
        let error = editor.checked(&[]).expect_err("not readable");
        assert!(error.starts_with("Jump hosts:"), "{error}");
        editor.session.proxy_jump = " gw ".into();
        assert_eq!(
            editor.checked(&[]).map(|s| s.proxy_jump),
            Ok("gw".to_owned())
        );
        editor.port = "0".into();
        assert!(editor.checked(&[]).is_err(), "port");
        editor.port = "22".into();
        editor.key_files = " ~/.ssh/a \n\n b.ppk".into();
        assert_eq!(
            editor.checked(&[]).expect("valid").key_files,
            ["~/.ssh/a", "b.ppk"]
        );
        let defaults = Editor::defaults(&SavedSession::default());
        assert!(
            defaults.checked(&[]).is_ok(),
            "the defaults need no name or host"
        );
    }

    fn harness(editor: Editor) -> Harness<'static, (Editor, Option<Edited>)> {
        Harness::builder()
            .with_size(egui::vec2(900.0, 700.0))
            .build_ui_state(
                |ui, (editor, done): &mut (Editor, Option<Edited>)| {
                    parley_ui_egui::test_support::install_fonts(ui);
                    if let Some(edited) = editor.show(ui.ctx(), &[]) {
                        *done = Some(edited);
                    }
                },
                (editor, None),
            )
    }

    #[test]
    fn the_editors_buttons_stay_put_and_delete_asks_twice_r_ssh_11() {
        let mut h = harness(Editor::new(prod(), Some(0)));
        h.run_steps(3);
        let save = h.get_by_label("Save").rect();
        h.run_steps(5);
        assert_eq!(h.get_by_label("Save").rect(), save);
        h.get_by_label("Delete").click();
        h.run_steps(2);
        assert_eq!(h.state().1, None, "asked first");
        h.get_by_label("Delete").click();
        h.run_steps(2);
        assert_eq!(h.state().1, Some(Edited::Delete));
    }

    #[test]
    fn saving_returns_the_session_as_typed_r_ssh_11() {
        let mut h = harness(Editor::new(prod(), Some(0)));
        h.run_steps(2);
        h.get_by_label("Save").click();
        h.run_steps(2);
        assert_eq!(h.state().1, Some(Edited::Save(prod())));
    }
}
