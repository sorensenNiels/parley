//! A session's connection and what runs on the server (R-SSH-26..29, D51): keepalives and the IP version,
//! rekeying and compression, a proxy, a command instead of the shell, environment variables and the terminal
//! type; and the session editor's Connection and On the server pages.

use std::time::Duration;

use egui::Ui;
use parley_ssh_core::net::{AddressFamily, Proxy, Shell, Transport};
use parley_ui_egui::theme;
use serde::{Deserialize, Serialize};

use super::SavedSession;

/// The IP version, as the settings keep it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Family {
    #[default]
    Any,
    V4,
    V6,
}

/// The kind of proxy, as the settings keep it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProxyKind {
    #[default]
    None,
    Socks5,
    Http,
    Command,
}

/// How the connection is kept and carried.
pub fn transport(s: &SavedSession) -> Transport {
    let proxy = match s.proxy {
        ProxyKind::None => Proxy::None,
        ProxyKind::Socks5 => Proxy::Socks5 {
            host: s.proxy_host.trim().to_owned(),
            port: s.proxy_port,
            user: s.proxy_user.trim().to_owned(),
        },
        ProxyKind::Http => Proxy::Http {
            host: s.proxy_host.trim().to_owned(),
            port: s.proxy_port,
            user: s.proxy_user.trim().to_owned(),
        },
        ProxyKind::Command => Proxy::Command(s.proxy_command.trim().to_owned()),
    };
    Transport {
        keepalive: (s.keepalive > 0).then(|| Duration::from_secs(u64::from(s.keepalive))),
        family: match s.address_family {
            Family::Any => AddressFamily::Any,
            Family::V4 => AddressFamily::V4,
            Family::V6 => AddressFamily::V6,
        },
        rekey_after: Duration::from_secs(u64::from(s.rekey_minutes.max(1)) * 60),
        rekey_bytes: usize::from(s.rekey_mb.clamp(1, 1024)) << 20,
        compression: s.compression,
        proxy,
    }
}

/// What runs on the server.
///
/// # Errors
/// An environment line that is not `NAME=value`, said for the user.
pub fn shell(s: &SavedSession) -> Result<Shell, String> {
    let command = Some(s.remote_command.trim().to_owned()).filter(|c| !c.is_empty());
    let terminal_type = Some(s.terminal_type.trim().to_owned())
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| Shell::default().terminal_type);
    Ok(Shell {
        command,
        env: environment(&s.environment)?,
        terminal_type,
    })
}

/// `NAME=value` lines; blank lines and those starting with `#` are skipped.
///
/// # Errors
/// A line without `=` or with a name that is not letters, digits and `_`.
pub fn environment(text: &str) -> Result<Vec<(String, String)>, String> {
    let mut env = Vec::new();
    for line in text.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (name, value) = line
            .split_once('=')
            .ok_or_else(|| format!("\"{line}\" is not NAME=value"))?;
        let name = name.trim();
        if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return Err(format!(
                "\"{name}\" is not a variable name (letters, digits and _)"
            ));
        }
        env.push((name.to_owned(), value.to_owned()));
    }
    Ok(env)
}

/// What is wrong with the proxy, if anything.
pub fn check_proxy(s: &SavedSession) -> Result<(), String> {
    match s.proxy {
        ProxyKind::Socks5 | ProxyKind::Http if s.proxy_host.trim().is_empty() => {
            Err("Name the proxy's host.".to_owned())
        }
        ProxyKind::Command if s.proxy_command.trim().is_empty() => {
            Err("Write the proxy command, e.g. nc -X connect -x proxy:3128 %h %p.".to_owned())
        }
        ProxyKind::None | ProxyKind::Socks5 | ProxyKind::Http | ProxyKind::Command => Ok(()),
    }
}

fn number(
    ui: &mut Ui,
    name: &str,
    value: &mut u16,
    range: std::ops::RangeInclusive<u16>,
    suffix: &str,
) {
    let label = ui.label(name);
    ui.add(egui::DragValue::new(value).range(range).suffix(suffix))
        .labelled_by(label.id);
    ui.end_row();
}

fn text(ui: &mut Ui, name: &str, value: &mut String, hint: &str) {
    let label = ui.label(name);
    theme::sized_edit(ui, 300.0, egui::TextEdit::singleline(value).hint_text(hint))
        .labelled_by(label.id);
    ui.end_row();
}

/// The editor's Connection page: keeping it, the IP version, rekeying, compression and the proxy.
pub fn connection_page(ui: &mut Ui, s: &mut SavedSession) {
    egui::Grid::new("ssh-session-connection")
        .num_columns(2)
        .spacing([12.0, 8.0])
        .show(ui, |ui| {
            number(ui, "Keepalive every", &mut s.keepalive, 0..=3600, " s");
            ui.label("IP version");
            theme::segmented(
                ui,
                &mut s.address_family,
                &[
                    (Family::Any, "Either"),
                    (Family::V4, "IPv4"),
                    (Family::V6, "IPv6"),
                ],
            );
            ui.end_row();
            number(ui, "New keys after", &mut s.rekey_minutes, 1..=1440, " min");
            number(ui, "or after", &mut s.rekey_mb, 1..=1024, " MB");
        });
    theme::hint(
        ui,
        "Keepalive 0: none. Three unanswered keepalives count as a lost connection.",
    );
    theme::switch_row(ui, "Compress (for slow links)", &mut s.compression);
    ui.add_space(10.0);
    theme::section_label(ui, "Proxy");
    theme::segmented(
        ui,
        &mut s.proxy,
        &[
            (ProxyKind::None, "None"),
            (ProxyKind::Socks5, "SOCKS 5"),
            (ProxyKind::Http, "HTTP"),
            (ProxyKind::Command, "Command"),
        ],
    );
    ui.add_space(6.0);
    egui::Grid::new("ssh-session-proxy")
        .num_columns(2)
        .spacing([12.0, 8.0])
        .show(ui, |ui| match s.proxy {
            ProxyKind::None => {}
            ProxyKind::Socks5 | ProxyKind::Http => {
                text(ui, "Proxy host", &mut s.proxy_host, "proxy.example.com");
                number(ui, "Proxy port", &mut s.proxy_port, 1..=65535, "");
                text(ui, "Proxy user", &mut s.proxy_user, "empty: none");
            }
            ProxyKind::Command => {
                text(
                    ui,
                    "Command",
                    &mut s.proxy_command,
                    "nc -X connect -x proxy:3128 %h %p",
                );
            }
        });
    match s.proxy {
        ProxyKind::Socks5 | ProxyKind::Http => {
            theme::hint(
                ui,
                "A proxy password is asked when the proxy wants one, never kept.",
            );
        }
        ProxyKind::Command => theme::hint(ui, "%h, %p and %r are the host, port and user name."),
        ProxyKind::None => {}
    }
}

/// The editor's On the server page: a command, environment variables and the terminal type.
pub fn server_page(ui: &mut Ui, s: &mut SavedSession) {
    egui::Grid::new("ssh-session-server")
        .num_columns(2)
        .spacing([12.0, 8.0])
        .show(ui, |ui| {
            text(
                ui,
                "Run instead of the shell",
                &mut s.remote_command,
                "tmux new -A -s main",
            );
            let label = ui.label("Environment");
            let edit = egui::TextEdit::multiline(&mut s.environment)
                .desired_rows(3)
                .hint_text("One NAME=value per line, e.g. LANG=da_DK.UTF-8");
            theme::sized_edit(ui, 300.0, edit).labelled_by(label.id);
            ui.end_row();
            text(ui, "Terminal type", &mut s.terminal_type, "xterm-256color");
        });
    theme::hint(
        ui,
        "The server takes only the variables its configuration allows (OpenSSH's AcceptEnv); the event log says which.",
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_session_gives_its_transport_and_shell_r_ssh_26_r_ssh_27_r_ssh_28() {
        let s = SavedSession {
            keepalive: 0,
            address_family: Family::V6,
            rekey_minutes: 30,
            rekey_mb: 4096,
            compression: true,
            remote_command: "  tmux attach ".into(),
            environment: "# mine\nLANG=da_DK.UTF-8\n\nEDITOR = vim\n".into(),
            terminal_type: " ".into(),
            ..SavedSession::default()
        };
        let t = transport(&s);
        assert_eq!(t.keepalive, None);
        assert_eq!(t.family, AddressFamily::V6);
        assert_eq!(t.rekey_after, Duration::from_mins(30));
        assert_eq!(t.rekey_bytes, 1 << 30, "at most 1 GiB");
        assert!(t.compression);
        let sh = shell(&s).expect("readable");
        assert_eq!(sh.command.as_deref(), Some("tmux attach"));
        assert_eq!(
            sh.env,
            [
                ("LANG".into(), "da_DK.UTF-8".into()),
                ("EDITOR".into(), " vim".into())
            ]
        );
        assert_eq!(sh.terminal_type, "xterm-256color");
        assert_eq!(
            transport(&SavedSession::default()).keepalive,
            Some(Duration::from_secs(30))
        );
    }

    #[test]
    fn environment_lines_that_cannot_be_read_are_said_r_ssh_28() {
        assert!(
            environment("JUST_A_NAME")
                .expect_err("no =")
                .contains("NAME=value")
        );
        assert!(
            environment("MY-VAR=1")
                .expect_err("bad name")
                .contains("not a variable name")
        );
        assert_eq!(
            environment("A=æøå=1"),
            Ok(vec![("A".into(), "æøå=1".into())])
        );
    }

    #[test]
    fn a_proxy_needs_its_host_or_command_r_ssh_29() {
        let mut s = SavedSession {
            proxy: ProxyKind::Http,
            ..SavedSession::default()
        };
        assert!(check_proxy(&s).is_err());
        s.proxy_host = "px".into();
        assert!(check_proxy(&s).is_ok());
        assert_eq!(
            transport(&s).proxy,
            Proxy::Http {
                host: "px".into(),
                port: 1080,
                user: String::new()
            }
        );
        s.proxy = ProxyKind::Command;
        assert!(check_proxy(&s).is_err());
    }
}
