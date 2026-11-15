//! Sessions and fonts for the UI tests, also Parley SQL's (`features = ["testing"]`).

use parley_core::record::{self, Opcode};
use parley_core::telnet::Telnet;
use parley_core::{CodePage, Session, SessionConfig};
use std::cell::RefCell;
use std::rc::Rc;

use parley_core::macros::{Macro, Step};

use crate::{MacroStore, Profile, StoredMacro};

/// Parley's fonts, once per context, for a widget tested on its own: the icons and
/// `theme::semibold` need them.
pub fn install_fonts(ui: &egui::Ui) {
    let id = egui::Id::unique("test_support::install_fonts");
    let done = ui
        .ctx()
        .data_mut(|d| std::mem::replace(d.get_temp_mut_or_default::<bool>(id), true));
    if !done {
        crate::theme::install(ui.ctx(), crate::fonts::DEFAULT_TERMINAL_FONT);
    }
}

/// A session showing "Hello" and one input field at row 2, column 2, waiting for input.
pub fn waiting_session() -> Session {
    let mut session = Session::new(SessionConfig::default());
    session.receive(&waiting_screen());
    session
}

/// The host's record behind [`waiting_session`]: the screen, then a read that unlocks the
/// keyboard.
pub fn waiting_screen() -> Vec<u8> {
    let mut data = vec![0x04, 0x40, 0x04, 0x11, 0x00, 0x00, 0x11, 1, 1];
    data.extend("Hello".chars().filter_map(|c| CodePage::Ccsid37.to_byte(c)));
    data.extend([
        0x11, 2, 1, 0x1D, 0x40, 0x00, 0x24, 0x00, 0x0A, 0x04, 0x52, 0x00, 0x00,
    ]);
    Telnet::frame(&record::build(0, Opcode::PutGet, &data))
}

/// Macros in memory, shared with the test so it can look inside.
#[derive(Debug, Clone, Default)]
pub struct MemoryMacros(pub Rc<RefCell<std::collections::BTreeMap<String, Macro>>>);

impl MacroStore for MemoryMacros {
    fn list(&self) -> Result<Vec<StoredMacro>, String> {
        Ok(self
            .0
            .borrow()
            .iter()
            .map(|(name, m)| StoredMacro {
                name: name.clone(),
                contents: Ok(m.clone()),
            })
            .collect())
    }

    fn read_text(&self, name: &str) -> Result<String, String> {
        let macros = self.0.borrow();
        let m = macros.get(name).ok_or("no such macro")?;
        Ok(m.to_text())
    }

    fn save_text(&self, name: &str, text: &str) -> Result<(), String> {
        let m = Macro::parse(text).map_err(|e| e.to_string())?;
        self.0.borrow_mut().insert(name.to_owned(), m);
        Ok(())
    }

    fn delete(&self, name: &str) -> Result<(), String> {
        self.0.borrow_mut().remove(name);
        Ok(())
    }

    fn rename(&self, old: &str, new: &str) -> Result<(), String> {
        let m = self.0.borrow_mut().remove(old).ok_or("no such macro")?;
        self.0.borrow_mut().insert(new.to_owned(), m);
        Ok(())
    }

    fn folder(&self) -> Option<String> {
        Some("/memory".to_owned())
    }

    /// As if the file `Imported.macro` was chosen.
    fn import(&self) -> Vec<Result<String, String>> {
        self.0
            .borrow_mut()
            .insert("Imported".to_owned(), Macro::default());
        vec![Ok("Imported".to_owned())]
    }

    /// As if the copy was saved in `/elsewhere`.
    fn export(&self, name: &str) -> Result<Option<String>, String> {
        Ok(Some(format!("/elsewhere/{name}.macro")))
    }
}

/// A store holding one macro for every session, called `name`.
pub fn macros_with(name: &str) -> MemoryMacros {
    let store = MemoryMacros::default();
    store.0.borrow_mut().insert(
        name.to_owned(),
        Macro {
            session: None,
            steps: vec![Step::Text("wrkactjob".into()), Step::Wait],
        },
    );
    store
}

pub fn profile(name: &str, host: &str) -> Profile {
    Profile {
        name: name.to_owned(),
        config: SessionConfig {
            host: host.to_owned(),
            ..SessionConfig::default()
        },
        ..Profile::default()
    }
}
