//! The SQL assistant's API keys in the operating system's keychain (R-SQL-48, D27): Keychain
//! Services on macOS, the Credential Manager on Windows, the Secret Service (GNOME Keyring,
//! KDE Wallet) on Linux. One entry per provider, shared by Parley and Parley SQL.

use keyring::{Entry, Error};
use parley_core::sql::assistant::Provider;
use parley_ui_egui::ApiKeys;

/// The keychain service the keys are filed under; the account is the provider's id.
const SERVICE: &str = "Parley SQL assistant";

/// The keychain.
#[derive(Debug, Default, Clone, Copy)]
pub struct Keychain;

fn entry(provider: Provider) -> Result<Entry, String> {
    Entry::new(SERVICE, provider.id()).map_err(|e| said(&e))
}

/// The error in words; a system without a keychain says so.
fn said(error: &Error) -> String {
    match error {
        Error::NoDefaultStore => "this system has no keychain".to_owned(),
        other => other.to_string(),
    }
}

impl ApiKeys for Keychain {
    fn get(&self, provider: Provider) -> Result<Option<String>, String> {
        match entry(provider)?.get_password() {
            Ok(key) => Ok(Some(key)),
            Err(Error::NoEntry) => Ok(None),
            Err(e) => Err(said(&e)),
        }
    }

    fn set(&self, provider: Provider, key: &str) -> Result<(), String> {
        entry(provider)?.set_password(key).map_err(|e| said(&e))
    }

    fn delete(&self, provider: Provider) -> Result<(), String> {
        match entry(provider)?.delete_credential() {
            Ok(()) | Err(Error::NoEntry) => Ok(()),
            Err(e) => Err(said(&e)),
        }
    }
}
