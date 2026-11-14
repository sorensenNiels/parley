//! Parley SQL's command line, and starting Parley SQL from Parley's Tools ▸ SQL… (R-SQL-70,
//! D29): the same program as `make run-sql`, told which host to open.

use parley_core::CodePage;
use parley_ui_egui::SqlConnection;

/// The usage text of `parley-sql`.
pub const USAGE: &str = "\
Usage: parley-sql [OPTION] [CONNECTION]

Opens Parley SQL, the SQL window of Parley on its own, for SQL on IBM i. With CONNECTION,
the name of a saved connection, opens its sign-on straight away.

Options:
  --host HOST          Open the sign-on for HOST: a saved connection to it, or a new one
                       saved with the options below (Parley's Tools > SQL... uses these)
  --name NAME          The new connection's name (default: HOST)
  --tls | --no-tls     Whether the new connection uses TLS (default: TLS)
  --ca-file FILE       A PEM file of CAs to trust besides the system's own
  --ccsid N            The new connection's code page: 37, 277, 1140 or 1142
  --screenshot FILE    After a few seconds, save the window as a PPM image and quit
                       (for development)
  --version            Print the version and quit
  --help               Print this help and quit";

/// What `parley-sql`'s command line asks for.
#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Run(SqlArgs),
    Version,
    Help,
}

/// The window to open with.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct SqlArgs {
    /// A saved connection's name.
    pub connection: Option<String>,
    /// A host to open, from `--host` and the options that go with it.
    pub start: Option<SqlConnection>,
    pub screenshot: Option<String>,
}

/// Reads `parley-sql`'s arguments (without the program's name).
///
/// # Errors
/// An unknown option, a missing value, or an unknown CCSID, said for the terminal.
pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Command, String> {
    let mut parsed = SqlArgs::default();
    let mut start: Option<SqlConnection> = None;
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        let mut value = || args.next().ok_or_else(|| format!("'{arg}' needs a value"));
        match arg.as_str() {
            "--version" | "-V" => return Ok(Command::Version),
            "--help" | "-h" => return Ok(Command::Help),
            "--screenshot" => parsed.screenshot = Some(value()?),
            "--host" => start.get_or_insert_default().host = value()?,
            "--name" => start.get_or_insert_default().name = value()?,
            "--tls" => start.get_or_insert_default().tls = true,
            "--no-tls" => start.get_or_insert_default().tls = false,
            "--ca-file" => start.get_or_insert_default().ca_file = value()?,
            "--ccsid" => {
                let text = value()?;
                let code_page = CodePage::ALL
                    .into_iter()
                    .find(|c| c.ccsid().to_string() == text)
                    .ok_or_else(|| format!("unknown CCSID '{text}'"))?;
                start.get_or_insert_default().code_page = code_page;
            }
            _ if !arg.starts_with('-') && parsed.connection.is_none() => {
                parsed.connection = Some(arg);
            }
            _ => return Err(format!("unknown option '{arg}'")),
        }
    }
    if let Some(mut start) = start {
        if start.host.trim().is_empty() {
            return Err("the connection options need --host".to_owned());
        }
        if start.name.trim().is_empty() {
            start.name.clone_from(&start.host);
        }
        parsed.start = Some(start);
    }
    Ok(Command::Run(parsed))
}

/// The arguments that make Parley SQL open `connection`'s host (R-SQL-70).
#[must_use]
pub fn args_for(connection: &SqlConnection) -> Vec<String> {
    let mut args = vec![
        "--host".to_owned(),
        connection.host.clone(),
        "--name".to_owned(),
        connection.name.clone(),
        if connection.tls { "--tls" } else { "--no-tls" }.to_owned(),
        "--ccsid".to_owned(),
        connection.code_page.ccsid().to_string(),
    ];
    if !connection.ca_file.trim().is_empty() {
        args.extend(["--ca-file".to_owned(), connection.ca_file.clone()]);
    }
    args
}

/// Starts Parley SQL, installed beside this program, on `connection`'s host (R-SQL-70).
///
/// # Errors
/// When Parley SQL is not beside Parley or cannot be started, said for the operator.
pub fn start(connection: &SqlConnection) -> Result<(), String> {
    let program = std::env::current_exe()
        .map_err(|e| format!("Parley could not find its own folder: {e}"))?
        .with_file_name(format!("parley-sql{}", std::env::consts::EXE_SUFFIX));
    if !program.is_file() {
        return Err(format!(
            "Parley SQL is not installed beside Parley (looked for {}).",
            program.display()
        ));
    }
    let mut child = std::process::Command::new(&program)
        .args(args_for(connection))
        .spawn()
        .map_err(|e| format!("Parley SQL could not be started: {e}"))?;
    // Parley SQL runs on its own; waiting on a thread only collects its exit status, so it
    // leaves no zombie process behind on Unix. What it returns is of no use to Parley.
    std::thread::spawn(move || child.wait().ok());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(text: &str) -> Vec<String> {
        text.split_whitespace().map(str::to_owned).collect()
    }

    #[test]
    fn the_arguments_parley_passes_open_the_same_connection_r_sql_70() {
        let connection = SqlConnection {
            name: "Prod æøå".to_owned(),
            host: "ibmi.example.com".to_owned(),
            tls: false,
            ca_file: "/etc/ca.pem".to_owned(),
            code_page: CodePage::Ccsid277,
            user: String::new(),
        };
        let parsed = parse(args_for(&connection)).expect("Parley's own arguments parse");
        let Command::Run(parsed) = parsed else {
            panic!("not a run: {parsed:?}");
        };
        assert_eq!(parsed.start, Some(connection));
        assert_eq!(parsed.connection, None);
    }

    #[test]
    fn a_host_alone_is_named_after_itself_with_tls_r_sql_70() {
        let Ok(Command::Run(parsed)) = parse(args("--host pub400.com")) else {
            panic!("parses");
        };
        let start = parsed.start.expect("a host to open");
        assert_eq!(start.name, "pub400.com");
        assert!(start.tls, "TLS by default, as R-P-3");
    }

    #[test]
    fn a_saved_connection_and_the_plain_options_still_work_r_sql_45() {
        assert_eq!(parse(args("--version")), Ok(Command::Version));
        assert_eq!(parse(args("-h")), Ok(Command::Help));
        let Ok(Command::Run(parsed)) = parse(args("Prod --screenshot a.ppm")) else {
            panic!("parses");
        };
        assert_eq!(parsed.connection.as_deref(), Some("Prod"));
        assert_eq!(parsed.screenshot.as_deref(), Some("a.ppm"));
        assert_eq!(parsed.start, None);
    }

    #[test]
    fn bad_arguments_are_errors_not_panics_r_sql_70() {
        for bad in ["--host", "--ccsid 500 --host h", "--tls", "--bogus", "a b"] {
            assert!(parse(args(bad)).is_err(), "{bad}");
        }
    }
}
