//! Where the SQL helper's jars are (R-SQL-13): the installers put them in a `helpers` folder
//! among the app's resources, a development build finds them in `target/helpers`.

use std::path::{Path, PathBuf};

/// The folder that holds `parley-sql.jar`, if any.
pub fn sql_helpers() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let candidates = candidates(
        &exe,
        std::env::var_os("PARLEY_HELPERS").map(PathBuf::from),
        std::env::var_os("APPDIR").map(PathBuf::from),
    );
    candidates
        .into_iter()
        .find(|folder| folder.join("parley-sql.jar").is_file())
}

/// Where to look, in order: `PARLEY_HELPERS`; beside the program (Windows); the macOS bundle's
/// Resources; the .deb's `/usr/lib/parley`; inside an `AppImage` (`APPDIR`); and `target/helpers`
/// of a development build (the program in `target/debug` or `target/release`).
fn candidates(exe: &Path, configured: Option<PathBuf>, appdir: Option<PathBuf>) -> Vec<PathBuf> {
    let dir = exe.parent().unwrap_or_else(|| Path::new("."));
    let mut out: Vec<PathBuf> = configured.into_iter().collect();
    out.push(dir.join("helpers"));
    out.push(dir.join("../Resources/helpers"));
    out.push(PathBuf::from("/usr/lib/parley/helpers"));
    if let Some(appdir) = appdir {
        out.push(appdir.join("usr/lib/parley/helpers"));
    }
    out.push(dir.join("../helpers"));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_override_comes_first_and_a_development_build_comes_last_r_sql_13() {
        let exe = Path::new("/repo/target/debug/parley");
        let list = candidates(
            exe,
            Some(PathBuf::from("/mine")),
            Some(PathBuf::from("/tmp/.mount_x")),
        );
        assert_eq!(list.first(), Some(&PathBuf::from("/mine")));
        assert!(list.contains(&PathBuf::from("/tmp/.mount_x/usr/lib/parley/helpers")));
        assert_eq!(
            list.last(),
            Some(&PathBuf::from("/repo/target/debug/../helpers"))
        );
    }

    #[test]
    fn the_macos_bundle_and_the_windows_folder_are_looked_in_r_sql_13() {
        let mac = candidates(
            Path::new("/Applications/Parley.app/Contents/MacOS/parley"),
            None,
            None,
        );
        assert!(mac.contains(&PathBuf::from(
            "/Applications/Parley.app/Contents/MacOS/../Resources/helpers"
        )));
        let windows = candidates(Path::new("C:/Program Files/Parley/parley.exe"), None, None);
        assert!(windows.contains(&PathBuf::from("C:/Program Files/Parley/helpers")));
    }
}

/// Saves the SQL window's exports through the system's file window (R-SQL-8), as macro export
/// does (rfd, GTK 3 on Linux).
#[derive(Debug)]
pub struct CsvSaver;

impl parley_ui_egui::FileSaver for CsvSaver {
    fn save(&self, suggested: &str, bytes: &[u8]) -> Result<Option<String>, String> {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Export rows")
            .add_filter("CSV", &["csv"])
            .set_file_name(suggested)
            .save_file()
        else {
            return Ok(None);
        };
        std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
        Ok(Some(path.display().to_string()))
    }
}
