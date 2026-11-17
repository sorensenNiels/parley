//! Session logs (R-SSH-25, D46): what the host sent, as plain text or raw, appended to a file named from a pattern:
//! `&H` the host, `&Y`, `&M`, `&D` the date, `&T` the time (hhmmss), `&&` an ampersand; `~` the home folder.

use std::io::Write as _;
use std::path::PathBuf;

use chrono::{DateTime, Local};
use parley_ssh_core::term::PlainText;
use serde::{Deserialize, Serialize};

/// What a session log keeps.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogMode {
    #[default]
    Off,
    /// The text as shown, without escape sequences.
    Text,
    /// The output exactly as it came.
    Raw,
}

/// Where logs go unless a session says otherwise.
pub const DEFAULT_PATTERN: &str = "~/parley-ssh-logs/&H-&Y&M&D-&T.log";

/// The file `pattern` names for `host` at `now`.
pub fn file_name(pattern: &str, host: &str, now: &DateTime<Local>) -> PathBuf {
    let mut name = String::new();
    let mut chars = pattern.trim().chars().peekable();
    while let Some(c) = chars.next() {
        if c != '&' {
            name.push(c);
            continue;
        }
        match chars.next() {
            Some('H') => name.push_str(&host.replace(['/', '\\', ':'], "_")),
            Some('Y') => name.push_str(&now.format("%Y").to_string()),
            Some('M') => name.push_str(&now.format("%m").to_string()),
            Some('D') => name.push_str(&now.format("%d").to_string()),
            Some('T') => name.push_str(&now.format("%H%M%S").to_string()),
            Some('&') | None => name.push('&'),
            Some(other) => {
                name.push('&');
                name.push(other);
            }
        }
    }
    match name.strip_prefix("~/").or_else(|| name.strip_prefix("~\\")) {
        Some(rest) => std::env::home_dir().unwrap_or_default().join(rest),
        None => PathBuf::from(name),
    }
}

/// A log being written.
#[derive(Debug)]
pub struct SessionLog {
    file: std::fs::File,
    pub path: PathBuf,
    /// For a plain-text log; `None` for a raw one.
    plain: Option<PlainText>,
}

impl SessionLog {
    /// Opens (or appends to) the log of `label` on `host`, writing a line that says when it started.
    ///
    /// # Errors
    /// The folder or file cannot be made or written, said for the user.
    pub fn open(pattern: &str, mode: LogMode, host: &str, label: &str) -> Result<Self, String> {
        let now = Local::now();
        let path = file_name(pattern, host, &now);
        let cannot = |e: std::io::Error| {
            format!("The session log {} cannot be written: {e}", path.display())
        };
        if let Some(folder) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(folder).map_err(cannot)?;
        }
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(cannot)?;
        let kind = if mode == LogMode::Raw { "raw" } else { "text" };
        let header = format!(
            "\n=== Parley SSH log ({kind}) of {label}, {} ===\n",
            now.format("%Y-%m-%d %H:%M:%S")
        );
        file.write_all(header.as_bytes()).map_err(cannot)?;
        Ok(Self {
            file,
            path,
            plain: (mode != LogMode::Raw).then(PlainText::default),
        })
    }

    /// Appends what the host sent.
    ///
    /// # Errors
    /// The file cannot be written, said for the user.
    pub fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
        if bytes.is_empty() {
            return Ok(());
        }
        let result = match &mut self.plain {
            Some(plain) => self.file.write_all(plain.push(bytes).as_bytes()),
            None => self.file.write_all(bytes),
        };
        result.map_err(|e| {
            format!(
                "The session log {} cannot be written: {e}",
                self.path.display()
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    #[test]
    fn a_log_is_named_from_its_pattern_r_ssh_25() {
        let now = Local
            .with_ymd_and_hms(2026, 11, 6, 9, 5, 7)
            .single()
            .expect("a time");
        assert_eq!(
            file_name("/logs/&H-&Y&M&D-&T && &x.log", "bøx:22/a", &now),
            PathBuf::from("/logs/bøx_22_a-20261106-090507 & &x.log")
        );
        let home = file_name("~/l/&H.log", "h", &now);
        assert!(
            home.ends_with("l/h.log") && home.is_absolute(),
            "{}",
            home.display()
        );
    }

    #[test]
    fn a_text_log_appends_without_escape_sequences_and_a_raw_one_as_it_came_r_ssh_25() {
        let folder = std::env::temp_dir().join(format!("parley-ssh-log-{}", std::process::id()));
        let pattern = format!("{}/new/&H.log", folder.display());
        let mut text = SessionLog::open(&pattern, LogMode::Text, "box", "me@box").expect("opened");
        text.write(b"\x1b[1mhej\x1b[0m \xc3\xa6\r\n")
            .expect("written");
        drop(text);
        let mut raw =
            SessionLog::open(&pattern, LogMode::Raw, "box", "me@box").expect("opened again");
        raw.write(b"\x1b[1mraw").expect("written");
        drop(raw);
        let kept = std::fs::read_to_string(folder.join("new/box.log")).expect("the log");
        assert!(
            kept.contains("=== Parley SSH log (text) of me@box"),
            "{kept}"
        );
        assert!(kept.contains("hej æ\n"), "{kept}");
        assert!(
            kept.contains("(raw)") && kept.ends_with("\x1b[1mraw"),
            "appended: {kept}"
        );
        std::fs::remove_dir_all(&folder).expect("cleaned");
    }
}
