//! The SQL window's snippets as `.sql` files (R-SQL-24): the operator's own in a `snippets`
//! folder next to Parley's settings, and a shared folder named in the settings.

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use parley_ui_egui::SnippetStore;

const EXTENSION: &str = "sql";

/// The snippet folders.
#[derive(Debug)]
pub struct SnippetFolders {
    mine: Option<PathBuf>,
}

impl SnippetFolders {
    /// Mine next to eframe's settings for the app id `parley`.
    pub fn beside_settings() -> Self {
        Self::new(eframe::storage_dir("parley").map(|dir| dir.join("snippets")))
    }

    pub const fn new(mine: Option<PathBuf>) -> Self {
        Self { mine }
    }
}

/// The file of snippet `name` in `folder`; a name that would leave the folder is refused.
fn path(folder: &str, name: &str) -> Result<PathBuf, String> {
    if name.is_empty() || name.contains(['/', '\\']) || name.starts_with('.') {
        return Err(format!("\"{name}\" cannot be a file name"));
    }
    Ok(Path::new(folder).join(format!("{name}.{EXTENSION}")))
}

impl SnippetStore for SnippetFolders {
    fn mine(&self) -> Option<String> {
        self.mine.as_ref().map(|p| p.display().to_string())
    }

    fn list(&self, folder: &str) -> Result<Vec<(String, String)>, String> {
        let entries = match fs::read_dir(folder) {
            Ok(entries) => entries,
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e.to_string()),
        };
        let mut list = Vec::new();
        for path in entries.filter_map(Result::ok).map(|entry| entry.path()) {
            if path.extension().is_none_or(|e| e != EXTENSION) {
                continue;
            }
            let name = path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            // A file that cannot be read as text is left out, not the whole folder.
            if let Ok(text) = fs::read_to_string(&path) {
                list.push((name, text));
            }
        }
        list.sort();
        Ok(list)
    }

    fn save(&self, folder: &str, name: &str, text: &str) -> Result<(), String> {
        let path = path(folder, name)?;
        fs::create_dir_all(folder).map_err(|e| e.to_string())?;
        fs::write(path, text).map_err(|e| e.to_string())
    }

    fn delete(&self, folder: &str, name: &str) -> Result<(), String> {
        fs::remove_file(path(folder, name)?).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::macros::tests::TempDir;

    #[test]
    fn snippets_are_sql_files_saved_listed_and_deleted_r_sql_24() {
        let temp = TempDir::new("snippets");
        let folders = SnippetFolders::new(Some(temp.0.join("snippets")));
        let mine = folders.mine().expect("a folder");
        assert_eq!(
            folders.list(&mine),
            Ok(Vec::new()),
            "not made until a snippet is saved"
        );
        folders
            .save(&mine, "Låse-æøå", "-- title: Låse\nVALUES 1\n")
            .expect("saved");
        fs::write(temp.0.join("snippets/notes.txt"), "not a snippet").expect("written");
        assert_eq!(
            folders.list(&mine),
            Ok(vec![(
                "Låse-æøå".to_owned(),
                "-- title: Låse\nVALUES 1\n".to_owned()
            )])
        );
        folders.delete(&mine, "Låse-æøå").expect("deleted");
        assert_eq!(folders.list(&mine), Ok(Vec::new()));
    }

    #[test]
    fn a_name_cannot_leave_the_folder_r_sql_24() {
        let temp = TempDir::new("snippets-names");
        let folders = SnippetFolders::new(Some(temp.0.clone()));
        let mine = folders.mine().expect("a folder");
        for name in ["../x", "a/b", "a\\b", ".hidden", ""] {
            assert!(folders.save(&mine, name, "VALUES 1").is_err(), "{name}");
        }
    }
}
