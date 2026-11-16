//! Desktop launch: command-line options and the window.

use eframe::egui;
use parley_core::SessionConfig;
use parley_ui_egui::{ParleyUi, Profile};

use crate::{App, OLD_CONFIG_KEY, PASSWORDS_REMOVED_KEY, SESSIONS_KEY, SETTINGS_KEY};

const USAGE: &str = "\
Usage: parley [OPTION] [HOST[:PORT]]

Opens Parley, a TN5250 terminal for IBM i. With HOST, connects to it straight away,
with the saved session for that host if there is one (else port 23, or 992 with --tls).
Without HOST, connects to the session marked \"connect on program start\", if any.

Options:
  --tls                Connect with TLS
  --sql [ARGS]         Start Parley SQL instead, with ARGS (see parley-sql --help)
  --screenshot FILE    After a few seconds, save the window as a PPM image and quit
                       (for development)
  --version            Print the version and quit
  --help               Print this help and quit";

/// What Parley's command line asks for.
#[derive(Debug, PartialEq, Eq)]
enum Command {
    Run(Args),
    Version,
    Help,
}

/// The window to open with.
#[derive(Debug, Default, PartialEq, Eq)]
struct Args {
    host: Option<(String, Option<u16>)>,
    tls: bool,
    screenshot: Option<String>,
}

/// Reads Parley's arguments (without the program's name and without `--sql`, which hands
/// the rest to Parley SQL first).
///
/// # Errors
/// An unknown option, a second host, or an option without its value, said for the terminal.
fn parse(args: impl IntoIterator<Item = String>) -> Result<Command, String> {
    let mut parsed = Args::default();
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--version" | "-V" => return Ok(Command::Version),
            "--help" | "-h" => return Ok(Command::Help),
            "--tls" => parsed.tls = true,
            "--screenshot" => {
                let file = args
                    .next()
                    .ok_or_else(|| format!("'{arg}' needs a value"))?;
                parsed.screenshot = Some(file);
            }
            _ if !arg.starts_with('-') && parsed.host.is_none() => {
                parsed.host = Some(host_port(&arg));
            }
            _ => return Err(format!("unknown option '{arg}'")),
        }
    }
    Ok(Command::Run(parsed))
}

pub fn run() -> eframe::Result {
    // R-NF-2: `parley --sql`, or a link named parley-sql to the AppImage, is Parley SQL.
    let all: Vec<String> = std::env::args().skip(1).collect();
    let argv0 = std::env::var("ARGV0").ok();
    if let Some(sql) = parley_app::sql_start::sql_args(&all, argv0.as_deref()) {
        match parley_app::sql_start::hand_over(&sql) {
            Ok(code) => std::process::exit(code),
            Err(e) => {
                eprintln!("parley: {e}");
                std::process::exit(1);
            }
        }
    }
    let args = match parse(all) {
        Ok(Command::Run(args)) => args,
        Ok(Command::Version) => {
            println!("parley {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        Ok(Command::Help) => {
            println!("{USAGE}");
            return Ok(());
        }
        Err(e) => {
            eprintln!("parley: {e}\n\n{USAGE}");
            std::process::exit(2);
        }
    };

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Parley 5250")
        .with_app_id("parley")
        .with_inner_size([1000.0, 640.0])
        .with_min_inner_size([480.0, 320.0]);
    match parley_app::window_icon() {
        Ok(icon) => viewport = viewport.with_icon(std::sync::Arc::new(icon)),
        Err(e) => eprintln!("parley: could not load the window icon: {e}"),
    }
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    // The name shown (D40). The settings folder comes from the app id `parley` above, so saved
    // sessions stay where they were.
    eframe::run_native(
        "Parley 5250",
        options,
        Box::new(move |cc| {
            let profiles = cc.storage.map(saved_profiles).unwrap_or_default();
            let passwords_removed = remove_saved_passwords(cc.storage, &profiles);
            let mut ui = ParleyUi::new(profiles);
            // The settings first: they say whether the Macros add-on reads its folder (R-ADD-2).
            if let Some(settings) = cc.storage.and_then(|s| eframe::get_value(s, SETTINGS_KEY)) {
                ui.set_settings(settings);
            }
            // R-SQL-70: Tools ▸ SQL… starts Parley SQL, installed beside Parley.
            ui.set_sql_starter(Box::new(parley_app::sql_start::start));
            ui.set_beep(Box::new(parley_app::beep::beep));
            match parley_app::macros::MacroFolder::beside_settings() {
                Some(folder) => ui.set_macro_store(Box::new(folder)),
                None => eprintln!("parley: no folder for macros on this system"),
            }
            ui.set_update_checker(parley_app::update_checker());
            match parley_app::logo() {
                Ok(logo) => ui.set_logo(logo),
                Err(e) => eprintln!("parley: could not load the menu bar icon: {e}"),
            }
            let connect_on_start = match args.host {
                Some((host, port)) => {
                    let saved = ui.profile_for_host(&host);
                    Some(command_line_profile(saved, host, port, args.tls))
                }
                None => ui.startup_profile().cloned(),
            };
            Ok(Box::new(App {
                ui,
                title: String::new(),
                connect_on_start,
                screenshot: args.screenshot.map(parley_app::screenshot::Pending::new),
                passwords_removed,
            }))
        }),
    )
}

/// Deletes the passwords Parley 0.2 saved for `profiles` from the keychain, once (R-NET-7);
/// true when none is left. One that could not be deleted is tried again at the next start.
fn remove_saved_passwords(storage: Option<&dyn eframe::Storage>, profiles: &[Profile]) -> bool {
    let done = storage.and_then(|s| eframe::get_value::<bool>(s, PASSWORDS_REMOVED_KEY));
    if done == Some(true) {
        return true;
    }
    let failed = crate::keychain::remove_saved_passwords(profiles.iter().map(|p| p.name.as_str()));
    for failure in &failed {
        eprintln!("parley: could not delete a saved password from the keychain: {failure}");
    }
    failed.is_empty()
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

/// The session for a host on the command line: `saved`, the saved session for it, or a new
/// unsaved one. A port on the command line wins; else the saved session keeps its own port
/// unless `--tls` turned TLS on, which takes TLS's port.
fn command_line_profile(
    saved: Option<&Profile>,
    host: String,
    port: Option<u16>,
    tls: bool,
) -> Profile {
    let mut profile = saved.cloned().unwrap_or_else(|| {
        Profile::for_config(SessionConfig {
            host,
            ..SessionConfig::default()
        })
    });
    let config = &mut profile.config;
    let was_tls = config.tls;
    config.tls = tls || config.tls && port.is_none();
    config.port = match port {
        Some(port) => port,
        None if saved.is_some() && config.tls == was_tls => config.port,
        None => SessionConfig::default_port(config.tls),
    };
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

    fn run(args: &[&str]) -> Result<Command, String> {
        parse(args.iter().map(|&a| a.to_owned()))
    }

    #[test]
    fn the_command_line_is_read_into_host_tls_and_screenshot_r_ui_3() {
        assert_eq!(
            run(&["--tls", "pub400.com:992", "--screenshot", "æøå.ppm"]),
            Ok(Command::Run(Args {
                host: Some(("pub400.com".into(), Some(992))),
                tls: true,
                screenshot: Some("æøå.ppm".into()),
            }))
        );
        assert_eq!(run(&[]), Ok(Command::Run(Args::default())));
        assert_eq!(run(&["--version"]), Ok(Command::Version));
        assert_eq!(run(&["-h", "--bogus"]), Ok(Command::Help));
    }

    #[test]
    fn screenshot_without_a_file_is_refused_r_ui_3() {
        let error = run(&["--screenshot"]).expect_err("a missing value");
        assert!(error.contains("needs a value"), "{error}");
    }

    #[test]
    fn an_unknown_option_or_a_second_host_is_refused_r_ui_3() {
        assert!(run(&["--bogus"]).is_err());
        assert!(run(&["one", "two"]).is_err());
    }

    fn saved(port: u16, tls: bool) -> Profile {
        let mut profile = Profile::for_config(SessionConfig {
            host: "ibmi".into(),
            port,
            tls,
            ..SessionConfig::default()
        });
        profile.name = "Prod æøå".into();
        profile
    }

    #[test]
    fn a_saved_session_on_the_command_line_keeps_its_port_r_ui_3() {
        for tls in [false, true] {
            let saved = saved(2323, tls);
            let profile = command_line_profile(Some(&saved), "ibmi".into(), None, false);
            assert_eq!(profile.config.port, 2323);
            assert_eq!(profile.config.tls, tls);
            assert_eq!(profile.name, "Prod æøå");
        }
    }

    #[test]
    fn the_command_lines_port_and_tls_override_a_saved_session_r_ui_3() {
        let plain = saved(2323, false);
        let with_port = command_line_profile(Some(&plain), "ibmi".into(), Some(24), false);
        assert_eq!(with_port.config.port, 24);
        let with_tls = command_line_profile(Some(&plain), "ibmi".into(), None, true);
        assert!(with_tls.config.tls);
        assert_eq!(with_tls.config.port, 992);
        let new = command_line_profile(None, "other".into(), None, false);
        assert_eq!((new.config.host.as_str(), new.config.port), ("other", 23));
        let new_tls = command_line_profile(None, "other".into(), None, true);
        assert_eq!(new_tls.config.port, 992);
    }

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
