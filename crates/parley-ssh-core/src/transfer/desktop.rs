//! A copy between the desktop and a server (R-SSH-55): rsync on the desktop, with Parley SSH's relay as its remote
//! shell, so the server is reached through the session's open connection ([`crate::net::pipe`]).

use super::run::{Mode, Options, quote};

/// The name rsync is given for the server; the relay ignores it, as the window knows which session it is.
const HOST: &str = "parley";

/// A copy between the desktop and a server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Desktop {
    /// The relay rsync runs as its remote shell: the program and its arguments (`parley-ssh --relay PORT TOKEN`).
    pub relay: Vec<String>,
    /// The server's rsync, as its check found it.
    pub server_rsync: String,
    /// The path on the desktop, in the form the desktop's rsync takes ([`msys_path`] on Windows).
    pub desktop_path: String,
    pub server_path: String,
    /// From the desktop to the server; else from the server to the desktop.
    pub upload: bool,
    pub options: Options,
}

/// The arguments for the desktop's rsync, after the program's name.
pub fn desktop_args(desktop: &Desktop, mode: Mode) -> Vec<String> {
    let mut args = desktop.options.words(mode);
    args.push("--protect-args".to_owned());
    // The relay joins the server's command line with spaces, as ssh does: the server's shell reads the quotes.
    args.push(format!("--rsync-path={}", quote(&desktop.server_rsync)));
    args.push("-e".to_owned());
    args.push(
        desktop
            .relay
            .iter()
            .map(|word| rsh_word(word))
            .collect::<Vec<_>>()
            .join(" "),
    );
    let server = format!("{HOST}:{}", desktop.server_path);
    if desktop.upload {
        args.extend([desktop.desktop_path.clone(), server]);
    } else {
        args.extend([server, desktop.desktop_path.clone()]);
    }
    args
}

/// `word` as rsync splits its `-e` string: as it is when plain, else in single quotes, or double quotes when it
/// holds a single quote (rsync's `do_cmd`: in double quotes, a backslash escapes only `"` and `\`).
fn rsh_word(word: &str) -> String {
    let plain = !word.is_empty()
        && word
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_./:=+,@".contains(c));
    if plain {
        word.to_owned()
    } else if !word.contains('\'') {
        format!("'{word}'")
    } else {
        format!("\"{}\"", word.replace('\\', r"\\").replace('"', "\\\""))
    }
}

/// A Windows path as MSYS2's rsync takes it: `C:\Users\æøå` as `/c/Users/æøå`, `\\server\share` as
/// `//server/share` (a `C:` would otherwise be read as a host called C).
pub fn msys_path(path: &str) -> String {
    let forward = path.replace('\\', "/");
    let mut chars = forward.chars();
    match (chars.next(), chars.next(), chars.next()) {
        (Some(drive), Some(':'), Some('/') | None) if drive.is_ascii_alphabetic() => {
            format!(
                "/{}/{}",
                drive.to_ascii_lowercase(),
                forward.get(3..).unwrap_or_default()
            )
        }
        _ => forward,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn desktop() -> Desktop {
        Desktop {
            relay: vec![
                "/opt/Parley SSH/parley-ssh".into(),
                "--relay".into(),
                "40123".into(),
                "0123abcd".into(),
            ],
            server_rsync: "/QOpenSys/pkgs/bin/rsync".into(),
            desktop_path: "/home/niels/rapporter æøå/".into(),
            server_path: "/backup/it's here".into(),
            upload: true,
            options: Options::default(),
        }
    }

    #[test]
    fn the_desktops_rsync_reaches_the_server_through_the_relay_r_ssh_55() {
        assert_eq!(
            desktop_args(&desktop(), Mode::Copy),
            [
                "-az",
                "--partial",
                "--info=progress2",
                "--no-inc-recursive",
                "--protect-args",
                "--rsync-path='/QOpenSys/pkgs/bin/rsync'",
                "-e",
                "'/opt/Parley SSH/parley-ssh' --relay 40123 0123abcd",
                "/home/niels/rapporter æøå/",
                "parley:/backup/it's here",
            ]
        );
        let mut down = desktop();
        down.upload = false;
        down.options.excludes = vec!["*.log".into()];
        let args = desktop_args(&down, Mode::DryRun);
        assert!(
            args.contains(&"--exclude=*.log".to_owned()),
            "one argument, no shell: {args:?}"
        );
        assert_eq!(
            args[args.len() - 2..],
            ["parley:/backup/it's here", "/home/niels/rapporter æøå/"],
            "from the server to the desktop"
        );
    }

    #[test]
    fn the_relays_words_survive_rsyncs_splitting_r_ssh_55() {
        assert_eq!(rsh_word("--relay"), "--relay");
        assert_eq!(
            rsh_word(r"C:\Program Files\Parley\parley-ssh.exe"),
            r"'C:\Program Files\Parley\parley-ssh.exe'"
        );
        assert_eq!(
            rsh_word(r"/home/o'brien/parley-ssh"),
            r#""/home/o'brien/parley-ssh""#
        );
        assert_eq!(rsh_word(""), "''");
    }

    #[test]
    fn windows_paths_are_given_to_msys2s_rsync_in_its_form_r_ssh_55() {
        assert_eq!(msys_path(r"C:\Users\Niels\æøå"), "/c/Users/Niels/æøå");
        assert_eq!(msys_path("d:"), "/d/");
        assert_eq!(msys_path(r"\\nas\share\x"), "//nas/share/x");
        assert_eq!(msys_path("relative/dir"), "relative/dir");
        assert_eq!(msys_path("/already/posix"), "/already/posix");
    }
}
