//! Parley SSH's engine errors. The UI turns them into text at the edge.

/// Why a connection could not be made or ended badly.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("could not reach {host}: {source}")]
    Connect {
        host: String,
        source: std::io::Error,
    },
    #[error("no answer from {0} within {1} seconds")]
    Timeout(String, u64),
    #[error("the server offers no {kind} that Parley SSH accepts (it offers {theirs})")]
    NoCommonAlgorithm { kind: String, theirs: String },
    #[error("the host key was not accepted")]
    HostKeyRejected,
    #[error("the server's host key is marked as revoked in known_hosts")]
    HostKeyRevoked,
    #[error("the host key could not be saved: {0}")]
    SaveHostKey(std::io::Error),
    #[error("signing on was cancelled")]
    Cancelled,
    #[error("the server refused the sign-on (it offers: {0})")]
    SignOnRefused(String),
    #[error("the server would not start a shell")]
    ShellRefused,
    #[error("SSH: {0}")]
    Ssh(#[from] russh::Error),
    #[error("could not start the connection's thread: {0}")]
    Runtime(std::io::Error),
}
