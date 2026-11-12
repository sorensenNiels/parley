//! Macros as text files (D14, R-KEY-3): `<name>.macro` in a `macros` folder next to Parley's
//! settings, so people can read, edit and share them.

use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;

use parley_core::macros::Macro;
use parley_ui_egui::{MacroStore, StoredMacro};

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

    fn save(&self, name: &str, contents: &Macro) -> Result<(), String> {
        fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;
        fs::write(self.path(name), contents.to_text()).map_err(|e| e.to_string())
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
}

#[cfg(test)]
mod tests {
    use parley_core::macros::Step;

    use super::*;

    /// A fresh folder under the system's temporary folder, removed when dropped.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(test: &str) -> Self {
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
