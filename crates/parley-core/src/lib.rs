//! UI-agnostic TN5250 engine for Parley.
//!
//! The protocol is sans-I/O: [`Session`] takes the bytes the host sent and produces the bytes
//! to send back, and is driven by keys from the UI. [`net::Connection`] runs a session over a
//! TCP or TLS socket on a background thread.

pub mod ebcdic;
pub mod error;
pub mod hostserver;
pub mod macros;
pub mod net;
pub mod password;
pub mod record;
pub mod screen;
pub mod session;
pub mod sql;
pub mod telnet;
pub mod update;

pub use ebcdic::CodePage;
pub use error::{Error, Result};
pub use screen::{Color, Screen, ScreenSize, Style};
pub use session::{Aid, Key, KeyboardState, MacroState, OperatorError, Session, SessionConfig};
