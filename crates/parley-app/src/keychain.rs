//! Removes the passwords Parley 0.2 saved in the operating system's keychain (R-NET-7):
//! Parley no longer signs on by itself, so it keeps no password. Keychain Services on macOS,
//! the Credential Manager on Windows, the Secret Service (GNOME Keyring, KDE Wallet) on Linux.

use keyring::Entry;

/// The keychain service name Parley 0.2 filed each session's password under.
const SERVICE: &str = "Parley";

/// Deletes the password saved for each of `sessions`; returns what could not be deleted, by
/// session. A session without one, or a system without a keychain, has nothing to delete.
pub fn remove_saved_passwords<'a>(sessions: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    sessions
        .into_iter()
        .filter_map(|session| {
            let deleted = Entry::new(SERVICE, session).and_then(|e| e.delete_credential());
            match deleted {
                Ok(()) | Err(keyring::Error::NoEntry | keyring::Error::NoDefaultStore) => None,
                Err(e) => Some(format!("{session}: {e}")),
            }
        })
        .collect()
}
