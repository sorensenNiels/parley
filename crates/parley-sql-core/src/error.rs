//! Errors from Parley SQL's engine (D20). What it shares with the terminal (connecting, TLS)
//! fails with the terminal engine's [`parley_core::Error`], carried in [`Error::Core`].

use std::io;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A record from an IBM i host server is not valid (R-SQL-1).
    #[error("invalid data from the IBM i host server: {what}")]
    HostServer { what: String },

    /// A sign-on that cannot be sent, or that the host refused (R-SQL-2): a sentence for the
    /// operator. Never holds the password.
    #[error("{0}")]
    SignOn(String),

    /// An SQL statement or connection the host refused (R-SQL-4): its message and SQLSTATE.
    #[error("{0}")]
    Sql(crate::protocol::SqlError),

    /// The SQL helper could not be run or talked to (R-SQL-11).
    #[error("{0}")]
    SqlHelper(String),

    /// No usable Java for the SQL helper (R-SQL-12): a sentence for the operator.
    #[error("{0}")]
    Java(String),

    /// The SQL assistant could not get an answer (R-SQL-44): a sentence for the operator.
    #[error("{0}")]
    Assistant(String),

    /// A download or upload file that cannot be written or read (R-SQL-71..73): a sentence
    /// for the operator, naming the file.
    #[error("{0}")]
    Transfer(String),

    /// A request that cannot be made from what it was given, such as a spooled file number
    /// that is not a number (R-SQL-78): a sentence for the operator.
    #[error("{0}")]
    Request(String),

    /// Connecting to the host, as the terminal does (R-NET-1, R-NET-2).
    #[error(transparent)]
    Core(#[from] parley_core::Error),

    #[error("the connection failed: {0}")]
    Io(#[from] io::Error),
}

impl Error {
    pub(crate) fn host_server(what: impl Into<String>) -> Self {
        Self::HostServer { what: what.into() }
    }
}

pub type Result<T> = std::result::Result<T, Error>;
