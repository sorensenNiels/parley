//! The hosts of OpenSSH's config on the start screen and the command line (R-SSH-42): read through
//! `parley-ssh-core`'s `openssh_config`, read again when a file changed, and made into sessions with the defaults.

use std::time::{Duration, Instant};

use parley_ssh_core::openssh_config::{self, Config, Host};

use crate::sessions::SavedSession;

/// How often the files are looked at while a start screen shows.
const CHECK_EVERY: Duration = Duration::from_secs(1);

/// OpenSSH's config and its hosts, as OpenSSH would use them.
#[derive(Debug, Default)]
pub struct Hosts {
    config: Option<Config>,
    resolved: Vec<Host>,
    checked: Option<Instant>,
}

impl Hosts {
    pub fn new(config: Config) -> Self {
        let mut hosts = Self::default();
        hosts.set(config);
        hosts
    }

    fn set(&mut self, config: Config) {
        let user = openssh_config::local_user();
        self.resolved = config
            .hosts()
            .iter()
            .map(|alias| config.host(alias, &user))
            .collect();
        self.config = Some(config);
    }

    /// Reads the config again if a file changed; at most once a [`CHECK_EVERY`].
    pub fn refresh(&mut self) {
        let now = Instant::now();
        if self
            .checked
            .is_some_and(|t| now.duration_since(t) < CHECK_EVERY)
        {
            return;
        }
        self.checked = Some(now);
        if let Some(config) = self.config.as_ref().filter(|c| c.changed()) {
            let config = config.reread();
            self.set(config);
        }
    }

    pub fn hosts(&self) -> &[Host] {
        &self.resolved
    }

    /// The lines of the config that were skipped.
    pub fn problems(&self) -> &[String] {
        self.config.as_ref().map_or(&[], Config::problems)
    }

    /// What OpenSSH would use for `name`, which need not be listed (`Host *.lan` applies to `box.lan`).
    pub fn lookup(&self, name: &str) -> Option<Host> {
        let config = self.config.as_ref()?;
        Some(config.host(name, &openssh_config::local_user()))
    }
}

/// A session for a host of the config, named by its alias: what the config leaves out from `defaults`, the user
/// name last from the local one (R-SSH-42).
pub fn session(defaults: &SavedSession, host: &Host) -> SavedSession {
    let mut session = defaults.clone();
    host.alias.clone_into(&mut session.name);
    apply(&mut session, host);
    if session.user.trim().is_empty() {
        session.user = openssh_config::local_user();
    }
    session
}

/// The config's settings for `host` over `session`'s: `HostName`, `User`, `Port`, the `IdentityFile`s before the
/// session's key files, `ForwardAgent`.
pub fn apply(session: &mut SavedSession, host: &Host) {
    host.host_name.clone_into(&mut session.host);
    if let Some(user) = &host.user {
        user.clone_into(&mut session.user);
    }
    if let Some(port) = host.port {
        session.port = port;
    }
    let mut keys: Vec<String> = host
        .identity_files
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    for key in session.key_files.drain(..) {
        if !keys.contains(&key) {
            keys.push(key);
        }
    }
    session.key_files = keys;
    if let Some(forward) = host.forward_agent {
        session.agent_forwarding = forward;
    }
    if let Some(jumps) = &host.proxy_jump {
        jumps.clone_into(&mut session.proxy_jump);
    }
    apply_connection(session, host);
}

/// `RemoteCommand`, `SetEnv`, `AddressFamily`, `Compression` and `ProxyCommand` (R-SSH-26..29).
fn apply_connection(session: &mut SavedSession, host: &Host) {
    use crate::sessions::connection::{Family, ProxyKind};
    if let Some(command) = &host.remote_command {
        command.clone_into(&mut session.remote_command);
    }
    if !host.set_env.is_empty() {
        session.environment = host.set_env.join("\n");
    }
    if let Some(family) = host.address_family {
        session.address_family = match family {
            parley_ssh_core::net::AddressFamily::Any => Family::Any,
            parley_ssh_core::net::AddressFamily::V4 => Family::V4,
            parley_ssh_core::net::AddressFamily::V6 => Family::V6,
        };
    }
    if let Some(on) = host.compression {
        session.compression = on;
    }
    if let Some(command) = &host.proxy_command {
        session.proxy = ProxyKind::Command;
        command.clone_into(&mut session.proxy_command);
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn a_config_host_takes_what_it_leaves_out_from_the_defaults_r_ssh_42() {
        let defaults = SavedSession {
            user: "dflt".into(),
            key_files: vec!["/k/default".into()],
            agent_forwarding: false,
            mouse_reporting: false,
            ..SavedSession::default()
        };
        let host = Host {
            alias: "web".into(),
            host_name: "web1.lan".into(),
            port: Some(2222),
            identity_files: vec![PathBuf::from("/k/web")],
            forward_agent: Some(true),
            proxy_jump: Some("bastion".into()),
            ..Host::default()
        };
        let session = session(&defaults, &host);
        assert_eq!(session.proxy_jump, "bastion", "R-SSH-43");
        let connection = Host {
            remote_command: Some("tmux attach".into()),
            set_env: vec!["A=1".into(), "B=æ".into()],
            address_family: Some(parley_ssh_core::net::AddressFamily::V4),
            compression: Some(true),
            proxy_command: Some("nc %h %p".into()),
            ..host.clone()
        };
        let s = super::session(&defaults, &connection);
        assert_eq!(
            (
                s.remote_command.as_str(),
                s.environment.as_str(),
                s.proxy_command.as_str()
            ),
            ("tmux attach", "A=1\nB=æ", "nc %h %p"),
            "R-SSH-28, R-SSH-29"
        );
        assert!(s.compression && s.address_family == crate::sessions::connection::Family::V4);
        assert_eq!(s.proxy, crate::sessions::connection::ProxyKind::Command);
        assert_eq!(session.name, "web");
        assert_eq!(session.host, "web1.lan");
        assert_eq!(session.user, "dflt");
        assert_eq!(session.port, 2222);
        assert_eq!(session.key_files, ["/k/web", "/k/default"]);
        assert!(session.agent_forwarding);
        assert!(!session.mouse_reporting);
    }

    #[test]
    fn with_no_user_anywhere_the_local_one_is_used() {
        let host = Host {
            alias: "x".into(),
            host_name: "x".into(),
            ..Host::default()
        };
        let session = session(&SavedSession::default(), &host);
        assert_eq!(session.user, openssh_config::local_user());
        assert_eq!(session.port, 22);
    }
}
