//! Parley SQL's engine (Phase 5, D20, D36): Parley SQL runs SQL on an IBM i through a Java
//! helper with IBM's `JTOpen` (`helpers/sql`). UI-agnostic, as the terminal's `parley-core`, which
//! it uses only to connect (`parley_core::net`), for code pages and for [`Password`]. [`protocol`] is what the two say to each other, sans-I/O
//! (D4); [`java`] finds a Java to run the helper with (R-SQL-12); [`helper`] runs it (R-SQL-11).

pub mod assistant;
pub mod catalog;
pub mod compare;
pub mod complete;
pub mod edit;
pub mod error;
pub mod export;
pub mod generate;
pub mod helper;
pub mod history;
pub mod hostserver;
pub mod java;
pub mod message;
pub mod net;
pub mod options;
pub mod params;
pub mod protocol;
pub mod runner;
pub mod services;
pub mod snippet;
pub mod text;
pub mod transfer;
pub mod upload;

pub use error::{Error, Result};
pub use parley_core::password::Password;
