//! What a program does around an [`SqlWindow`]: opens it with the settings, keeps what it
//! changed back in them, and says why it could not open. Shared by Parley's SQL add-on
//! (R-SQL-6) and Parley SQL (R-SQL-45), so both keep a session's history, scripts and pinned
//! libraries the same way.

use std::path::{Path, PathBuf};

use parley_core::SessionConfig;
use parley_core::sql::helper::HelperFiles;

use super::SqlWindow;
use crate::settings::Settings;
use crate::{dialog, theme};

/// How many statements the history keeps per session (R-SQL-7).
const HISTORY: usize = 100;

/// Said when this installation has no SQL helper (R-SQL-13).
const NO_HELPER: &str =
    "This Parley was installed without the SQL helper, so the SQL window cannot open.";

/// Where Parley reads Parley SQL's settings, which its SQL window uses (R-SQL-69): read each
/// time the window opens, so changes made in Parley SQL count. `None`: none saved yet.
pub struct SettingsReader(pub Box<dyn Fn() -> Option<Settings>>);

impl std::fmt::Debug for SettingsReader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SettingsReader")
    }
}

/// The window for `name` on `config`'s host, with the headings, scripts and pinned libraries
/// `kept`, and the Java and limits `chosen` (in Parley SQL the same settings; in Parley, Parley
/// SQL's, R-SQL-69); `user` is filled in. The helper's jars are in `helpers`; Java is the one
/// the settings name, or found. `Err` says why it cannot open.
pub fn open(
    name: &str,
    config: &SessionConfig,
    helpers: Option<&Path>,
    user: &str,
    kept: &Settings,
    chosen: &Settings,
) -> Result<SqlWindow, String> {
    let folder = helpers.ok_or_else(|| NO_HELPER.to_owned())?;
    let java_path = chosen.java_path.trim();
    let configured = (!java_path.is_empty()).then(|| PathBuf::from(java_path));
    let java = parley_core::sql::java::find(configured.as_deref()).map_err(|e| e.to_string())?;
    let scripts = kept.sql_scripts.get(name).cloned().unwrap_or_default();
    let pinned = kept.sql_pinned.get(name).cloned().unwrap_or_default();
    Ok(SqlWindow::open(
        name,
        config,
        java.path,
        HelperFiles::in_folder(folder),
        user,
        &scripts,
    )
    .with_limits(chosen.sql_rows, chosen.sql_timeout)
    .with_header(kept.sql_header)
    .with_pinned(pinned))
}

/// Keeps what `window` for session `name` changed in `settings`: the statements it ran
/// (R-SQL-7), the headings chosen (R-SQL-29), the pinned libraries (R-SQL-34) and the scripts
/// (R-SQL-20), these only when their count moved on from `scripts_kept`.
pub fn keep(window: &mut SqlWindow, name: &str, settings: &mut Settings, scripts_kept: &mut u64) {
    settings.sql_header = window.header;
    let pinned = window.pinned();
    if settings
        .sql_pinned
        .get(name)
        .map_or(!pinned.is_empty(), |p| p != pinned)
    {
        settings.sql_pinned.insert(name.to_owned(), pinned.to_vec());
    }
    if window.scripts_generation() != *scripts_kept {
        let (generation, scripts) = window.scripts();
        *scripts_kept = generation;
        settings.sql_scripts.insert(name.to_owned(), scripts);
    }
    let ran = window.take_ran();
    if !ran.is_empty() {
        add_to_history(
            settings.sql_history.entry(name.to_owned()).or_default(),
            ran,
        );
    }
}

/// Moves what the settings keep for session `old` (history, scripts, pinned libraries) to its
/// new name `new` (R-SQL-46).
pub fn rename(settings: &mut Settings, old: &str, new: &str) {
    if let Some(history) = settings.sql_history.remove(old) {
        settings.sql_history.insert(new.to_owned(), history);
    }
    if let Some(scripts) = settings.sql_scripts.remove(old) {
        settings.sql_scripts.insert(new.to_owned(), scripts);
    }
    if let Some(pinned) = settings.sql_pinned.remove(old) {
        settings.sql_pinned.insert(new.to_owned(), pinned);
    }
}

/// Adds `ran` to `history`: a statement run again moves to the end, and only the last
/// [`HISTORY`] are kept.
fn add_to_history(history: &mut Vec<String>, ran: Vec<String>) {
    for statement in ran {
        history.retain(|s| *s != statement);
        history.push(statement);
    }
    let excess = history.len().saturating_sub(HISTORY);
    history.drain(..excess);
}

/// Says why the SQL window could not open, with where to get Java, while `notice` is set;
/// OK or the close button clears it.
pub fn notice_dialog(ctx: &egui::Context, notice: &mut Option<String>) {
    let Some(text) = notice.clone() else {
        return;
    };
    let (close, ok) = dialog::dialog(
        ctx,
        "sql-notice",
        "The SQL window cannot open",
        440.0,
        |ui| {
            ui.label(&text);
            ui.add_space(6.0);
            ui.hyperlink_to(
                "A free Java: Eclipse Temurin (adoptium.net)",
                "https://adoptium.net",
            );
            theme::hint(ui, "Then name it in Settings, or set JAVA_HOME.");
            ui.add_space(12.0);
            theme::button_row(ui, |ui| ui.add(theme::primary_button(ui, "OK")).clicked())
        },
    );
    if close || ok {
        *notice = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_history_keeps_a_statement_once_and_only_the_last_hundred_r_sql_7() {
        let mut history = vec!["A".to_owned(), "B".to_owned()];
        add_to_history(&mut history, vec!["A".to_owned(), "C".to_owned()]);
        assert_eq!(history, ["B", "A", "C"], "run again: moved to the end");
        add_to_history(&mut history, (0..150).map(|n| format!("S{n}")).collect());
        assert_eq!(history.len(), 100);
        assert_eq!(history.last().map(String::as_str), Some("S149"));
    }

    #[test]
    fn a_renamed_connection_keeps_its_history_scripts_and_pins_r_sql_46() {
        let mut settings = Settings::default();
        settings
            .sql_history
            .insert("Prod".to_owned(), vec!["VALUES 1".to_owned()]);
        settings
            .sql_pinned
            .insert("Prod".to_owned(), vec!["QSYS2".to_owned()]);
        rename(&mut settings, "Prod", "Production");
        assert!(!settings.sql_history.contains_key("Prod"));
        assert_eq!(settings.sql_history["Production"], ["VALUES 1"]);
        assert_eq!(settings.sql_pinned["Production"], ["QSYS2"]);
    }

    #[test]
    fn without_the_helper_the_window_says_why_it_cannot_open_r_sql_45() {
        let config = SessionConfig::default();
        let opened = open(
            "Prod",
            &config,
            None,
            "",
            &Settings::default(),
            &Settings::default(),
        );
        assert_eq!(opened.err().as_deref(), Some(NO_HELPER));
    }
}
