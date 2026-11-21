//! The SSH agent the user already runs (R-SSH-13, D44; draft-miller-ssh-agent): `SSH_AUTH_SOCK` on Linux and
//! macOS; on Windows OpenSSH's agent, then Pageant. Parley SSH has no agent of its own.

use std::path::PathBuf;

use russh::keys::agent::client::{AgentClient, AgentStream};

/// Where the agent listens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentAddress {
    /// A Unix socket, as `SSH_AUTH_SOCK` names.
    Socket(PathBuf),
    /// Windows: OpenSSH's agent (`\\.\pipe\openssh-ssh-agent`), else Pageant.
    Windows,
}

/// OpenSSH for Windows' agent pipe.
#[cfg(windows)]
const OPENSSH_PIPE: &str = r"\\.\pipe\openssh-ssh-agent";

impl AgentAddress {
    /// The agent the system names: `SSH_AUTH_SOCK` on Unix, the Windows agents on Windows. `None` when there is
    /// none, which is not an error.
    pub fn from_environment() -> Option<Self> {
        if cfg!(windows) {
            return Some(Self::Windows);
        }
        std::env::var_os("SSH_AUTH_SOCK")
            .filter(|v| !v.is_empty())
            .map(|v| Self::Socket(v.into()))
    }
}

/// A connection to an agent of either kind.
pub type Agent = AgentClient<Box<dyn AgentStream + Send + Unpin>>;

/// Connects to the agent at `address`.
pub async fn connect(address: &AgentAddress) -> Result<Agent, russh::keys::Error> {
    let stream: Box<dyn AgentStream + Send + Unpin> = match address {
        #[cfg(unix)]
        AgentAddress::Socket(path) => Box::new(tokio::net::UnixStream::connect(path).await?),
        #[cfg(windows)]
        AgentAddress::Windows => match AgentClient::connect_named_pipe(OPENSSH_PIPE).await {
            Ok(client) => client.into_inner(),
            Err(_) => AgentClient::connect_pageant().await?.into_inner(),
        },
        #[cfg(not(unix))]
        AgentAddress::Socket(_) => return Err(russh::keys::Error::AgentFailure),
        #[cfg(not(windows))]
        AgentAddress::Windows => return Err(russh::keys::Error::AgentFailure),
    };
    Ok(AgentClient::connect(stream))
}

/// The keys of the agent at `address`, as lines for `authorized_keys` (R-SSH-67).
///
/// # Errors
/// There is no agent, or it would not list its keys, said for the user.
pub(super) async fn keys(
    address: Option<&AgentAddress>,
) -> Result<Vec<crate::key_setup::PublicLine>, String> {
    let address = address.ok_or("No agent is running.")?;
    let mut client = connect(address)
        .await
        .map_err(|e| format!("The agent could not be reached: {e}"))?;
    let identities = client
        .request_identities()
        .await
        .map_err(|e| format!("The agent did not list its keys: {e}"))?;
    identities
        .iter()
        .map(|identity| {
            let line = identity
                .public_key()
                .to_openssh()
                .map_err(|e| format!("The agent's key could not be read: {e}"))?;
            crate::key_setup::PublicLine::parse(&line)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_agent_is_said_as_none_not_an_error_r_ssh_13() {
        // The test environment may or may not have an agent; either way there is no panic and a Unix
        // address is a socket.
        match AgentAddress::from_environment() {
            Some(AgentAddress::Socket(path)) => assert!(!path.as_os_str().is_empty()),
            Some(AgentAddress::Windows) => assert_eq!(std::env::consts::FAMILY, "windows"),
            None => {}
        }
    }
}
