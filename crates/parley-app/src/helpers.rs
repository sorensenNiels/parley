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
/// Resources; the `parley-sql` package's `/usr/lib/parley-sql` (R-NF-4), the all-in-one .deb's
/// `/usr/lib/parley` before it; inside an `AppImage` (`APPDIR`); and `target/helpers`
/// of a development build (the program in `target/debug` or `target/release`).
fn candidates(exe: &Path, configured: Option<PathBuf>, appdir: Option<PathBuf>) -> Vec<PathBuf> {
    let dir = exe.parent().unwrap_or_else(|| Path::new("."));
    let mut out: Vec<PathBuf> = configured.into_iter().collect();
    out.push(dir.join("helpers"));
    out.push(dir.join("../Resources/helpers"));
    out.push(PathBuf::from("/usr/lib/parley-sql/helpers"));
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
        let linux = candidates(Path::new("/usr/bin/parley-sql"), None, None);
        assert!(
            linux.contains(&PathBuf::from("/usr/lib/parley-sql/helpers")),
            "the parley-sql package's (R-NF-4)"
        );
        let windows = candidates(Path::new("C:/Program Files/Parley/parley.exe"), None, None);
        assert!(windows.contains(&PathBuf::from("C:/Program Files/Parley/helpers")));
    }
}

/// The SQL window's files through the system's file window (R-SQL-8, R-SQL-21), as macro
/// import and export (rfd, GTK 3 on Linux).
#[derive(Debug)]
pub struct SqlFileWindow;

impl parley_sql_ui::SqlFiles for SqlFileWindow {
    fn open_file(&self, path: &std::path::Path) -> Result<(), String> {
        opener::open(path).map_err(|e| e.to_string())
    }

    fn show_in_folder(&self, path: &std::path::Path) -> Result<(), String> {
        let folder = path.parent().unwrap_or(path);
        opener::open(folder).map_err(|e| e.to_string())
    }

    fn export_csv(&self, suggested: &str, bytes: &[u8]) -> Result<Option<String>, String> {
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

    fn download_path(
        &self,
        suggested: &str,
        format: parley_sql_core::transfer::Format,
    ) -> Result<Option<std::path::PathBuf>, String> {
        let (name, extension) = match format {
            parley_sql_core::transfer::Format::Csv => ("CSV", "csv"),
            parley_sql_core::transfer::Format::Excel => ("Excel", "xlsx"),
            parley_sql_core::transfer::Format::Sql => ("SQL", "sql"),
        };
        Ok(rfd::FileDialog::new()
            .set_title("Download rows")
            .add_filter(name, &[extension])
            .set_file_name(suggested)
            .save_file())
    }

    fn open_csv(&self) -> Result<Option<(String, String)>, String> {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Upload CSV")
            .add_filter("CSV", &["csv", "txt"])
            .pick_file()
        else {
            return Ok(None);
        };
        let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
        let text = String::from_utf8(bytes)
            .map_err(|_| format!("{} is not UTF-8 text", path.display()))?;
        Ok(Some((path.display().to_string(), text)))
    }

    fn save_text(&self, suggested: &str, text: &str) -> Result<Option<String>, String> {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Save as text")
            .add_filter("Text", &["txt"])
            .set_file_name(suggested)
            .save_file()
        else {
            return Ok(None);
        };
        std::fs::write(&path, text).map_err(|e| e.to_string())?;
        Ok(Some(path.display().to_string()))
    }

    fn open_script(&self) -> Result<Option<(String, String)>, String> {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Open SQL script")
            .add_filter("SQL", &["sql"])
            .pick_file()
        else {
            return Ok(None);
        };
        let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
        let text = String::from_utf8(bytes)
            .map_err(|_| format!("{} is not UTF-8 text", path.display()))?;
        Ok(Some((path.display().to_string(), text)))
    }

    fn save_script(
        &self,
        path: Option<&str>,
        suggested: &str,
        text: &str,
    ) -> Result<Option<String>, String> {
        let path = match path {
            Some(path) => std::path::PathBuf::from(path),
            None => match rfd::FileDialog::new()
                .set_title("Save SQL script")
                .add_filter("SQL", &["sql"])
                .set_file_name(suggested)
                .save_file()
            {
                Some(path) => path,
                None => return Ok(None),
            },
        };
        std::fs::write(&path, text).map_err(|e| e.to_string())?;
        Ok(Some(path.display().to_string()))
    }
}

/// Parley SSH's snippets exported to and imported from one JSON file (R-SSH-49), through the system's file window.
#[derive(Debug)]
pub struct SnippetFileWindow;

impl parley_ssh_ui::snippets::SnippetFiles for SnippetFileWindow {
    fn export(&self, json: &str) -> Result<Option<String>, String> {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Export snippets")
            .add_filter("Parley SSH snippets", &["json"])
            .set_file_name("parley-ssh-snippets.json")
            .save_file()
        else {
            return Ok(None);
        };
        std::fs::write(&path, json)
            .map_err(|e| format!("{} could not be saved: {e}", path.display()))?;
        Ok(Some(path.display().to_string()))
    }

    fn import(&self) -> Result<Option<String>, String> {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Import snippets")
            .add_filter("Parley SSH snippets", &["json"])
            .pick_file()
        else {
            return Ok(None);
        };
        std::fs::read_to_string(&path)
            .map(Some)
            .map_err(|e| format!("{} could not be read: {e}", path.display()))
    }
}

/// Desktop paths for Parley SSH's transfers (R-SSH-55), chosen through the system's file window.
#[derive(Debug)]
pub struct DesktopFileWindow;

impl parley_ssh_ui::transfer::DesktopFiles for DesktopFileWindow {
    fn choose(&self, folder: bool) -> Option<String> {
        let window = rfd::FileDialog::new();
        let chosen = if folder {
            window.set_title("Choose a folder").pick_folder()
        } else {
            window.set_title("Choose a file").pick_file()
        };
        chosen.map(|path| path.display().to_string())
    }
}
