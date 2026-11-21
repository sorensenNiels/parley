//! A transfer's command line and what rsync says while and after it runs (R-SSH-53, R-SSH-56, D58).

/// The name B's host key goes by in the temporary `known_hosts` file on A (`HostKeyAlias`), so the line does not
/// depend on how A would write B's name and port.
const ALIAS: &str = "parley-target";

/// Where B is, as B's open session reaches it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Remote {
    pub user: String,
    pub host: String,
    pub port: u16,
    /// The host key Parley verified for B, as an OpenSSH line (`ssh-ed25519 AAAA…`).
    pub host_key: String,
    /// B's rsync, as its check found it.
    pub rsync: String,
}

/// A copy from server A straight to server B (R-SSH-53).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Direct {
    /// A's rsync, as its check found it.
    pub rsync: String,
    /// The path on A; a trailing `/` copies a folder's contents rather than the folder.
    pub source: String,
    pub target: Remote,
    /// The path on B.
    pub target_path: String,
    pub options: Options,
}

/// How rsync copies (R-SSH-56's switches).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    /// Delete files at the target that the source does not have (`--delete`); excluded ones are kept.
    pub delete: bool,
    /// Keep permissions, owner, group and times (`-a`); without it, files and links only (`-rl`).
    pub keep: bool,
    /// Compress on the way (`-z`).
    pub compress: bool,
    /// Patterns not copied (`--exclude`), one each.
    pub excludes: Vec<String>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            delete: false,
            keep: true,
            compress: true,
            excludes: Vec::new(),
        }
    }
}

/// Whether rsync copies or only says what it would do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// `--dry-run --itemize-changes`, names unescaped (`-8`): what will be added, changed and deleted (R-SSH-56).
    DryRun,
    /// With progress, keeping partial files for the next try (`--partial`).
    Copy,
}

impl Options {
    /// rsync's switches for these options in `mode`, each one word for the shell.
    fn switches(&self, mode: Mode) -> String {
        let mut words = vec![format!(
            "-{}{}",
            if self.keep { "a" } else { "rl" },
            if self.compress { "z" } else { "" }
        )];
        words.push(
            match mode {
                Mode::DryRun => "--dry-run --itemize-changes -8",
                Mode::Copy => "--partial --info=progress2 --no-inc-recursive",
            }
            .to_owned(),
        );
        if self.delete {
            words.push("--delete".to_owned());
        }
        for pattern in self
            .excludes
            .iter()
            .map(|p| p.trim())
            .filter(|p| !p.is_empty())
        {
            words.push(format!("--exclude={}", quote(pattern)));
        }
        words.join(" ")
    }
}

/// `text` as one word for a Bourne shell: in single quotes, a quote inside as `'\''`.
pub fn quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', r"'\''"))
}

/// What runs on A for `direct` (R-SSH-53, D58): B's host key into a file of A's only the user can read (made with
/// `set -C`, so an existing file or link is never written through) and removed when the script ends however it
/// ends; then A's rsync, signing on to B with the agent (`BatchMode`: no password prompt to hang on) and trusting
/// only that key. Plain Bourne shell, as [`super::check_command`].
pub fn direct_command(direct: &Direct, mode: Mode) -> Result<String, String> {
    let key = direct.target.host_key.trim();
    if key.split_whitespace().count() != 2 {
        return Err("Parley has no host key for the target server: connect to it again".to_owned());
    }
    let line = quote(&format!("{ALIAS} {key}"));
    let ssh = format!(
        "ssh -p {} -o UserKnownHostsFile=$f -o StrictHostKeyChecking=yes -o HostKeyAlias={ALIAS} -o BatchMode=yes",
        direct.target.port
    );
    let host = if direct.target.host.contains(':') {
        format!("[{}]", direct.target.host)
    } else {
        direct.target.host.clone()
    };
    let to = format!("{}@{host}:{}", direct.target.user, direct.target_path);
    Ok(format!(
        "f=/tmp/parley-known-hosts.$$; (umask 077; set -C; printf '%s\\n' {line} > \"$f\") || exit 1; \
         trap 'rm -f \"$f\"' 0 1 2 15; \
         {} {} --protect-args --rsync-path={} -e \"{ssh}\" {} {}",
        quote(&direct.rsync),
        direct.options.switches(mode),
        quote(&direct.target.rsync),
        quote(&direct.source),
        quote(&to),
    ))
}

/// How far a transfer is, from a line of `--info=progress2`:
/// `    1,238,099  42%  117.13MB/s    0:00:05 (xfr#5, to-chk=0/12)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Progress {
    pub bytes: u64,
    pub percent: u8,
    /// As rsync writes it (`117.13MB/s`).
    pub speed: String,
    /// Time left while copying, the time it took on the last line (`0:00:05`).
    pub time: String,
}

/// The last progress line in `output`, which rsync separates with carriage returns; a line cut short is passed
/// over.
pub fn last_progress(output: &str) -> Option<Progress> {
    output.split(['\r', '\n']).rev().find_map(progress_line)
}

fn progress_line(line: &str) -> Option<Progress> {
    let mut words = line.split_whitespace();
    // Thousands separators depend on the server's locale: `,`, `.` or `'`.
    let bytes = words
        .next()?
        .chars()
        .filter(|c| !matches!(c, ',' | '.' | '\''))
        .collect::<String>()
        .parse()
        .ok()?;
    let percent = words.next()?.strip_suffix('%')?.parse().ok()?;
    let speed = words.next()?.to_owned();
    let time = words.next()?;
    if !time.contains(':') || !speed.ends_with("/s") {
        return None;
    }
    Some(Progress {
        bytes,
        percent,
        speed,
        time: time.to_owned(),
    })
}

/// Whether ssh said it could not sign on: `Permission denied (publickey,password).`, with the methods tried;
/// a file's `Permission denied (13)` is rsync's, not a refusal.
fn sign_on_refused(errors: &str) -> bool {
    const SAID: &str = "Permission denied (";
    errors.match_indices(SAID).any(|(at, _)| {
        errors
            .get(at + SAID.len()..)
            .and_then(|rest| rest.chars().next())
            .is_some_and(char::is_alphabetic)
    })
}

/// Bytes for people: `980 B`, `1.2 MB`, `3.4 GB` (powers of 1000, as rsync's speed).
pub fn bytes_text(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["kB", "MB", "GB", "TB", "PB"];
    if bytes < 1000 {
        return format!("{bytes} B");
    }
    #[expect(
        clippy::cast_precision_loss,
        reason = "shown with one decimal: the loss is far below it"
    )]
    let mut value = bytes as f64 / 1000.0;
    let mut unit = 0;
    while value >= 1000.0 && unit < UNITS.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    format!("{value:.1} {}", UNITS[unit])
}

/// What a transfer that ended with `exit_status` and wrote `errors` comes to, in plain words (`rsync(1)`'s
/// EXIT VALUES, and what ssh on A says when it cannot reach or sign on to B).
pub fn outcome(exit_status: u32, errors: &str) -> Result<(), String> {
    if exit_status == 0 {
        return Ok(());
    }
    let last = errors
        .lines()
        .map(str::trim)
        .rfind(|line| !line.is_empty())
        .unwrap_or_default();
    let reason = if errors.contains("Host key verification failed") {
        "The target server's host key is not the one Parley verified: connect to it again to check it".to_owned()
    } else if sign_on_refused(errors) {
        "The target server refused the sign-on: add a key of your agent's to its authorized_keys"
            .to_owned()
    } else if [
        "Could not resolve hostname",
        "Connection refused",
        "timed out",
        "No route to host",
    ]
    .iter()
    .any(|s| errors.contains(s))
    {
        "The source server cannot reach the target server".to_owned()
    } else {
        match exit_status {
            1 => "rsync did not understand its command line",
            2 => "The two servers' rsync versions cannot work together",
            3 => "rsync could not find or read what to copy",
            5 | 12 | 255 => "The connection between the two servers failed",
            10 | 30 | 35 => "The connection between the two servers broke or timed out",
            11 => "A file could not be read or written",
            20 => "rsync was stopped",
            23 => "Some files could not be copied",
            24 => "Some files went away while being copied",
            _ => "rsync failed",
        }
        .to_owned()
    };
    Err(if last.is_empty() {
        format!("{reason} (exit status {exit_status}).")
    } else {
        format!("{reason} (exit status {exit_status}): {last}")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn direct() -> Direct {
        Direct {
            rsync: "/QOpenSys/pkgs/bin/rsync".into(),
            source: "/home/niels/rapporter æøå/".into(),
            target: Remote {
                user: "niels".into(),
                host: "backup.example.com".into(),
                port: 2222,
                host_key: "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIAV5".into(),
                rsync: "rsync".into(),
            },
            target_path: "/srv/it's here".into(),
            options: Options::default(),
        }
    }

    #[test]
    fn the_direct_command_trusts_only_bs_key_and_signs_on_with_the_agent_r_ssh_53() {
        let command = direct_command(&direct(), Mode::Copy).expect("a command");
        assert_eq!(
            command,
            "f=/tmp/parley-known-hosts.$$; (umask 077; set -C; printf '%s\\n' \
             'parley-target ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIAV5' > \"$f\") || exit 1; \
             trap 'rm -f \"$f\"' 0 1 2 15; \
             '/QOpenSys/pkgs/bin/rsync' -az --partial --info=progress2 --no-inc-recursive --protect-args \
             --rsync-path='rsync' -e \"ssh -p 2222 -o UserKnownHostsFile=$f -o StrictHostKeyChecking=yes \
             -o HostKeyAlias=parley-target -o BatchMode=yes\" '/home/niels/rapporter æøå/' \
             'niels@backup.example.com:/srv/it'\\''s here'"
        );
    }

    #[test]
    fn an_ipv6_target_is_bracketed_and_a_missing_host_key_is_refused_r_ssh_53() {
        let mut d = direct();
        d.target.host = "fe80::1".into();
        assert!(
            direct_command(&d, Mode::Copy)
                .expect("a command")
                .ends_with("'niels@[fe80::1]:/srv/it'\\''s here'")
        );
        d.target.host_key = String::new();
        assert!(direct_command(&d, Mode::Copy).is_err());
        d.target.host_key = "ssh-ed25519 AAAA'; rm -rf ~; '".into();
        assert!(
            direct_command(&d, Mode::Copy).is_err(),
            "a key line with more than two words is not a key"
        );
    }

    #[test]
    fn the_dry_run_and_the_switches_change_only_rsyncs_options_r_ssh_56() {
        let mut d = direct();
        let dry = direct_command(&d, Mode::DryRun).expect("a command");
        assert!(
            dry.contains(
                "'/QOpenSys/pkgs/bin/rsync' -az --dry-run --itemize-changes -8 --protect-args"
            ),
            "{dry}"
        );
        assert!(!dry.contains("--partial"), "a dry run copies nothing");
        d.options = Options {
            delete: true,
            keep: false,
            compress: false,
            excludes: vec!["*.log".into(), "  ".into(), " it's/ ".into()],
        };
        let copy = direct_command(&d, Mode::Copy).expect("a command");
        assert!(
            copy.contains(
                "rsync' -rl --partial --info=progress2 --no-inc-recursive --delete \
                 --exclude='*.log' --exclude='it'\\''s/' --protect-args"
            ),
            "{copy}"
        );
    }

    #[test]
    fn quoting_keeps_any_text_one_word() {
        assert_eq!(quote(""), "''");
        assert_eq!(quote("a b"), "'a b'");
        assert_eq!(quote("it's"), r"'it'\''s'");
        assert_eq!(quote("$HOME `x` æøå"), "'$HOME `x` æøå'");
    }

    #[test]
    fn progress_is_read_from_the_last_whole_line_r_ssh_56() {
        let output = "      32,768   0%    0.00kB/s    0:00:00\r  1,238,099  42%  117.13MB/s    0:00:05 (xfr#5, to-chk=7/12)\r  1,3";
        assert_eq!(
            last_progress(output),
            Some(Progress {
                bytes: 1_238_099,
                percent: 42,
                speed: "117.13MB/s".into(),
                time: "0:00:05".into()
            })
        );
        assert_eq!(
            last_progress("  2.048.000 100%    1,95MB/s    0:00:01 (xfr#1, to-chk=0/1)\n")
                .map(|p| (p.bytes, p.percent)),
            Some((2_048_000, 100)),
            "a German server's separators"
        );
        assert_eq!(last_progress(""), None);
        assert_eq!(
            last_progress("sending incremental file list\nrapport.pdf\n"),
            None
        );
    }

    #[test]
    fn bytes_are_said_for_people() {
        assert_eq!(bytes_text(0), "0 B");
        assert_eq!(bytes_text(999), "999 B");
        assert_eq!(bytes_text(1_238_099), "1.2 MB");
        assert_eq!(bytes_text(3_400_000_000), "3.4 GB");
        assert_eq!(bytes_text(u64::MAX), "18446.7 PB");
    }

    #[test]
    fn a_failed_transfer_is_said_in_plain_words_r_ssh_56() {
        assert_eq!(outcome(0, "anything"), Ok(()));
        let key = outcome(
            255,
            "Host key verification failed.\r\nrsync: connection unexpectedly closed\n",
        );
        assert!(
            key.expect_err("failed")
                .starts_with("The target server's host key is not the one")
        );
        let refused =
            outcome(255, "niels@backup: Permission denied (publickey).\n").expect_err("failed");
        assert!(refused.contains("authorized_keys"), "{refused}");
        let unreachable = outcome(
            255,
            "ssh: connect to host backup port 22: Connection refused\n",
        )
        .expect_err("failed");
        assert!(
            unreachable.starts_with("The source server cannot reach"),
            "{unreachable}"
        );
        assert_eq!(
            outcome(23, "rsync: [sender] send_files failed to open \"/a/æøå\": Permission denied (13)\n"),
            Err("Some files could not be copied (exit status 23): rsync: [sender] send_files failed to open \"/a/æøå\": Permission denied (13)".into()),
            "a file's Permission denied is not a refused sign-on"
        );
        assert_eq!(
            outcome(24, ""),
            Err("Some files went away while being copied (exit status 24).".into())
        );
        assert_eq!(
            outcome(99, "x\n\n"),
            Err("rsync failed (exit status 99): x".into())
        );
    }
}
