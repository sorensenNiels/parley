//! What both of Parley's programs need from the desktop: `parley`, the terminal, and
//! `parley-sql`, Parley SQL, the SQL window on its own (R-SQL-45). The window icon, the SQL
//! helper's jars, the assistant's API keys in the keychain, the file window, the snippet and macro folders, the update check and the
//! development screenshot. UI code belongs in `parley-ui-egui`; the engine in `parley-core`.

pub mod api_keys;
pub mod helpers;
pub mod macros;
#[expect(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "a development option of the binaries, which says where it saved the window"
)]
pub mod screenshot;
pub mod snippets;

use eframe::egui;

/// Parley SQL's eframe storage id: where its connections and settings are kept (D26).
pub const PARLEY_SQL_ID: &str = "parley-sql";

/// Where Parley SQL keeps its settings in its eframe storage.
pub const SETTINGS_KEY: &str = "settings";

/// Parley SQL's saved settings, which the SQL window Parley opens uses too (R-SQL-69); `None`
/// while Parley SQL has saved none. A file that cannot be read or decoded counts as none, as
/// eframe itself treats it: the window then opens with the default settings.
#[must_use]
pub fn parley_sql_settings() -> Option<parley_ui_egui::Settings> {
    let file = eframe::storage_dir(PARLEY_SQL_ID)?.join("app.ron");
    settings_in(&std::fs::read_to_string(file).ok()?)
}

/// The settings in the text of an eframe storage file: a map of keys to RON values.
fn settings_in(storage: &str) -> Option<parley_ui_egui::Settings> {
    let values: std::collections::HashMap<String, String> = ron::from_str(storage).ok()?;
    ron::from_str(values.get(SETTINGS_KEY)?).ok()
}

/// The window icon.
///
/// # Errors
/// When the bundled PNG cannot be decoded.
pub fn window_icon() -> Result<egui::IconData, String> {
    eframe::icon_data::from_png_bytes(include_bytes!("../icons/256.png")).map_err(|e| e.to_string())
}

/// The icon for the start of the menu bar.
///
/// # Errors
/// When the bundled PNG cannot be decoded.
pub fn logo() -> Result<egui::ColorImage, String> {
    let icon = eframe::icon_data::from_png_bytes(include_bytes!("../icons/64.png"))
        .map_err(|e| e.to_string())?;
    Ok(egui::ColorImage::from_rgba_unmultiplied(
        [icon.width as usize, icon.height as usize],
        &icon.rgba,
    ))
}

/// Asks GitHub for the latest release of Parley, which Parley SQL ships with (R-UI-11).
#[must_use]
pub fn update_checker() -> parley_ui_egui::Checker {
    std::sync::Arc::new(|| {
        parley_core::update::check(
            parley_core::update::LATEST_RELEASE_URL,
            env!("CARGO_PKG_VERSION"),
        )
        .map_err(|e| e.to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parley_sql_settings_are_read_from_its_storage_file_r_sql_69() {
        let settings = parley_ui_egui::Settings {
            java_path: "/opt/java/æøå".to_owned(),
            sql_rows: 250,
            ..parley_ui_egui::Settings::default()
        };
        let value = ron::to_string(&settings).expect("settings serialize");
        let values = std::collections::HashMap::from([(SETTINGS_KEY.to_owned(), value)]);
        let file = ron::to_string(&values).expect("a map of strings serializes");
        let read = settings_in(&file).expect("the settings are found");
        assert_eq!(read.java_path, "/opt/java/æøå");
        assert_eq!(read.sql_rows, 250);
    }

    #[test]
    fn an_empty_or_broken_storage_file_has_no_settings_r_sql_69() {
        assert!(settings_in("").is_none());
        assert!(settings_in("{\"settings\": \"(not settings\"}").is_none());
        assert!(settings_in("{}").is_none());
    }
}
