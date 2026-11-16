//! Jump hosts (R-SSH-43): OpenSSH's `ProxyJump` and `-J` text, `[user@]host[:port]` separated by commas, each
//! name looked up in `~/.ssh/config` as `ssh -J` does (`ssh_config(5)`, `ProxyJump`).

use std::path::PathBuf;

use super::Target;
use crate::openssh_config::Config;

/// How deep a jump host's own `ProxyJump` is followed before it is taken for a loop.
const MAX_DEPTH: usize = 8;

/// One jump host on the way to the target, in the order they are reached.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Jump {
    pub target: Target,
    /// Key files for this hop: its `IdentityFile`s from the config, then the session's.
    pub key_files: Vec<PathBuf>,
}

/// The jump hosts `spec` names, first one first. A name the config knows takes its `HostName`, `User`, `Port`,
/// `IdentityFile`s and its own `ProxyJump` (reached before it); what is typed (`user@`, `:port`) wins. A hop with
/// no user anywhere signs on as `user`, the session's. `none` or empty text is no jump host.
pub fn resolve(
    spec: &str,
    user: &str,
    key_files: &[PathBuf],
    config: Option<&Config>,
) -> Result<Vec<Jump>, String> {
    let mut jumps = Vec::new();
    add(spec, user, key_files, config, 0, &mut jumps)?;
    Ok(jumps)
}

fn add(
    spec: &str,
    user: &str,
    key_files: &[PathBuf],
    config: Option<&Config>,
    depth: usize,
    jumps: &mut Vec<Jump>,
) -> Result<(), String> {
    if depth > MAX_DEPTH {
        return Err(format!(
            "the jump hosts lead through more than {MAX_DEPTH} levels of ProxyJump; is there a loop?"
        ));
    }
    for hop in parse(spec)? {
        let host = config.map(|c| c.host(&hop.host, user));
        if let Some(inner) = host.as_ref().and_then(|h| h.proxy_jump.as_deref()) {
            add(inner, user, key_files, config, depth + 1, jumps)?;
        }
        let mut keys: Vec<PathBuf> = host
            .as_ref()
            .map(|h| h.identity_files.clone())
            .unwrap_or_default();
        for key in key_files {
            if !keys.contains(key) {
                keys.push(key.clone());
            }
        }
        let target = Target {
            host: host
                .as_ref()
                .map_or_else(|| hop.host.clone(), |h| h.host_name.clone()),
            port: hop
                .port
                .or_else(|| host.as_ref().and_then(|h| h.port))
                .unwrap_or(22),
            user: hop
                .user
                .or_else(|| host.as_ref().and_then(|h| h.user.clone()))
                .unwrap_or_else(|| user.to_owned()),
        };
        jumps.push(Jump {
            target,
            key_files: keys,
        });
    }
    Ok(())
}

/// One hop as typed.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Hop {
    user: Option<String>,
    host: String,
    port: Option<u16>,
}

/// Checks `spec` without the config: the message says what is wrong, for the session editor and the command line.
pub fn check(spec: &str) -> Result<(), String> {
    parse(spec).map(|_| ())
}

/// `[user@]host[:port]` (or `ssh://` before it, `[…]` around an IPv6 address), separated by commas.
fn parse(spec: &str) -> Result<Vec<Hop>, String> {
    let spec = spec.trim();
    if spec.is_empty() || spec.eq_ignore_ascii_case("none") {
        return Ok(Vec::new());
    }
    spec.split(',').map(|hop| parse_hop(hop.trim())).collect()
}

fn parse_hop(text: &str) -> Result<Hop, String> {
    let bare = text.strip_prefix("ssh://").unwrap_or(text);
    let bare = bare.strip_suffix('/').unwrap_or(bare);
    let (user, address) = match bare.rsplit_once('@') {
        Some((user, address)) if !user.is_empty() => (Some(user.to_owned()), address),
        Some(_) => return Err(format!("\"{text}\" has no user name before the @")),
        None => (None, bare),
    };
    let (host, port) = if let Some(rest) = address.strip_prefix('[') {
        let (host, after) = rest
            .split_once(']')
            .ok_or_else(|| format!("\"{text}\" has a [ without its ]"))?;
        match after {
            "" => (host, None),
            _ => match after.strip_prefix(':') {
                Some(port) => (host, Some(port)),
                None => return Err(format!("\"{text}\" has something after the ]")),
            },
        }
    } else {
        match address.split_once(':') {
            // More than one colon: an IPv6 address without a port.
            Some((host, port)) if !port.contains(':') => (host, Some(port)),
            _ => (address, None),
        }
    };
    if host.is_empty() {
        return Err(format!("\"{text}\" names no host"));
    }
    if host.contains(char::is_whitespace) {
        return Err(format!(
            "\"{text}\" has a space in the host name; separate jump hosts with commas"
        ));
    }
    let port = port
        .map(|p| {
            p.parse::<u16>()
                .ok()
                .filter(|&p| p != 0)
                .ok_or_else(|| format!("\"{p}\" in \"{text}\" is not a port (1–65535)"))
        })
        .transpose()?;
    Ok(Hop {
        user,
        host: host.to_owned(),
        port,
    })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn target(user: &str, host: &str, port: u16) -> Target {
        Target {
            host: host.into(),
            port,
            user: user.into(),
        }
    }

    fn targets(spec: &str, config: Option<&Config>) -> Vec<Target> {
        resolve(spec, "me", &[], config)
            .expect("test setup")
            .into_iter()
            .map(|j| j.target)
            .collect()
    }

    #[test]
    fn jump_hosts_are_read_as_openssh_writes_them_r_ssh_43() {
        assert_eq!(
            targets(
                "bastion, ops@gw.example.com:2222,ssh://u@[fe80::1]:23/",
                None
            ),
            [
                target("me", "bastion", 22),
                target("ops", "gw.example.com", 2222),
                target("u", "fe80::1", 23),
            ]
        );
        assert_eq!(targets("::1", None), [target("me", "::1", 22)]);
        assert_eq!(targets("[::1]", None), [target("me", "::1", 22)]);
        assert_eq!(targets("a@b@host", None), [target("a@b", "host", 22)]);
        assert_eq!(targets("bøssa.dk", None), [target("me", "bøssa.dk", 22)]);
    }

    #[test]
    fn none_and_empty_text_are_no_jump_hosts_r_ssh_43() {
        assert_eq!(targets("", None), []);
        assert_eq!(targets("  None ", None), []);
    }

    #[test]
    fn a_jump_host_that_cannot_be_read_says_what_is_wrong_r_ssh_43() {
        for (spec, said) in [
            ("a,,b", "names no host"),
            ("@host", "no user name"),
            ("host:0", "not a port"),
            ("host:99999", "not a port"),
            ("host:x", "not a port"),
            ("[::1", "without its ]"),
            ("[::1]x", "after the ]"),
            ("one two", "separate jump hosts with commas"),
            ("user@:22", "names no host"),
        ] {
            let error = check(spec).expect_err("not readable");
            assert!(error.contains(said), "{spec}: {error}");
        }
    }

    #[test]
    fn a_jump_host_named_in_the_config_takes_its_settings_and_its_own_jumps_r_ssh_43() {
        let dir = std::env::temp_dir().join(format!("parley-ssh-jump-{}", std::process::id()));
        fs::create_dir_all(&dir).expect("test setup");
        let file = dir.join("config");
        fs::write(
            &file,
            "Host bastion\n  HostName b.example.com\n  User ops\n  Port 2200\n  IdentityFile /k/bastion\n  ProxyJump outer\nHost outer\n  HostName o.example.com\n",
        )
        .expect("test setup");
        let config = Config::read(&file, &dir);
        let session_key = PathBuf::from("/k/session");
        let jumps = resolve(
            "bastion:2201",
            "me",
            std::slice::from_ref(&session_key),
            Some(&config),
        )
        .expect("test setup");
        assert_eq!(jumps.len(), 2);
        assert_eq!(jumps[0].target, target("me", "o.example.com", 22));
        assert_eq!(jumps[0].key_files, std::slice::from_ref(&session_key));
        assert_eq!(jumps[1].target, target("ops", "b.example.com", 2201));
        assert_eq!(
            jumps[1].key_files,
            [PathBuf::from("/k/bastion"), session_key]
        );
        fs::remove_dir_all(&dir).expect("test setup");
    }

    #[test]
    fn a_proxy_jump_loop_in_the_config_is_said_not_followed_forever_r_ssh_43() {
        let dir = std::env::temp_dir().join(format!("parley-ssh-jump-loop-{}", std::process::id()));
        fs::create_dir_all(&dir).expect("test setup");
        let file = dir.join("config");
        fs::write(&file, "Host a\n  ProxyJump b\nHost b\n  ProxyJump a\n").expect("test setup");
        let config = Config::read(&file, &dir);
        let error = resolve("a", "me", &[], Some(&config)).expect_err("not readable");
        assert!(error.contains("loop"), "{error}");
        fs::remove_dir_all(&dir).expect("test setup");
    }
}
