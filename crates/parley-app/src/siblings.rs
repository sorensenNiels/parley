//! The other programs installed beside `parley` (R-NF-2, R-SSH-1): Parley SQL and Parley SSH. `parley --sql …` and
//! `parley --ssh …`, or an app image started through a link named `parley-sql` or `parley-ssh`, hand over to them,
//! because an app image has one entry point.

use std::path::PathBuf;

/// A program of the release besides Parley 5250.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sibling {
    Sql,
    Ssh,
}

impl Sibling {
    const ALL: [Self; 2] = [Self::Sql, Self::Ssh];

    /// Its command: the binary's name.
    pub const fn command(self) -> &'static str {
        match self {
            Self::Sql => "parley-sql",
            Self::Ssh => "parley-ssh",
        }
    }

    /// The option of `parley` that starts it.
    const fn option(self) -> &'static str {
        match self {
            Self::Sql => "--sql",
            Self::Ssh => "--ssh",
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Sql => "Parley SQL",
            Self::Ssh => "Parley SSH",
        }
    }

    /// Its program, installed beside this one.
    ///
    /// # Errors
    /// When it is not there, said for the user.
    pub fn beside(self) -> Result<PathBuf, String> {
        let program = std::env::current_exe()
            .map_err(|e| format!("Parley 5250 could not find its own folder: {e}"))?
            .with_file_name(format!(
                "{}{}",
                self.command(),
                std::env::consts::EXE_SUFFIX
            ));
        if program.is_file() {
            Ok(program)
        } else {
            Err(format!(
                "{} is not installed beside Parley (looked for {}).",
                self.name(),
                program.display()
            ))
        }
    }

    /// Runs it with `args` and waits for it; its exit code.
    ///
    /// # Errors
    /// When it is not beside Parley or cannot be started, said for the user.
    pub fn hand_over(self, args: &[String]) -> Result<i32, String> {
        let status = std::process::Command::new(self.beside()?)
            .args(args)
            .status()
            .map_err(|e| format!("{} could not be started: {e}", self.name()))?;
        Ok(status.code().unwrap_or(1))
    }
}

/// Which program `parley` was asked to be, with its arguments: `args` start with `--sql` or `--ssh`, or it was
/// started by the name `parley-sql` or `parley-ssh`, which an app image gives in `argv0` (`$ARGV0`) when started
/// through a link of that name. `None`: run Parley 5250.
pub fn handed_over(args: &[String], argv0: Option<&str>) -> Option<(Sibling, Vec<String>)> {
    if let Some(first) = args.first()
        && let Some(sibling) = Sibling::ALL.into_iter().find(|s| s.option() == first)
    {
        return Some((sibling, args.get(1..).unwrap_or_default().to_vec()));
    }
    let name = argv0
        .map(std::path::Path::new)
        .and_then(std::path::Path::file_stem)
        .and_then(std::ffi::OsStr::to_str)?;
    Sibling::ALL
        .into_iter()
        .find(|s| s.command() == name)
        .map(|s| (s, args.to_vec()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(text: &str) -> Vec<String> {
        text.split_whitespace().map(str::to_owned).collect()
    }

    /// R-NF-2: `parley --sql …` and a link named `parley-sql` to the app image start Parley SQL with the rest of
    /// the arguments; anything else is Parley.
    #[test]
    fn parley_hands_over_to_parley_sql_when_asked_r_nf_2() {
        let sql = |a: &str| Some((Sibling::Sql, args(a)));
        assert_eq!(handed_over(&args("--sql Prod"), None), sql("Prod"));
        assert_eq!(handed_over(&args("--sql"), None), sql(""));
        assert_eq!(
            handed_over(
                &args("--host ibmi"),
                Some("/home/æøå/.local/bin/parley-sql")
            ),
            sql("--host ibmi")
        );
        assert_eq!(
            handed_over(&args("ibmi"), Some("/opt/Parley.AppImage")),
            None
        );
        assert_eq!(handed_over(&args("ibmi --sql"), None), None, "only first");
        assert_eq!(handed_over(&[], Some("parley")), None);
    }

    /// R-SSH-1: the same for Parley SSH, so the app image starts it too.
    #[test]
    fn parley_hands_over_to_parley_ssh_when_asked_r_ssh_1() {
        let ssh = |a: &str| Some((Sibling::Ssh, args(a)));
        assert_eq!(
            handed_over(&args("--ssh --version"), None),
            ssh("--version")
        );
        assert_eq!(
            handed_over(&[], Some("/home/me/.local/bin/parley-ssh")),
            ssh("")
        );
        assert_eq!(handed_over(&args("ibmi --ssh"), None), None, "only first");
    }

    #[test]
    fn a_sibling_not_installed_is_said_by_name() {
        // The test binary has no parley-ssh beside it.
        let error = Sibling::Ssh.beside().expect_err("not beside the test");
        assert!(
            error.starts_with("Parley SSH is not installed beside Parley"),
            "{error}"
        );
    }
}
