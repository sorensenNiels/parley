//! The SQL add-on's engine (Phase 5, D20): Parley runs SQL on an IBM i through a Java helper
//! with IBM's `JTOpen` (`helpers/sql`). [`protocol`] is what the two say to each other, sans-I/O
//! (D4); [`java`] finds a Java to run the helper with (R-SQL-12); [`helper`] runs it (R-SQL-11).

pub mod catalog;
pub mod complete;
pub mod helper;
pub mod java;
pub mod message;
pub mod protocol;
pub mod runner;
pub mod snippet;
pub mod text;
