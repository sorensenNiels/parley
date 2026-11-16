//! Parley SSH's engine (D41): the SSH client, host keys, and the xterm terminal emulation. No UI code.

pub mod error;
pub mod keys;
pub mod known_hosts;
pub mod mouse;
pub mod net;
pub mod openssh_config;
pub mod secret;
pub mod snippet;
pub mod term;

pub use error::Error;
pub use secret::Secret;
