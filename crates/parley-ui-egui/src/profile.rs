//! Saved sessions (R-UI-5): each one a name, the connection settings and what the window does
//! around the session. No user or password: Parley never signs on by itself (R-NET-7).

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
    /// The macro to run on connecting (R-KEY-4); empty for none.
    pub run_macro: String,
    /// The keyboard map's name (R-KEY-2); empty, or a map since deleted, for Default.
    pub keyboard_map: String,
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
            keyboard_map: String::new(),
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
