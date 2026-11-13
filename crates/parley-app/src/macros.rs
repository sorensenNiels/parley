//! Macros as text files (D14, R-KEY-3): `<name>.macro` in a `macros` folder next to Parley's
//! settings, so people can read, edit and share them.

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use parley_core::macros::Macro;
use parley_ui_egui::{MacroStore, StoredMacro, check_macro_name};

const EXTENSION: &str = "macro";

/// The macro files in one folder.
#[derive(Debug)]
pub struct MacroFolder {
    dir: PathBuf,
}

impl MacroFolder {
    /// The folder next to eframe's settings for the app id `parley`.
    pub fn beside_settings() -> Option<Self> {
        eframe::storage_dir("parley").map(|dir| Self::new(dir.join("macros")))
    }

    pub const fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.join(format!("{name}.{EXTENSION}"))
    }

    /// Copies each of `paths` in as a macro (R-KEY-7); see [`Self::import_file`].
    fn import_files(&self, paths: &[PathBuf]) -> Vec<Result<String, String>> {
        paths.iter().map(|path| self.import_file(path)).collect()
    }

    /// Copies the macro file at `path` in, named after the file, as it is. A file that does
    /// not read as a macro, or whose name a macro has, is not taken.
    fn import_file(&self, path: &Path) -> Result<String, String> {
        let shown = path.display();
        let stem = path.file_stem().unwrap_or_default().to_string_lossy();
        let name = check_macro_name(&stem).map_err(|e| format!("{shown}: {e}"))?;
        let text = fs::read_to_string(path).map_err(|e| format!("{shown}: {e}"))?;
        Macro::parse(&text).map_err(|e| format!("{shown}: {e}"))?;
        if self.path(&name).exists() {
            return Err(format!(
                "{shown}: there is already a macro called \"{name}\""
            ));
        }
        self.save_text(&name, &text)?;
        Ok(name)
    }

    /// Copies the macro `name` to `path` (R-KEY-7).
    fn export_to(&self, name: &str, path: &Path) -> Result<(), String> {
        fs::copy(self.path(name), path)
            .map(drop)
            .map_err(|e| e.to_string())
    }
}

/// A file window for macro files, titled `title`.
fn macro_file_window(title: &str) -> rfd::FileDialog {
    rfd::FileDialog::new()
        .set_title(title)
        .add_filter("Parley macro", &[EXTENSION])
}

impl MacroStore for MacroFolder {
    fn list(&self) -> Result<Vec<StoredMacro>, String> {
        let entries = match fs::read_dir(&self.dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e.to_string()),
        };
        let mut list: Vec<StoredMacro> = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|e| e == EXTENSION))
            .filter_map(|path| {
                let name = path.file_stem()?.to_string_lossy().into_owned();
                let contents = fs::read_to_string(&path)
                    .map_err(|e| e.to_string())
                    .and_then(|text| Macro::parse(&text).map_err(|e| e.to_string()));
                Some(StoredMacro { name, contents })
            })
            .collect();
        list.sort_by_key(|m| m.name.to_lowercase());
        Ok(list)
    }

    fn read_text(&self, name: &str) -> Result<String, String> {
        fs::read_to_string(self.path(name)).map_err(|e| e.to_string())
    }

    fn save_text(&self, name: &str, text: &str) -> Result<(), String> {
        fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;
        fs::write(self.path(name), text).map_err(|e| e.to_string())
    }

    fn delete(&self, name: &str) -> Result<(), String> {
        match fs::remove_file(self.path(name)) {
            Err(e) if e.kind() != ErrorKind::NotFound => Err(e.to_string()),
            _ => Ok(()),
        }
    }

    fn rename(&self, old: &str, new: &str) -> Result<(), String> {
        let target = self.path(new);
        // A name differing only in case is the same file on Windows and macOS.
        if target.exists() && !old.eq_ignore_ascii_case(new) {
            return Err(format!("there is already a macro called \"{new}\""));
        }
        fs::rename(self.path(old), target).map_err(|e| e.to_string())
    }

    fn folder(&self) -> Option<String> {
        Some(self.dir.display().to_string())
    }

    /// The file window blocks the frame while it is open, as Tortuga's do: GTK and macOS
    /// want it on the UI thread.
    fn import(&self) -> Vec<Result<String, String>> {
        let paths = macro_file_window("Import macros")
            .pick_files()
            .unwrap_or_default();
        self.import_files(&paths)
    }

    fn export(&self, name: &str) -> Result<Option<String>, String> {
        let Some(path) = macro_file_window("Export macro")
            .set_file_name(format!("{name}.{EXTENSION}"))
            .save_file()
        else {
            return Ok(None);
        };
        self.export_to(name, &path)?;
        Ok(Some(path.display().to_string()))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use parley_core::macros::Step;

    use super::*;

    /// A fresh folder under the system's temporary folder, removed when dropped.
    pub(crate) struct TempDir(pub(crate) PathBuf);

    impl TempDir {
        pub(crate) fn new(test: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("parley-{test}-{}", std::process::id()));
            // Left over from an earlier run that panicked: harmless if it is not there.
            let _ = fs::remove_dir_all(&dir);
            Self(dir)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            // Cleanup only: a failure leaves a folder in the temporary folder.
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn macro_files_are_imported_and_exported_as_they_are_r_key_7() {
        let temp = TempDir::new("macro-import");
        let folder = MacroFolder::new(temp.0.join("macros"));
        let outside = temp.0.join("outside");
        fs::create_dir_all(&outside).expect("a folder");
        let good = outside.join("Sign on æøå.macro");
        let text = "# shared\ntext \"wrkactjob\"\nenter\n";
        fs::write(&good, text).expect("written");
        let bad = outside.join("Broken.macro");
        fs::write(&bad, "teleport\n").expect("written");

        let results = folder.import_files(&[good.clone(), bad]);
        assert_eq!(results[0], Ok("Sign on æøå".to_owned()));
        assert!(
            results[1].as_ref().is_err_and(|e| e.contains("line 1")),
            "{results:?}"
        );
        assert_eq!(folder.read_text("Sign on æøå").as_deref(), Ok(text));
        assert!(folder.read_text("Broken").is_err(), "not taken");
        let again = folder.import_files(&[good]);
        assert!(
            again[0].as_ref().is_err_and(|e| e.contains("already")),
            "{again:?}"
        );

        let copy = outside.join("copy.macro");
        folder.export_to("Sign on æøå", &copy).expect("exported");
        assert_eq!(fs::read_to_string(&copy).as_deref().ok(), Some(text));
        assert!(
            folder
                .export_to("Gone", &outside.join("gone.macro"))
                .is_err()
        );
    }

    #[test]
    fn a_macro_file_is_edited_as_text_with_its_comments_r_key_5() {
        let temp = TempDir::new("macro-text");
        let folder = MacroFolder::new(temp.0.join("macros"));
        let text = "# Sign on, then æøå\ntext \"æøå\"\n\nenter\n";
        folder.save_text("Mine", text).expect("saved");
        assert_eq!(folder.read_text("Mine").as_deref(), Ok(text));
        let listed = folder.list().expect("a list");
        assert!(listed[0].contents.is_ok(), "{listed:?}");
        assert!(folder.read_text("Gone").is_err());
    }

    #[test]
    fn macros_are_saved_listed_renamed_and_deleted_as_files_r_key_3() {
        let temp = TempDir::new("macros");
        let folder = MacroFolder::new(temp.0.join("macros"));
        assert_eq!(folder.list(), Ok(Vec::new()), "no folder yet is no macros");
        let m = Macro {
            session: Some("PUB400".into()),
            steps: vec![Step::Text("æøå".into()), Step::Wait],
        };
        folder.save("Jobs æ", &m).expect("saved");
        folder.save("b", &Macro::default()).expect("saved");
        let text = fs::read_to_string(temp.0.join("macros/Jobs æ.macro")).expect("a file");
        assert!(text.contains("text \"æøå\""), "{text}");
        let names: Vec<_> = folder
            .list()
            .expect("listed")
            .into_iter()
            .map(|m| m.name)
            .collect();
        assert_eq!(names, ["b", "Jobs æ"]);
        assert!(folder.rename("b", "Jobs æ").is_err(), "taken");
        folder.rename("b", "c").expect("renamed");
        folder.delete("Jobs æ").expect("deleted");
        folder
            .delete("gone")
            .expect("deleting a missing macro is fine");
        let list = folder.list().expect("listed");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "c");
    }

    #[test]
    fn a_file_that_does_not_parse_is_listed_with_the_reason_r_key_4() {
        let temp = TempDir::new("bad-macro");
        fs::create_dir_all(&temp.0).expect("folder");
        fs::write(temp.0.join("bad.macro"), "enter\nf99\n").expect("written");
        fs::write(temp.0.join("notes.txt"), "not a macro").expect("written");
        let list = MacroFolder::new(temp.0.clone()).list().expect("listed");
        assert_eq!(list.len(), 1);
        assert!(
            list[0]
                .contents
                .as_ref()
                .is_err_and(|e| e.contains("line 2"))
        );
    }
}
