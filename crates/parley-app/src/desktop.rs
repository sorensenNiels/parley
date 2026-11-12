//! Desktop launch: command-line options and the window.

use eframe::egui;
use parley_core::SessionConfig;
use parley_ui_egui::{ParleyUi, Profile};

use crate::keychain::SystemKeychain;
use crate::{App, OLD_CONFIG_KEY, SESSIONS_KEY, SETTINGS_KEY};

const USAGE: &str = "\
Usage: parley [OPTION] [HOST[:PORT]]

Opens Parley, a TN5250 terminal for IBM i. With HOST, connects to it straight away,
with the saved session for that host if there is one (else port 23, or 992 with --tls).
Without HOST, connects to the session marked \"connect on program start\", if any.

Options:
  --tls                Connect with TLS
  --screenshot FILE    After a few seconds, save the window as a PPM image and quit
                       (for development)
  --version            Print the version and quit
  --help               Print this help and quit";

/// What the command line asked for.
#[derive(Debug, Default)]
struct Args {
    host: Option<(String, Option<u16>)>,
    tls: bool,
    screenshot: Option<String>,
}

pub fn run() -> eframe::Result {
    let mut args = Args::default();
    let mut command_line = std::env::args().skip(1);
    while let Some(arg) = command_line.next() {
        match arg.as_str() {
            "--version" | "-V" => {
                println!("parley {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            "--help" | "-h" => {
                println!("{USAGE}");
                return Ok(());
            }
            "--tls" => args.tls = true,
            "--screenshot" => args.screenshot = command_line.next(),
            _ if !arg.starts_with('-') && args.host.is_none() => args.host = Some(host_port(&arg)),
            _ => {
                eprintln!("parley: unknown option '{arg}'\n\n{USAGE}");
                std::process::exit(2);
            }
        }
    }

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Parley")
        .with_app_id("parley")
        .with_inner_size([1000.0, 640.0])
        .with_min_inner_size([480.0, 320.0]);
    match eframe::icon_data::from_png_bytes(include_bytes!("../icons/256.png")) {
        Ok(icon) => viewport = viewport.with_icon(std::sync::Arc::new(icon)),
        Err(e) => eprintln!("parley: could not load the window icon: {e}"),
    }
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        "Parley",
        options,
        Box::new(move |cc| {
            let profiles = cc.storage.map(saved_profiles).unwrap_or_default();
            let mut ui = ParleyUi::new(profiles, Box::new(SystemKeychain));
            if let Some(settings) = cc.storage.and_then(|s| eframe::get_value(s, SETTINGS_KEY)) {
                ui.set_settings(settings);
            }
            ui.set_update_checker(std::sync::Arc::new(|| {
                parley_core::update::check(
                    parley_core::update::LATEST_RELEASE_URL,
                    env!("CARGO_PKG_VERSION"),
                )
                .map_err(|e| e.to_string())
            }));
            match eframe::icon_data::from_png_bytes(include_bytes!("../icons/64.png")) {
                Ok(icon) => ui.set_logo(egui::ColorImage::from_rgba_unmultiplied(
                    [icon.width as usize, icon.height as usize],
                    &icon.rgba,
                )),
                Err(e) => eprintln!("parley: could not load the menu bar icon: {e}"),
            }
            let connect_on_start = match args.host {
                Some((host, port)) => Some(command_line_profile(&ui, host, port, args.tls)),
                None => ui.startup_profile().cloned(),
            };
            Ok(Box::new(App {
                ui,
                title: String::new(),
                connect_on_start,
                screenshot: args.screenshot.map(crate::screenshot::Pending::new),
            }))
        }),
    )
}

/// The saved sessions, or one made from Parley 0.1's single remembered connection.
fn saved_profiles(storage: &dyn eframe::Storage) -> Vec<Profile> {
    if let Some(profiles) = eframe::get_value(storage, SESSIONS_KEY) {
        return profiles;
    }
    eframe::get_value::<SessionConfig>(storage, OLD_CONFIG_KEY)
        .filter(|config| !config.host.trim().is_empty())
        .map(|config| vec![Profile::for_config(config)])
        .unwrap_or_default()
}

/// The saved session for a host on the command line, or a new unsaved one.
fn command_line_profile(ui: &ParleyUi, host: String, port: Option<u16>, tls: bool) -> Profile {
    let mut profile = ui.profile_for_host(&host).cloned().unwrap_or_else(|| {
        Profile::for_config(SessionConfig {
            host,
            ..SessionConfig::default()
        })
    });
    let config = &mut profile.config;
    config.tls = tls || config.tls && port.is_none();
    config.port = port.unwrap_or(if config.tls { 992 } else { 23 });
    profile
}

/// `host`, `host:port`, or `[v6 address]:port`.
fn host_port(arg: &str) -> (String, Option<u16>) {
    if let Some((host, port)) = arg.rsplit_once(':')
        && let Ok(port) = port.parse()
        && (!host.contains(':') || host.starts_with('['))
    {
        let host = host.trim_start_matches('[').trim_end_matches(']');
        return (host.to_owned(), Some(port));
    }
    (arg.to_owned(), None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_and_port_are_split_from_the_command_line() {
        assert_eq!(host_port("pub400.com"), ("pub400.com".into(), None));
        assert_eq!(
            host_port("pub400.com:992"),
            ("pub400.com".into(), Some(992))
        );
        assert_eq!(host_port("[::1]:23"), ("::1".into(), Some(23)));
        assert_eq!(host_port("::1"), ("::1".into(), None));
    }
}
