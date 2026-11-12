//! Passwords in the operating system's keychain (R-P-3): Keychain Services on macOS, the
//! Credential Manager on Windows, the Secret Service (GNOME Keyring, KDE Wallet) on Linux.

use keyring::Entry;
use parley_ui_egui::Keychain;

/// The keychain service name; each session's password is filed under its name.
const SERVICE: &str = "Parley";

#[derive(Debug)]
pub struct SystemKeychain;

fn entry(session: &str) -> Result<Entry, String> {
    Entry::new(SERVICE, session).map_err(|e| match e {
        keyring::Error::NoDefaultStore => {
            "no keychain is available (on Linux, a Secret Service such as GNOME Keyring must be running)"
                .to_owned()
        }
        e => e.to_string(),
    })
}

impl Keychain for SystemKeychain {
    fn password(&self, session: &str) -> Result<Option<String>, String> {
        match entry(session)?.get_password() {
            Ok(password) => Ok(Some(password)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    fn set_password(&self, session: &str, password: &str) -> Result<(), String> {
        entry(session)?
            .set_password(password)
            .map_err(|e| e.to_string())
    }

    fn delete_password(&self, session: &str) -> Result<(), String> {
        match entry(session)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }
}
