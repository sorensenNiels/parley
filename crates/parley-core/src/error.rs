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

    /// A record from an IBM i host server is not valid (R-SQL-1).
    #[error("invalid data from the IBM i host server: {what}")]
    HostServer { what: String },

    /// A sign-on that cannot be sent, or that the host refused (R-SQL-2): a sentence for the
    /// operator. Never holds the password.
    #[error("{0}")]
    SignOn(String),

    #[error("TLS: {0}")]
    Tls(String),

    #[error("cannot read the CA certificate file {path}: {message}")]
    CaFile { path: String, message: String },

    /// Asking GitHub for the latest release failed (R-UI-11).
    #[error("could not check for a new version: {0}")]
    UpdateCheck(String),

    /// A macro file that cannot be read (R-KEY-4); the text names the line.
    #[error("{0}")]
    Macro(String),

    #[error("the connection failed: {0}")]
    Io(#[from] io::Error),
}

impl Error {
    pub(crate) fn data(what: impl Into<String>) -> Self {
        Self::DataStream { what: what.into() }
    }

    pub(crate) fn host_server(what: impl Into<String>) -> Self {
        Self::HostServer { what: what.into() }
    }

    pub(crate) fn unsupported(what: impl Into<String>) -> Self {
        Self::Unsupported { what: what.into() }
    }
}

pub type Result<T> = std::result::Result<T, Error>;
