//! Errors from the TN5250 engine.

use std::io;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A 5250 record from the host is not valid; `what` names the part that is wrong.
    #[error("invalid data from the host: {what}")]
    DataStream { what: String },

    /// The host sent a command or order Parley does not support yet.
    #[error("the host sent {what}, which Parley does not support yet")]
    Unsupported { what: String },

    #[error("could not connect to {host}:{port}: {source}")]
    Connect {
        host: String,
        port: u16,
        source: io::Error,
    },

    #[error("TLS: {0}")]
    Tls(String),

    #[error("cannot read the CA certificate file {path}: {message}")]
    CaFile { path: String, message: String },

    #[error("the connection failed: {0}")]
    Io(#[from] io::Error),
}

impl Error {
    pub(crate) fn data(what: impl Into<String>) -> Self {
        Self::DataStream { what: what.into() }
    }

    pub(crate) fn unsupported(what: impl Into<String>) -> Self {
        Self::Unsupported { what: what.into() }
    }
}

pub type Result<T> = std::result::Result<T, Error>;
