//! Connects to a host, waits for the first screen that takes input, prints it, optionally
//! presses keys and prints each screen that follows, and disconnects. A quick check of a host
//! without the UI; with `PARLEY_TRACE=file` it also records what the host sent, for test fixtures.
//!
//! ```sh
//! cargo run -p parley-core --example probe -- HOST [PORT] [--tls] [--ccsid 277] [--wide] \
//!     [--keys help,reset,f12,enter,pageup,pagedown,f3,...] [--macro FILE]
//! ```
//!
//! `--macro FILE` plays a macro file (R-KEY-4) on the first screen and prints the screen it
//! ends on. A `password` step gets the password from the file `PARLEY_PASSWORD_FILE` names: the
//! owner's test profile for Claude's checks (D22); the file is never printed. Without it the
//! step gives up.

#![expect(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "a command-line tool"
)]

use std::thread;
use std::time::{Duration, Instant};

use parley_core::macros::Macro;
use parley_core::net::{Connection, LinkState};
use parley_core::password::Password;
use parley_core::{Aid, CodePage, Key, KeyboardState, MacroState, ScreenSize, SessionConfig};

fn main() {
    let mut config = SessionConfig::default();
    let mut keys = Vec::new();
    let mut macro_file = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--tls" => {
                config.tls = true;
                if config.port == 23 {
                    config.port = parley_core::session::TLS_PORT;
                }
            }
            "--wide" => config.screen_size = ScreenSize::Wide,
            "--macro" => macro_file = args.next(),
            "--keys" => {
                let list = args.next().unwrap_or_default();
                for name in list.split(',') {
                    let Some(key) = key(name) else {
                        eprintln!("probe: unknown key '{name}'");
                        std::process::exit(2);
                    };
                    keys.push((name.to_owned(), key));
                }
            }
            "--ccsid" => {
                let ccsid = args.next().and_then(|c| c.parse().ok()).unwrap_or(37);
                config.code_page = CodePage::ALL
                    .into_iter()
                    .find(|p| p.ccsid() == ccsid)
                    .unwrap_or_default();
            }
            _ if config.host.is_empty() => config.host = arg,
            _ => config.port = arg.parse().unwrap_or(config.port),
        }
    }
    if config.host.is_empty() {
        eprintln!("usage: probe HOST [PORT] [--tls] [--ccsid 37|277|1140|1142] [--wide]");
        std::process::exit(2);
    }

    let connection = Connection::open(config, || {});
    print_when_ready(&connection, "first screen");
    for (name, key) in keys {
        connection.session().key(key);
        print_when_ready(&connection, &format!("after {name}"));
    }
    if let Some(file) = macro_file {
        play(&connection, &file);
    }
}

/// Plays a macro file and prints the screen once it has ended (or after a minute).
fn play(connection: &Connection, file: &str) {
    let parsed = std::fs::read_to_string(file)
        .map_err(|e| e.to_string())
        .and_then(|text| Macro::parse(&text).map_err(|e| e.to_string()));
    let steps = match parsed {
        Ok(m) => m.steps,
        Err(e) => {
            eprintln!("probe: {file}: {e}");
            std::process::exit(2);
        }
    };
    connection.session().play(file, steps);
    let start = Instant::now();
    loop {
        let state = connection.session().macro_state();
        if state == MacroState::Off || start.elapsed() > Duration::from_secs(60) {
            break;
        }
        if state == MacroState::NeedsPassword {
            let password = password_from_file();
            connection.session().give_macro_password(password.as_ref());
        }
        thread::sleep(Duration::from_millis(50));
    }
    print_when_ready(connection, &format!("after macro {file}"));
}

/// The test profile's password from the file `PARLEY_PASSWORD_FILE` names (D22); `None` stops
/// the macro.
fn password_from_file() -> Option<Password> {
    let file = std::env::var_os("PARLEY_PASSWORD_FILE")?;
    match std::fs::read_to_string(&file) {
        Ok(text) => Some(Password::new(
            text.trim_end_matches(['\r', '\n']).to_owned(),
        )),
        Err(e) => {
            eprintln!("probe: cannot read PARLEY_PASSWORD_FILE: {e}");
            None
        }
    }
}

fn key(name: &str) -> Option<Key> {
    let aid = match name.to_ascii_lowercase().as_str() {
        "reset" => return Some(Key::Reset),
        "enter" => Aid::ENTER,
        "help" => Aid::HELP,
        "pageup" => Aid::ROLL_DOWN,
        "pagedown" => Aid::ROLL_UP,
        f => f
            .strip_prefix('f')
            .and_then(|n| n.parse().ok())
            .and_then(Aid::function)?,
    };
    Some(Key::Aid(aid))
}

/// Waits until the keyboard unlocks (or 20 seconds), then prints the screen.
fn print_when_ready(connection: &Connection, title: &str) {
    let start = Instant::now();
    // Give the host a moment to lock the keyboard after a key.
    thread::sleep(Duration::from_millis(300));
    loop {
        if let LinkState::Closed(reason) | LinkState::Dropped(reason) = connection.state() {
            eprintln!("probe: {reason}");
            std::process::exit(1);
        }
        let session = connection.session();
        if session.keyboard() == KeyboardState::Unlocked
            || start.elapsed() > Duration::from_secs(20)
        {
            println!("==== {title}");
            println!("{}", session.screen().text(session.code_page()));
            let (row, column) = session.screen().row_column(session.screen().cursor);
            println!(
                "-- keyboard: {:?}, cursor: row {} column {}, input fields: {}",
                session.keyboard(),
                row + 1,
                column + 1,
                session.screen().fields().len()
            );
            if let Some(notice) = session.notice() {
                println!("-- notice: {notice}");
            }
            return;
        }
        drop(session);
        thread::sleep(Duration::from_millis(50));
    }
}
