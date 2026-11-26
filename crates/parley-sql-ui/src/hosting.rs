//! What a program does around an [`SqlWindow`]: opens it with the settings, keeps what it
//! changed back in them, and says why it could not open. Parley SQL's (R-SQL-45); Parley
//! shows only its notice, when Parley SQL cannot be started (R-SQL-70).

use std::path::{Path, PathBuf};

use parley_core::SessionConfig;
use parley_sql_core::helper::HelperFiles;

use parley_sql_core::history;

use super::history::HistoryAction;
use super::{SqlWindow, browser};
use crate::settings::Settings;
use parley_ui_egui::{dialog, theme};

/// Said when this installation has no SQL helper (R-SQL-13).
const NO_HELPER: &str =
    "This Parley was installed without the SQL helper, so the SQL window cannot open.";

/// The window for `name` on `config`'s host, with `settings`' limits, headings, scripts, side
/// panel and library groups; `user` is filled in. The helper's jars are in `helpers`; Java is the one
/// the settings name, or found. `Err` says why it cannot open.
pub fn open(
    name: &str,
    config: &SessionConfig,
    helpers: Option<&Path>,
    user: &str,
    settings: &Settings,
) -> Result<SqlWindow, String> {
    let folder = helpers.ok_or_else(|| NO_HELPER.to_owned())?;
    let configured =
        (!settings.java_path.trim().is_empty()).then(|| PathBuf::from(settings.java_path.trim()));
    let java = parley_sql_core::java::find(configured.as_deref()).map_err(|e| e.to_string())?;
    let scripts = settings.sql_scripts.get(name).cloned().unwrap_or_default();
    // R-SQL-83: a connection without groups yet gets its pinned libraries as one.
    let groups = settings.sql_groups.get(name).cloned().unwrap_or_else(|| {
        browser::from_pinned(settings.sql_pinned.get(name).cloned().unwrap_or_default())
    });
    Ok(SqlWindow::open(
        config,
        java.path,
        HelperFiles::in_folder(folder),
        user,
        &scripts,
    )
    .with_limits(settings.sql_rows, settings.sql_timeout)
    .with_header(settings.sql_header)
    .with_side(settings.sql_side, groups)
    .with_snippet_values(
        settings
            .sql_snippet_values
            .get(name)
            .filter(|_| settings.sql_remember_values)
            .cloned()
            .unwrap_or_default(),
    ))
}

/// Keeps what `window` for session `name` changed in `settings`: the statements it ran
/// (R-SQL-7), the headings chosen (R-SQL-29), the side panel's layout and the library groups
/// (R-SQL-82, R-SQL-83), the placeholder values used (R-SQL-113) and the scripts (R-SQL-20),
/// these only when their count moved on from
/// `scripts_kept`.
pub fn keep(window: &mut SqlWindow, name: &str, settings: &mut Settings, scripts_kept: &mut u64) {
    settings.sql_header = window.header;
    let side = window.side_layout();
    if settings.sql_side != side {
        settings.sql_side = side;
    }
    let groups = window.library_groups();
    if settings.sql_groups.get(name).map(Vec::as_slice) != Some(groups) {
        settings.sql_groups.insert(name.to_owned(), groups.to_vec());
    }
    // R-SQL-122: kept only while the settings say so; turned off, those kept go.
    let values = window.snippet_values();
    if !settings.sql_remember_values {
        settings.sql_snippet_values.clear();
    } else if settings.sql_snippet_values.get(name) != Some(values) && !values.is_empty() {
        settings
            .sql_snippet_values
            .insert(name.to_owned(), values.clone());
    }
    // The pinned libraries are in the groups now (R-SQL-83).
    settings.sql_pinned.remove(name);
    if window.scripts_generation() != *scripts_kept {
        let (generation, scripts) = window.scripts();
        *scripts_kept = generation;
        settings.sql_scripts.insert(name.to_owned(), scripts);
    }
    // R-SQL-121: runs are kept while the settings say so, as many as they say.
    let ran = window.take_ran();
    if !ran.is_empty() && settings.sql_history_keep {
        history::add(
            settings.sql_history.entry(name.to_owned()).or_default(),
            ran,
            settings.sql_history_size,
        );
    }
    // R-SQL-90, R-SQL-91: Remove from history and Clear history.
    for edit in window.take_history_edits() {
        let Some(runs) = settings.sql_history.get_mut(name) else {
            continue;
        };
        match edit {
            HistoryAction::Remove(entry) => {
                if let Some(at) = runs.iter().position(|e| *e == entry) {
                    runs.remove(at);
                }
            }
            HistoryAction::Clear => runs.clear(),
            HistoryAction::Append(_)
            | HistoryAction::Insert(_)
            | HistoryAction::NewTab(_)
            | HistoryAction::Run(_)
            | HistoryAction::SaveSnippet(_) => {}
        }
    }
}

/// Moves what the settings keep for session `old` (history, scripts, library groups, placeholder
/// values) to its
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
    if let Some(groups) = settings.sql_groups.remove(old) {
        settings.sql_groups.insert(new.to_owned(), groups);
    }
    if let Some(values) = settings.sql_snippet_values.remove(old) {
        settings.sql_snippet_values.insert(new.to_owned(), values);
    }
}

/// Says why the SQL window could not open, with where to get Java, while `notice` is set;
/// OK or the close button clears it.
pub fn notice_dialog(ctx: &egui::Context, notice: &mut Option<String>) {
    dialog::notice(
        ctx,
        "sql-notice",
        "The SQL window cannot open",
        notice,
        |ui| {
            ui.add_space(6.0);
            ui.hyperlink_to(
                "A free Java: Eclipse Temurin (adoptium.net)",
                "https://adoptium.net",
            );
            theme::hint(ui, "Then name it in Settings, or set JAVA_HOME.");
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_notice_keeps_its_button_in_place_and_ok_clears_it_r_sql_12() {
        use egui_kittest::kittest::Queryable;
        let mut harness = egui_kittest::Harness::builder()
            .with_size(egui::vec2(900.0, 700.0))
            .with_max_steps(16)
            .build_ui_state(
                |ui, notice: &mut Option<String>| {
                    crate::test_support::install_fonts(ui);
                    notice_dialog(ui.ctx(), notice);
                },
                Some("Java was not found: æøå.".to_owned()),
            );
        harness.get_by_label("The SQL window cannot open");
        crate::test_support::keeps_its_place(&mut harness, "OK");
        harness.get_by_label("OK").click();
        harness.run();
        assert!(harness.state().is_none());
    }

    fn run(statement: &str) -> history::Entry {
        history::Entry::now(statement, std::time::Duration::ZERO, history::Outcome::Done)
    }

    /// R-SQL-89..91: runs go into the connection's history; Remove takes one run out and Clear
    /// empties it, for this connection only.
    #[test]
    fn runs_are_kept_and_removed_or_cleared_per_connection_r_sql_89() {
        let mut settings = Settings::default();
        settings
            .sql_history
            .insert("Other".to_owned(), vec![run("VALUES 'æøå'")]);
        let mut window = crate::tests::window();
        window.ran = vec![run("SELECT 1"), run("SELECT 1"), run("SELECT 2")];
        keep(&mut window, "Prod", &mut settings, &mut 0);
        assert_eq!(settings.sql_history["Prod"].len(), 3, "every run");
        let second = settings.sql_history["Prod"][1].clone();
        window.history_edits = vec![HistoryAction::Remove(second)];
        keep(&mut window, "Prod", &mut settings, &mut 0);
        assert_eq!(settings.sql_history["Prod"].len(), 2);
        window.history_edits = vec![HistoryAction::Clear];
        keep(&mut window, "Prod", &mut settings, &mut 0);
        assert_eq!(settings.sql_history["Prod"], []);
        assert_eq!(
            settings.sql_history["Other"].len(),
            1,
            "another connection's stays"
        );
    }

    #[test]
    fn a_renamed_connection_keeps_its_history_scripts_and_pins_r_sql_46() {
        let mut settings = Settings::default();
        settings
            .sql_history
            .insert("Prod".to_owned(), vec![run("VALUES 1")]);
        settings
            .sql_pinned
            .insert("Prod".to_owned(), vec!["QSYS2".to_owned()]);
        rename(&mut settings, "Prod", "Production");
        assert!(!settings.sql_history.contains_key("Prod"));
        assert_eq!(settings.sql_history["Production"][0].statement, "VALUES 1");
        assert_eq!(settings.sql_pinned["Production"], ["QSYS2"]);
    }

    /// R-SQL-83: a connection's pinned libraries become its group "Pinned", which is kept in
    /// their place, and follows a renamed connection (R-SQL-46).
    #[test]
    fn pinned_libraries_are_kept_as_a_group_that_follows_a_rename_r_sql_83() {
        let mut settings = Settings::default();
        settings
            .sql_pinned
            .insert("Prod".to_owned(), vec!["QSYS2".to_owned()]);
        let groups = browser::from_pinned(settings.sql_pinned["Prod"].clone());
        let mut window = crate::tests::window().with_side(settings.sql_side, groups);
        keep(&mut window, "Prod", &mut settings, &mut 0);
        assert!(settings.sql_pinned.is_empty(), "read once, then removed");
        assert_eq!(settings.sql_groups["Prod"][0].name, "Pinned");
        assert_eq!(settings.sql_groups["Prod"][0].libraries, ["QSYS2"]);
        rename(&mut settings, "Prod", "Production");
        assert_eq!(settings.sql_groups["Production"][0].libraries, ["QSYS2"]);
        assert!(!settings.sql_groups.contains_key("Prod"));
    }

    #[test]
    fn placeholder_values_are_kept_by_connection_and_follow_a_rename_r_sql_113() {
        let mut settings = Settings::default();
        let used = crate::RecentValues::from([("lib".to_owned(), vec!["æøå".to_owned()])]);
        let mut window = crate::tests::window().with_snippet_values(used.clone());
        keep(&mut window, "Prod", &mut settings, &mut 0);
        assert_eq!(settings.sql_snippet_values["Prod"], used);
        let text = serde_json::to_string(&settings).expect("settings write");
        let read: Settings = serde_json::from_str(&text).expect("settings read");
        assert_eq!(read.sql_snippet_values["Prod"], used, "after a restart");
        rename(&mut settings, "Prod", "Production");
        assert_eq!(settings.sql_snippet_values["Production"], used);
        assert!(!settings.sql_snippet_values.contains_key("Prod"));
    }

    #[test]
    fn runs_are_kept_as_many_as_set_and_not_while_the_history_is_off_r_sql_121() {
        let mut settings = Settings {
            sql_history_size: 2,
            ..Settings::default()
        };
        let mut window = crate::tests::window();
        window.ran = vec![run("A"), run("B"), run("æøå")];
        keep(&mut window, "Prod", &mut settings, &mut 0);
        let kept: Vec<&str> = settings.sql_history["Prod"]
            .iter()
            .map(|e| e.statement.as_str())
            .collect();
        assert_eq!(kept, ["B", "æøå"]);
        settings.sql_history_keep = false;
        window.ran = vec![run("C")];
        keep(&mut window, "Prod", &mut settings, &mut 0);
        assert_eq!(
            settings.sql_history["Prod"].len(),
            2,
            "none added, none removed"
        );
    }

    #[test]
    fn values_are_forgotten_when_remembering_is_turned_off_r_sql_122() {
        let mut settings = Settings::default();
        let used = crate::RecentValues::from([("lib".to_owned(), vec!["æøå".to_owned()])]);
        settings
            .sql_snippet_values
            .insert("Other".to_owned(), used.clone());
        let mut window = crate::tests::window().with_snippet_values(used);
        settings.sql_remember_values = false;
        keep(&mut window, "Prod", &mut settings, &mut 0);
        assert!(settings.sql_snippet_values.is_empty(), "those kept go too");
    }

    #[test]
    fn without_the_helper_the_window_says_why_it_cannot_open_r_sql_45() {
        let config = SessionConfig::default();
        let opened = open("Prod", &config, None, "", &Settings::default());
        assert_eq!(opened.err().as_deref(), Some(NO_HELPER));
    }
}
