//! Transfers with rsync (Phase 31, D48, D58), as text in and text out: what Parley runs on a server and what
//! it makes of rsync's answers. Nothing here does I/O; `net` runs the commands on side channels.

use std::fmt;

mod changes;
mod run;
pub use changes::{Changes, Listed};
pub use run::{
    Direct, Mode, Options, Progress, Remote, bytes_text, direct_command, last_progress, outcome,
    quote, unreachable,
};

/// The oldest rsync with `--info=progress2`, which the progress needs (R-SSH-57).
pub const OLDEST: Version = Version {
    major: 3,
    minor: 1,
    patch: 0,
};

/// What [`check_command`]'s script prints before the path of the rsync it found.
const FOUND: &str = "parley-rsync:";

/// Where rsync is looked for, in order: the `PATH`, then where IBM i's open-source packages put it (often not on
/// the `PATH` of a command run over SSH) and where self-built ones go.
const PLACES: [&str; 3] = ["rsync", "/QOpenSys/pkgs/bin/rsync", "/usr/local/bin/rsync"];

/// The script that finds a server's rsync and says its version (R-SSH-57): plain Bourne shell, as IBM i's
/// `bsh` and every Unix shell run it. Exit status 127 when there is none.
pub fn check_command() -> String {
    format!(
        "for r in {}; do if \"$r\" --version 2>/dev/null; then echo \"{FOUND} $r\"; exit 0; fi; done; exit 127",
        PLACES.join(" ")
    )
}

/// An rsync version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// What a server's rsync turned out to be (R-SSH-57).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Check {
    /// 3.1 or later, at this path: transfers can go ahead.
    Usable { version: Version, path: String },
    /// Older than 3.1.
    TooOld { version: Version, path: String },
    /// Apple's and OpenBSD's rsync, which lacks `--info=progress2`.
    Openrsync { path: String },
    /// No rsync in any place looked at.
    Missing,
    /// Something answered that Parley cannot read: its first line.
    Unknown(String),
}

/// Reads the answer to [`check_command`]: its exit status and what it printed.
pub fn check(exit_status: Option<u32>, output: &str) -> Check {
    let path = output
        .lines()
        .rev()
        .find_map(|line| line.trim().strip_prefix(FOUND))
        .map(|p| p.trim().to_owned());
    let Some(path) = path else {
        return if exit_status == Some(127) {
            Check::Missing
        } else {
            Check::Unknown(first_line(output))
        };
    };
    if output.contains("openrsync") {
        return Check::Openrsync { path };
    }
    match output.lines().find_map(version_line) {
        Some(version) if version >= OLDEST => Check::Usable { version, path },
        Some(version) => Check::TooOld { version, path },
        None => Check::Unknown(first_line(output)),
    }
}

/// The version in rsync's first line: `rsync  version 3.2.7  protocol version 31`, also `version v3.4.1`.
fn version_line(line: &str) -> Option<Version> {
    let mut words = line.split_whitespace();
    if words.next()? != "rsync" || words.next()? != "version" {
        return None;
    }
    let number = words.next()?;
    let number = number.strip_prefix('v').unwrap_or(number);
    let mut parts = number
        .split('.')
        .map(|part| part.trim_end_matches(|c: char| !c.is_ascii_digit()));
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    Some(Version {
        major,
        minor,
        patch,
    })
}

fn first_line(output: &str) -> String {
    output
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("nothing")
        .to_owned()
}

impl Check {
    /// What is wrong at `end` (`user@host`), in plain words with how to put it right; `None` when rsync can be
    /// used (R-SSH-57).
    pub fn problem(&self, end: &str) -> Option<String> {
        const HOW: &str = "Install rsync 3.1 or later there: `sudo apt install rsync` or `sudo dnf install rsync`; \
                           on IBM i, `yum install rsync` from IBM's open-source packages.";
        match self {
            Self::Usable { .. } => None,
            Self::TooOld { version, path } => Some(format!(
                "{end} has rsync {version} ({path}), which is too old: Parley needs 3.1 or later. {HOW}"
            )),
            Self::Openrsync { path } => Some(format!(
                "{end} has openrsync ({path}), which cannot report progress: Parley needs rsync 3.1 or later. {HOW}"
            )),
            Self::Missing => Some(format!("{end} has no rsync. {HOW}")),
            Self::Unknown(line) => Some(format!(
                "Parley could not find out which rsync {end} has; it answered: {line}"
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found(version_text: &str, path: &str) -> String {
        format!("{version_text}\n{FOUND} {path}\n")
    }

    #[test]
    fn gnu_rsync_from_3_1_on_is_usable_r_ssh_57() {
        for (text, version) in [
            ("rsync  version 3.1.0  protocol version 31", (3, 1, 0)),
            (
                "rsync  version 3.2.7  protocol version 31\nCopyright (C) 1996-2022 by Andrew Tridgell",
                (3, 2, 7),
            ),
            ("rsync  version v3.4.1  protocol version 32", (3, 4, 1)),
            ("rsync  version 3.2.3dev  protocol version 31", (3, 2, 3)),
        ] {
            let (major, minor, patch) = version;
            assert_eq!(
                check(Some(0), &found(text, "/usr/bin/rsync")),
                Check::Usable {
                    version: Version {
                        major,
                        minor,
                        patch
                    },
                    path: "/usr/bin/rsync".into(),
                },
                "{text}"
            );
        }
        assert_eq!(
            check(
                Some(0),
                &found(
                    "rsync  version 3.1.2  protocol version 31",
                    "/QOpenSys/pkgs/bin/rsync"
                )
            )
            .problem("niels@ibmi"),
            None
        );
    }

    #[test]
    fn old_rsync_openrsync_and_none_are_said_in_plain_words_r_ssh_57() {
        let old = check(
            Some(0),
            &found("rsync  version 2.6.9  protocol version 29", "rsync"),
        );
        assert!(matches!(old, Check::TooOld { version, .. } if version.to_string() == "2.6.9"));
        let said = old.problem("æøå@host").expect("a problem");
        assert!(
            said.starts_with("æøå@host has rsync 2.6.9 (rsync), which is too old"),
            "{said}"
        );

        let apple = check(
            Some(0),
            &found(
                "openrsync: protocol version 29\nrsync version 2.6.9 compatible",
                "rsync",
            ),
        );
        assert_eq!(
            apple,
            Check::Openrsync {
                path: "rsync".into()
            }
        );
        assert!(
            apple
                .problem("mac")
                .expect("a problem")
                .contains("openrsync")
        );

        assert_eq!(check(Some(127), ""), Check::Missing);
        assert!(
            Check::Missing
                .problem("box")
                .expect("a problem")
                .contains("yum install rsync")
        );
    }

    #[test]
    fn an_answer_that_cannot_be_read_says_what_came_back_r_ssh_57() {
        let shell = check(
            Some(1),
            "\n'for' is not recognized as an internal or external command\n",
        );
        assert_eq!(
            shell,
            Check::Unknown("'for' is not recognized as an internal or external command".into())
        );
        assert_eq!(check(None, ""), Check::Unknown("nothing".into()));
        assert!(matches!(
            check(Some(0), &found("rsync version", "rsync")),
            Check::Unknown(_)
        ));
        assert!(matches!(
            check(Some(0), &found("rsync  version x.y  protocol", "rsync")),
            Check::Unknown(_)
        ));
    }

    #[test]
    fn the_check_looks_on_the_path_then_where_ibm_i_keeps_rsync_r_ssh_57() {
        let script = check_command();
        assert!(
            script.starts_with("for r in rsync /QOpenSys/pkgs/bin/rsync /usr/local/bin/rsync;")
        );
        assert!(script.ends_with("exit 127"));
    }
}
