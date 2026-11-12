//! Saved sessions (R-UI-5): each one a name, the connection settings and what the window does
//! around the session. Passwords are not part of them: they live in the operating system's
//! keychain (R-P-3), behind [`Keychain`].

use parley_core::SessionConfig;
use serde::{Deserialize, Serialize};

const TLS_PORT: u16 = 992;

/// One saved session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Profile {
    pub name: String,
    pub config: SessionConfig,
    /// Close Parley when the host ends the session (sign-off, or the line drops).
    pub exit_on_close: bool,
    /// Ask before closing the window while connected.
    pub confirm_exit: bool,
    /// Connect when Parley starts; at most one session has it.
    pub connect_on_start: bool,
    /// The macro to run after sign-on (R-KEY-4); empty for none.
    pub run_macro: String,
}

impl Default for Profile {
    /// A new session: TLS on port 992, as R-P-3 asks.
    fn default() -> Self {
        Self {
            name: String::new(),
            config: SessionConfig {
                tls: true,
                port: TLS_PORT,
                ..SessionConfig::default()
            },
            exit_on_close: false,
            confirm_exit: true,
            connect_on_start: false,
            run_macro: String::new(),
        }
    }
}

impl Profile {
    /// A session for a host given on the command line or kept from before sessions had names.
    pub fn for_config(config: SessionConfig) -> Self {
        Self {
            name: config.host.clone(),
            config,
            ..Self::default()
        }
    }
}

/// Where passwords are kept: the operating system's keychain in the app, memory in tests.
/// Errors are display text: the UI only shows them.
pub trait Keychain: std::fmt::Debug {
    /// The password saved for `session`, `None` if there is none.
    fn password(&self, session: &str) -> Result<Option<String>, String>;
    fn set_password(&self, session: &str, password: &str) -> Result<(), String>;
    /// Removes the password; removing one that is not there is not an error.
    fn delete_password(&self, session: &str) -> Result<(), String>;
}

/// A keychain that holds nothing, for a UI built without one.
#[derive(Debug)]
pub struct NoKeychain;

impl Keychain for NoKeychain {
    fn password(&self, _: &str) -> Result<Option<String>, String> {
        Ok(None)
    }

    fn set_password(&self, _: &str, _: &str) -> Result<(), String> {
        Err("No keychain is available to keep the password.".to_owned())
    }

    fn delete_password(&self, _: &str) -> Result<(), String> {
        Ok(())
    }
}
