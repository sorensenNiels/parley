//! Finding a Java to run the SQL helper with (R-SQL-12): the path the settings name, else
//! `JAVA_HOME`, else `java` on the `PATH`; version 11 or later.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::error::{Error, Result};

/// The oldest Java the helper runs on.
pub const MINIMUM: u32 = 11;

/// A Java that was found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Java {
    pub path: PathBuf,
    /// As `java -version` gives it: "11.0.33".
    pub version: String,
    pub major: u32,
}

/// The Java to run the helper with: `configured` (from the settings) when given, used alone so
/// a wrong path is reported rather than quietly passed over; otherwise the first of 11 or
/// later in `JAVA_HOME` and on the `PATH`. The error says where it looked and what it found.
pub fn find(configured: Option<&Path>) -> Result<Java> {
    let candidates: Vec<PathBuf> = match configured {
        Some(path) => vec![path.to_path_buf()],
        None => std::env::var_os("JAVA_HOME")
            .map(|home| java_in(Path::new(&home)))
            .into_iter()
            .chain(std::iter::once(PathBuf::from(executable())))
            .collect(),
    };
    let mut found = Vec::new();
    for path in &candidates {
        match probe(path) {
            Some(java) if java.major >= MINIMUM => return Ok(java),
            Some(java) => found.push(format!("{} is Java {}", path.display(), java.version)),
            None if configured.is_some() => {
                found.push(format!("{} does not run as Java", path.display()));
            }
            None => {}
        }
    }
    let detail = if found.is_empty() {
        "no Java was found in JAVA_HOME or on the PATH".to_owned()
    } else {
        found.join("; ")
    };
    Err(Error::Java(format!(
        "The SQL window needs Java {MINIMUM} or later: {detail}."
    )))
}

/// The `java` executable inside a Java home folder.
fn java_in(home: &Path) -> PathBuf {
    home.join("bin").join(executable())
}

const fn executable() -> &'static str {
    if cfg!(windows) { "java.exe" } else { "java" }
}

/// `command` started without a console window: on Windows a window program's `java.exe` would
/// otherwise open one of its own for as long as it runs (R-SQL-103). Win32 process creation
/// flags: `CREATE_NO_WINDOW`; the pipes to the helper work as before.
pub(crate) fn without_window(command: &mut Command) -> &mut Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}

/// Runs `path -version`; `None` when it does not run or does not say a version.
fn probe(path: &Path) -> Option<Java> {
    let output = without_window(&mut Command::new(path))
        .arg("-version")
        .stdin(Stdio::null())
        .output()
        .ok()?;
    // Java prints its version on stderr.
    let text = String::from_utf8_lossy(&output.stderr);
    let (version, major) = parse_version(&text)?;
    Some(Java {
        path: path.to_path_buf(),
        version,
        major,
    })
}

/// The version and major version in `java -version`'s version line: `openjdk version "11.0.33"
/// …` gives 11, `java version "1.8.0_411"` gives 8. Lines before it, such as `Picked up
/// JAVA_TOOL_OPTIONS: …`, are passed over.
fn parse_version(text: &str) -> Option<(String, u32)> {
    let first = text.lines().find(|line| line.contains("version \""))?;
    let start = first.find('"')? + 1;
    let version = first.get(start..)?.split('"').next()?.to_owned();
    let mut parts = version.split(['.', '_', '-', '+']);
    let first_number: u32 = parts.next()?.parse().ok()?;
    let major = if first_number == 1 {
        parts.next()?.parse().ok()?
    } else {
        first_number
    };
    Some((version, major))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_read_from_java_version_r_sql_12() {
        for (text, version, major) in [
            (
                "openjdk version \"11.0.33\" 2026-10-20\nOpenJDK Runtime",
                "11.0.33",
                11,
            ),
            ("java version \"1.8.0_411\"\nJava(TM) SE", "1.8.0_411", 8),
            ("openjdk version \"21\" 2023-09-19", "21", 21),
            ("openjdk version \"25-ea\" 2025-09-16", "25-ea", 25),
        ] {
            assert_eq!(
                parse_version(text),
                Some((version.to_owned(), major)),
                "{text}"
            );
        }
        for text in ["", "no quotes here", "openjdk version \"x.y\""] {
            assert_eq!(parse_version(text), None, "{text}");
        }
    }

    #[test]
    fn lines_before_the_version_such_as_java_tool_options_are_passed_over_r_sql_12() {
        let text = "Picked up JAVA_TOOL_OPTIONS: -Dfile.encoding=\"UTF-8\"\nPicked up _JAVA_OPTIONS: -Xmx1g\nopenjdk version \"17.0.12\" 2024-07-16\nOpenJDK Runtime";
        assert_eq!(parse_version(text), Some(("17.0.12".to_owned(), 17)));
    }

    #[test]
    fn a_configured_java_that_is_not_there_is_named_not_passed_over_r_sql_12() {
        let missing = Path::new("/no/such/java/bin/java");
        assert!(probe(missing).is_none());
        let error = find(Some(missing)).expect_err("no fallback for a configured path");
        let text = error.to_string();
        assert!(
            text.contains("/no/such/java/bin/java does not run as Java"),
            "{text}"
        );
        assert!(text.contains("needs Java 11 or later"), "{text}");
    }
}
