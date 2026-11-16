//! Parley SSH's command line (R-SSH-14), as `PuTTY`'s and OpenSSH's: `parley-ssh [user@]host[:port]`, `-p`, `-l`,
//! `-i`, `-load`, `-A`/`-a`, `-J` (R-SSH-43); starting another Parley SSH window (R-SSH-15); and starting Parley SSH from Parley
//! 5250's Tools ▸ SSH… (R-SSH-59).

use parley_ssh_ui::StartRequest;

/// The usage text of `parley-ssh`.
pub const USAGE: &str = "\
Usage: parley-ssh [OPTION]... [[USER@]HOST[:PORT] [COMMAND...]]

Opens Parley SSH, an SSH client. With HOST, connects to it at once, signing on as the
default settings say unless told otherwise; with COMMAND, runs it instead of the shell.

Options:
  -load NAME           Open the saved session NAME
  -p PORT              Connect to PORT (default 22)
  -l USER              Sign on as USER
  -i FILE              Try the private key in FILE (OpenSSH or PuTTY .ppk); may be repeated
  -A                   Forward the agent to the server
  -a                   Do not forward the agent
  -J [USER@]HOST[:PORT][,...]
                       Through these jump hosts, in order (none: no jump host)
  --screenshot FILE    After a few seconds, save the window as a PPM image and quit
                       (for development)
  --version            Print the version and quit
  --help               Print this help and quit";

/// What `parley-ssh`'s command line asks for.
#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Run {
        request: StartRequest,
        screenshot: Option<String>,
    },
    Version,
    Help,
}

/// Reads `parley-ssh`'s arguments (without the program's name).
///
/// # Errors
/// An unknown option, a missing or wrong value, said for the terminal.
pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Command, String> {
    let mut request = StartRequest::default();
    let mut screenshot = None;
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        let mut value = || args.next().ok_or_else(|| format!("'{arg}' needs a value"));
        match arg.as_str() {
            "--version" | "-V" => return Ok(Command::Version),
            "--help" | "-h" => return Ok(Command::Help),
            "--screenshot" => screenshot = Some(value()?),
            "-load" | "--load" => request.load = Some(value()?),
            "-p" | "-P" => {
                let port = value()?;
                let parsed = port.parse::<u16>().ok().filter(|p| *p > 0);
                request.port =
                    Some(parsed.ok_or_else(|| format!("'{port}' is not a port (1 to 65535)"))?);
            }
            "-l" => request.user = Some(value()?),
            "-i" => request.key_files.push(value()?),
            "-A" => request.agent_forwarding = Some(true),
            "-a" => request.agent_forwarding = Some(false),
            "-J" => {
                let jumps = value()?;
                parley_ssh_core::net::jump::check(&jumps).map_err(|e| format!("-J: {e}"))?;
                request.proxy_jump = Some(jumps);
            }
            _ if !arg.starts_with('-') && request.host.is_none() => request.host = Some(arg),
            // As `ssh host command…`: the rest of the line is the command (R-SSH-28).
            _ if !arg.starts_with('-') => {
                let rest: Vec<String> = std::iter::once(arg).chain(args.by_ref()).collect();
                request.command = Some(rest.join(" "));
            }
            _ => return Err(format!("unknown option '{arg}'")),
        }
    }
    if request.load.is_some() && request.host.is_some() {
        return Err("give -load NAME or a host, not both".to_owned());
    }
    Ok(Command::Run {
        request,
        screenshot,
    })
}

/// Starts another Parley SSH window with `args` (R-SSH-15): this program again, which runs on its own.
///
/// # Errors
/// When the program cannot be found or started, said for the user.
pub fn spawn(args: &[String]) -> Result<(), String> {
    let program = std::env::current_exe()
        .map_err(|e| format!("Parley SSH could not find its own program: {e}"))?;
    let mut child = std::process::Command::new(program)
        .args(args)
        .spawn()
        .map_err(|e| format!("Another Parley SSH window could not be started: {e}"))?;
    // It runs on its own; waiting on a thread only collects its exit status, so it leaves no zombie process
    // behind on Unix. What it returns is of no use here.
    std::thread::spawn(move || child.wait().ok());
    Ok(())
}

/// The arguments that open Parley SSH on a Parley 5250 session's host (R-SSH-59): the host alone, on SSH's port,
/// as the user Parley SSH's defaults name. Parley 5250 knows no user and passes no password (D16).
fn args_for(config: &parley_core::SessionConfig) -> Vec<String> {
    vec![config.host.trim().to_owned()]
}

/// Starts Parley SSH, installed beside Parley 5250, on the host of Parley 5250's session (R-SSH-59). The
/// session's name is not passed: Parley SSH's saved sessions are its own.
///
/// # Errors
/// When Parley SSH is not beside Parley or cannot be started, said for the user.
pub fn start(_name: &str, config: &parley_core::SessionConfig) -> Result<(), String> {
    let program = crate::siblings::Sibling::Ssh.beside()?;
    let mut child = std::process::Command::new(&program)
        .args(args_for(config))
        .spawn()
        .map_err(|e| format!("Parley SSH could not be started: {e}"))?;
    // Parley SSH runs on its own; waiting on a thread only collects its exit status, so it leaves no zombie
    // process behind on Unix. What it returns is of no use to Parley 5250.
    std::thread::spawn(move || child.wait().ok());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(text: &str) -> Result<StartRequest, String> {
        match parse(text.split_whitespace().map(str::to_owned))? {
            Command::Run { request, .. } => Ok(request),
            other => Err(format!("{other:?}")),
        }
    }

    #[test]
    fn a_host_with_options_as_ssh_and_putty_take_them_r_ssh_14() {
        let request = run("-l root -p 2222 -i ~/.ssh/a -i b.ppk -A box.lan").expect("valid");
        assert_eq!(request.host.as_deref(), Some("box.lan"));
        assert_eq!(request.user.as_deref(), Some("root"));
        assert_eq!(request.port, Some(2222));
        assert_eq!(request.key_files, ["~/.ssh/a", "b.ppk"]);
        assert_eq!(request.agent_forwarding, Some(true));
        assert_eq!(
            run("me@æøå.lan:2200 -a").expect("valid").host.as_deref(),
            Some("me@æøå.lan:2200")
        );
    }

    #[test]
    fn what_follows_the_host_is_a_command_as_ssh_takes_it_r_ssh_28() {
        let request = run("-p 2200 box tail -f /var/log/syslog -n 5").expect("valid");
        assert_eq!(request.host.as_deref(), Some("box"));
        assert_eq!(request.port, Some(2200));
        assert_eq!(
            request.command.as_deref(),
            Some("tail -f /var/log/syslog -n 5")
        );
        assert_eq!(run("box").expect("valid").command, None);
    }

    #[test]
    fn jump_hosts_on_the_command_line_as_ssh_takes_them_r_ssh_43() {
        let request = run("-J gw,ops@inner:2200 box").expect("valid");
        assert_eq!(request.proxy_jump.as_deref(), Some("gw,ops@inner:2200"));
        assert_eq!(
            run("-J none box").expect("valid").proxy_jump.as_deref(),
            Some("none")
        );
        let error = run("-J gw:x box").expect_err("not a port");
        assert!(
            error.starts_with("-J:") && error.contains("not a port"),
            "{error}"
        );
        assert!(
            run("box -J")
                .expect_err("no value")
                .contains("needs a value")
        );
    }

    #[test]
    fn a_saved_session_by_name_r_ssh_14() {
        let request = run("-load Prod").expect("valid");
        assert_eq!(request.load.as_deref(), Some("Prod"));
        assert_eq!(run(""), Ok(StartRequest::default()));
    }

    /// R-SSH-59: what Parley 5250 passes reads back as its session's host, with no user or port.
    #[test]
    fn parley_5250_passes_the_sessions_host_alone_r_ssh_59() {
        let config = parley_core::SessionConfig {
            host: " ibmi.æøå.lan ".to_owned(),
            ..parley_core::SessionConfig::default()
        };
        let request = match parse(args_for(&config)) {
            Ok(Command::Run { request, .. }) => request,
            other => panic!("not a run: {other:?}"),
        };
        assert_eq!(
            request,
            StartRequest {
                host: Some("ibmi.æøå.lan".to_owned()),
                ..StartRequest::default()
            }
        );
    }

    #[test]
    fn wrong_arguments_are_said_r_ssh_14() {
        assert!(run("-p").is_err());
        assert!(run("-p 0 box").is_err());
        assert!(run("-p http box").is_err());
        assert!(run("-x box").is_err());
        assert!(run("box -x").is_err(), "an unknown option after the host");
        assert!(run("-load P box").is_err());
        assert_eq!(parse(["--version".to_owned()]), Ok(Command::Version));
        assert_eq!(parse(["-h".to_owned()]), Ok(Command::Help));
    }
}
