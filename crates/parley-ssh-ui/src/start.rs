//! The start screen (R-SSH-2): host, port and user name, and Connect. Saved sessions replace the plain form in
//! Phase 27 (R-SSH-11).

use egui::{Key, Ui};
use parley_ssh_core::net::Target;
use parley_ui_egui::theme;

use crate::settings::Settings;

/// The form's fields, as typed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Form {
    pub host: String,
    pub port: String,
    pub user: String,
    pub error: Option<String>,
    focus: bool,
}

impl Form {
    /// The form as it was last used.
    pub fn from_settings(settings: &Settings) -> Self {
        Self {
            host: settings.last_host.clone(),
            port: settings.last_port.to_string(),
            user: settings.last_user.clone(),
            error: None,
            focus: true,
        }
    }

    /// Draws the form; returns where to connect once Connect or Enter is pressed with valid fields.
    pub fn show(&mut self, ui: &mut Ui) -> Option<Target> {
        let mut go = false;
        ui.vertical_centered(|ui| {
            ui.add_space((ui.available_height() * 0.2).max(24.0));
            theme::card(ui).show(ui, |ui| {
                ui.set_width(380.0);
                ui.heading("Connect to a server");
                theme::hint(
                    ui,
                    "Over SSH. You sign on with the password or a one-time code.",
                );
                ui.add_space(12.0);
                egui::Grid::new("ssh-start")
                    .num_columns(2)
                    .spacing([12.0, 8.0])
                    .show(ui, |ui| {
                        let label = ui.label("Host");
                        let host = egui::TextEdit::singleline(&mut self.host)
                            .hint_text("server.example.com or user@server");
                        let host = theme::sized_edit(ui, 250.0, host).labelled_by(label.id);
                        if std::mem::take(&mut self.focus) {
                            host.request_focus();
                        }
                        go |= host.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                        ui.end_row();
                        let label = ui.label("Port");
                        let port =
                            theme::sized_edit(ui, 80.0, egui::TextEdit::singleline(&mut self.port))
                                .labelled_by(label.id);
                        go |= port.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                        ui.end_row();
                        let label = ui.label("User name");
                        let user = theme::sized_edit(
                            ui,
                            250.0,
                            egui::TextEdit::singleline(&mut self.user),
                        )
                        .labelled_by(label.id);
                        go |= user.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                        ui.end_row();
                    });
                if let Some(error) = &self.error {
                    ui.add_space(6.0);
                    theme::error(ui, error);
                }
                ui.add_space(12.0);
                theme::button_row(ui, |ui| {
                    go |= ui.add(theme::primary_button(ui, "Connect")).clicked();
                });
            });
        });
        if !go {
            return None;
        }
        match parse(&self.host, &self.port, &self.user) {
            Ok(target) => {
                self.error = None;
                Some(target)
            }
            Err(e) => {
                self.error = Some(e);
                None
            }
        }
    }
}

/// The fields as a [`Target`]. The host may carry the user and port, as `ssh` takes them: `user@host:port`;
/// `[::1]:2222` for an IPv6 address with a port.
pub fn parse(host: &str, port: &str, user: &str) -> Result<Target, String> {
    let mut host = host.trim();
    let mut user = user.trim().to_owned();
    if let Some((name, rest)) = host.rsplit_once('@') {
        name.trim().clone_into(&mut user);
        host = rest.trim();
    }
    let mut port = port.trim().to_owned();
    if let Some(rest) = host.strip_prefix('[') {
        let (address, after) = rest
            .split_once(']')
            .ok_or("An IPv6 address in brackets needs its ]")?;
        if let Some(p) = after.strip_prefix(':') {
            p.clone_into(&mut port);
        }
        host = address;
    } else if host.matches(':').count() == 1
        && let Some((name, p)) = host.split_once(':')
    {
        host = name;
        p.clone_into(&mut port);
    }
    if host.is_empty() {
        return Err("Name the server to connect to.".to_owned());
    }
    if host.chars().any(char::is_whitespace) {
        return Err("A host name has no spaces.".to_owned());
    }
    if user.is_empty() {
        return Err("Fill in the user name to sign on as.".to_owned());
    }
    let port = if port.is_empty() {
        22
    } else {
        port.parse::<u16>()
            .ok()
            .filter(|p| *p > 0)
            .ok_or("The port is a number from 1 to 65535.")?
    };
    Ok(Target {
        host: host.to_owned(),
        port,
        user,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(host: &str, port: u16, user: &str) -> Target {
        Target {
            host: host.into(),
            port,
            user: user.into(),
        }
    }

    #[test]
    fn the_fields_make_a_target_r_ssh_2() {
        assert_eq!(
            parse(" server ", "22", "æøå"),
            Ok(target("server", 22, "æøå"))
        );
        assert_eq!(parse("server", "", "me"), Ok(target("server", 22, "me")));
    }

    #[test]
    fn the_host_may_carry_user_and_port_as_ssh_takes_them() {
        assert_eq!(
            parse("root@box:2222", "22", "me"),
            Ok(target("box", 2222, "root"))
        );
        assert_eq!(
            parse("[::1]:2200", "22", "me"),
            Ok(target("::1", 2200, "me"))
        );
        assert_eq!(
            parse("fe80::1", "22", "me"),
            Ok(target("fe80::1", 22, "me"))
        );
    }

    #[test]
    fn wrong_fields_are_said_in_plain_words() {
        assert!(parse("", "22", "me").is_err());
        assert!(parse("a b", "22", "me").is_err());
        assert!(parse("server", "22", "").is_err());
        assert!(parse("server", "0", "me").is_err());
        assert!(parse("server", "70000", "me").is_err());
        assert!(parse("[::1", "22", "me").is_err());
    }
}
