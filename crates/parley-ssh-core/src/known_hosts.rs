//! Host keys (R-SSH-3; RFC 4251 §4.1; OpenSSH's sshd(8) "`SSH_KNOWN_HOSTS` FILE FORMAT").
//!
//! Known keys are read from Parley SSH's own file and from OpenSSH's `~/.ssh/known_hosts`. New keys are written
//! to Parley SSH's own file only; OpenSSH's is never changed. When Parley SSH's file names the host, it alone
//! decides, so a key the user accepted there wins over a stale one in OpenSSH's file.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use data_encoding::BASE64;
use hmac::{Hmac, KeyInit, Mac};
use russh::keys::ssh_key::{HashAlg, PublicKey};
use sha1::Sha1;

/// What the files say about a server's key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostKeyStatus {
    /// The key is saved for this host.
    Known,
    /// Nothing is saved for this host and key type.
    Unknown,
    /// Another key of the same type is saved for this host: someone may be in between (R-SSH-3).
    Changed { saved: Vec<SavedKey> },
    /// The key is marked `@revoked`.
    Revoked,
}

/// A key saved in a known-hosts file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedKey {
    pub file: PathBuf,
    /// One-based line number.
    pub line: usize,
    pub fingerprint: String,
}

impl SavedKey {
    /// Is it in Parley SSH's own file, so the user can remove it from the dialog?
    pub fn removable(&self, known: &KnownHosts) -> bool {
        self.file == known.own
    }
}

/// The key's type and SHA-256 fingerprint as OpenSSH shows them (`SHA256:…`).
pub fn fingerprint(key: &PublicKey) -> String {
    key.fingerprint(HashAlg::Sha256).to_string()
}

/// The key's type as its algorithm name (`ssh-ed25519`).
pub fn key_type(key: &PublicKey) -> String {
    key.algorithm().as_str().to_owned()
}

/// The known-hosts files Parley SSH reads and the one it writes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnownHosts {
    own: PathBuf,
    openssh: Option<PathBuf>,
}

impl KnownHosts {
    /// `own` is Parley SSH's file; `openssh` OpenSSH's, if there is a home folder.
    pub fn new(own: PathBuf, openssh: Option<PathBuf>) -> Self {
        Self { own, openssh }
    }

    /// The usual files: `known_hosts` in `folder` (Parley SSH's) and `~/.ssh/known_hosts`.
    pub fn in_folder(folder: &Path) -> Self {
        let openssh = std::env::home_dir().map(|home| home.join(".ssh").join("known_hosts"));
        Self::new(folder.join("known_hosts"), openssh)
    }

    pub fn own_file(&self) -> &Path {
        &self.own
    }

    /// Checks `key` for `host` and `port`. A file that cannot be read counts as empty.
    pub fn check(&self, host: &str, port: u16, key: &PublicKey) -> HostKeyStatus {
        let own = status_in(&self.own, host, port, key);
        if own.named_host {
            return own.status;
        }
        match &self.openssh {
            Some(path) => status_in(path, host, port, key).status,
            None => HostKeyStatus::Unknown,
        }
    }

    /// Saves `key` for `host` and `port` in Parley SSH's file, creating it and its folder if needed.
    pub fn learn(&self, host: &str, port: u16, key: &PublicKey) -> io::Result<()> {
        if let Some(folder) = self.own.parent() {
            fs::create_dir_all(folder)?;
        }
        let line = format!(
            "{} {}\n",
            host_pattern(host, port),
            key.to_openssh().map_err(io::Error::other)?
        );
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.own)?;
        file.write_all(line.as_bytes())
    }

    /// Removes every key saved for `host` and `port` from Parley SSH's file (the explicit step of R-SSH-3).
    /// Returns how many lines went. Lines that name other hosts too lose only this host's name.
    pub fn forget(&self, host: &str, port: u16) -> io::Result<usize> {
        let text = match fs::read_to_string(&self.own) {
            Ok(text) => text,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(0),
            Err(e) => return Err(e),
        };
        let target = host_pattern(host, port);
        let mut removed = 0;
        let mut kept = String::with_capacity(text.len());
        for line in text.lines() {
            match parse(line) {
                Some(entry) if entry.marker.is_none() && matches_host(entry.hosts, &target) => {
                    removed += 1;
                    let others: Vec<&str> = entry
                        .hosts
                        .split(',')
                        .filter(|p| !pattern_matches(p, &target))
                        .collect();
                    if !others.is_empty() {
                        kept.push_str(&others.join(","));
                        kept.push(' ');
                        kept.push_str(entry.key_text);
                        kept.push('\n');
                    }
                }
                _ => {
                    kept.push_str(line);
                    kept.push('\n');
                }
            }
        }
        fs::write(&self.own, kept)?;
        Ok(removed)
    }
}

/// The host as known-hosts files write it: `host`, or `[host]:port` for a port other than 22.
fn host_pattern(host: &str, port: u16) -> String {
    if port == 22 {
        host.to_owned()
    } else {
        format!("[{host}]:{port}")
    }
}

struct FileStatus {
    status: HostKeyStatus,
    /// Some line names the host (with any key type).
    named_host: bool,
}

fn status_in(path: &Path, host: &str, port: u16, key: &PublicKey) -> FileStatus {
    // A missing or unreadable file holds nothing.
    let text = fs::read_to_string(path).unwrap_or_default();
    let (status, named_host) = match status_in_text(&text, host, port, key) {
        TextStatus::Known => (HostKeyStatus::Known, true),
        TextStatus::Unknown { named_host } => (HostKeyStatus::Unknown, named_host),
        TextStatus::Changed(lines) => {
            let saved = lines
                .into_iter()
                .map(|(line, fingerprint)| SavedKey {
                    file: path.to_owned(),
                    line,
                    fingerprint,
                })
                .collect();
            (HostKeyStatus::Changed { saved }, true)
        }
        TextStatus::Revoked => (HostKeyStatus::Revoked, true),
    };
    FileStatus { status, named_host }
}

#[derive(Debug, PartialEq, Eq)]
enum TextStatus {
    Known,
    Unknown { named_host: bool },
    Changed(Vec<(usize, String)>),
    Revoked,
}

/// The rules of sshd(8): a revoked key is refused; a matching key is known; another key of the same type for the
/// host means it changed; anything else is unknown.
fn status_in_text(text: &str, host: &str, port: u16, key: &PublicKey) -> TextStatus {
    let target = host_pattern(host, port);
    let mut named_host = false;
    let mut known = false;
    let mut changed = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let Some(entry) = parse(line) else { continue };
        let Ok(saved) = PublicKey::from_openssh(entry.key_text) else {
            continue;
        };
        match entry.marker {
            Some(Marker::Revoked) => {
                if saved.key_data() == key.key_data() {
                    return TextStatus::Revoked;
                }
                continue;
            }
            Some(Marker::CertAuthority) => continue,
            None => {}
        }
        if !matches_host(entry.hosts, &target) {
            continue;
        }
        named_host = true;
        if saved.key_data() == key.key_data() {
            known = true;
        } else if saved.algorithm() == key.algorithm() {
            changed.push((index + 1, fingerprint(&saved)));
        }
    }
    if known {
        TextStatus::Known
    } else if changed.is_empty() {
        TextStatus::Unknown { named_host }
    } else {
        TextStatus::Changed(changed)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Marker {
    CertAuthority,
    Revoked,
}

struct Entry<'a> {
    marker: Option<Marker>,
    hosts: &'a str,
    /// `type base64 [comment]`.
    key_text: &'a str,
}

/// One line: `[@marker] hosts type base64 [comment]`; comments and blank lines give `None`.
fn parse(line: &str) -> Option<Entry<'_>> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let (first, rest) = line.split_once(char::is_whitespace)?;
    let (marker, hosts, rest) = match first {
        "@revoked" | "@cert-authority" => {
            let marker = if first == "@revoked" {
                Marker::Revoked
            } else {
                Marker::CertAuthority
            };
            let (hosts, rest) = rest.trim_start().split_once(char::is_whitespace)?;
            (Some(marker), hosts, rest)
        }
        _ => (None, first, rest),
    };
    Some(Entry {
        marker,
        hosts,
        key_text: rest.trim(),
    })
}

/// Do the comma-separated `patterns` name `target`? A matching `!pattern` rules it out.
fn matches_host(patterns: &str, target: &str) -> bool {
    let mut matched = false;
    for pattern in patterns.split(',') {
        if let Some(negated) = pattern.strip_prefix('!') {
            if pattern_matches(negated, target) {
                return false;
            }
        } else if pattern_matches(pattern, target) {
            matched = true;
        }
    }
    matched
}

/// One pattern: hashed (`|1|salt|hash`, HMAC-SHA1 of the host), with `*` and `?` wildcards, or plain. Host names
/// compare without regard to case.
fn pattern_matches(pattern: &str, target: &str) -> bool {
    if let Some(hashed) = pattern.strip_prefix("|1|") {
        let Some((salt, hash)) = hashed.split_once('|') else {
            return false;
        };
        let (Ok(salt), Ok(hash)) = (
            BASE64.decode(salt.as_bytes()),
            BASE64.decode(hash.as_bytes()),
        ) else {
            return false;
        };
        let Ok(mac) = Hmac::<Sha1>::new_from_slice(&salt) else {
            return false;
        };
        return mac
            .chain_update(target.as_bytes())
            .verify_slice(&hash)
            .is_ok();
    }
    wildcard(&pattern.to_lowercase(), &target.to_lowercase())
}

/// `*` any run of characters, `?` one character.
fn wildcard(pattern: &str, text: &str) -> bool {
    let pattern: Vec<char> = pattern.chars().collect();
    let text: Vec<char> = text.chars().collect();
    let (mut p, mut t) = (0, 0);
    let mut star: Option<(usize, usize)> = None;
    while t < text.len() {
        match pattern.get(p) {
            Some('*') => {
                star = Some((p, t));
                p += 1;
            }
            Some(&c) if c == '?' || c == text[t] => {
                p += 1;
                t += 1;
            }
            _ => match star {
                Some((sp, st)) => {
                    p = sp + 1;
                    t = st + 1;
                    star = Some((sp, st + 1));
                }
                None => return false,
            },
        }
    }
    pattern[p..].iter().all(|&c| c == '*')
}

#[cfg(test)]
mod tests {
    use super::*;

    const ED25519_A: &str =
        "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIJ716A+0k496dPNeQhXQ4B0zYVVNT46pAYePSB/EGfUK";
    const ED25519_B: &str =
        "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIAV5CINKVHRsJdOO4vr5nwVr0ASkuG8DODBufVj2pCQ9";
    const ECDSA: &str = "ecdsa-sha2-nistp256 AAAAE2VjZHNhLXNoYTItbmlzdHAyNTYAAAAIbmlzdHAyNTYAAABBBAjXmmJ2dGLGWofKJj8ksOx0axFBn5/Xlo3yfdCKNr5wRGxx09WL86kgTR2IdJp2BLK8SA9UsFGvwrBgQ8vxF8Y=";

    fn key(text: &str) -> PublicKey {
        PublicKey::from_openssh(text).expect("test key parses")
    }

    fn status(text: &str, host: &str, port: u16, k: &str) -> TextStatus {
        status_in_text(text, host, port, &key(k))
    }

    #[test]
    fn a_saved_key_is_known_and_another_of_the_same_type_has_changed_r_ssh_3() {
        let file = format!("example.com {ED25519_A}\n");
        assert_eq!(
            status(&file, "example.com", 22, ED25519_A),
            TextStatus::Known
        );
        let TextStatus::Changed(lines) = status(&file, "example.com", 22, ED25519_B) else {
            panic!("changed")
        };
        assert_eq!(lines, [(1, fingerprint(&key(ED25519_A)))]);
    }

    #[test]
    fn a_host_with_only_another_key_type_saved_is_unknown_not_changed_r_ssh_3() {
        let file = format!("example.com {ECDSA}\n");
        assert_eq!(
            status(&file, "example.com", 22, ED25519_A),
            TextStatus::Unknown { named_host: true }
        );
        assert_eq!(
            status(&file, "other.com", 22, ED25519_A),
            TextStatus::Unknown { named_host: false }
        );
    }

    #[test]
    fn a_port_other_than_22_is_written_in_brackets() {
        let file = format!("[example.com]:2222 {ED25519_A}\n");
        assert_eq!(
            status(&file, "example.com", 2222, ED25519_A),
            TextStatus::Known
        );
        assert_eq!(
            status(&file, "example.com", 22, ED25519_A),
            TextStatus::Unknown { named_host: false }
        );
    }

    #[test]
    fn hashed_host_names_are_matched_r_ssh_3() {
        // `ssh-keygen -H` style: |1|base64(salt)|base64(HMAC-SHA1(salt, host)).
        let salt = [7u8; 20];
        let mac = Hmac::<Sha1>::new_from_slice(&salt)
            .expect("any key length")
            .chain_update(b"example.com");
        let hash = mac.finalize().into_bytes();
        let file = format!(
            "|1|{}|{} {ED25519_A}\n",
            BASE64.encode(&salt),
            BASE64.encode(&hash)
        );
        assert_eq!(
            status(&file, "example.com", 22, ED25519_A),
            TextStatus::Known
        );
        assert_eq!(
            status(&file, "example.org", 22, ED25519_A),
            TextStatus::Unknown { named_host: false }
        );
    }

    #[test]
    fn several_names_wildcards_negation_and_case_r_ssh_3() {
        let file = format!("alias,Example.COM,10.0.0.1 {ED25519_A}\n*.lan,!secret.lan {ECDSA}\n");
        assert_eq!(
            status(&file, "example.com", 22, ED25519_A),
            TextStatus::Known
        );
        assert_eq!(status(&file, "10.0.0.1", 22, ED25519_A), TextStatus::Known);
        assert_eq!(status(&file, "box.lan", 22, ECDSA), TextStatus::Known);
        assert_eq!(
            status(&file, "secret.lan", 22, ECDSA),
            TextStatus::Unknown { named_host: false }
        );
    }

    #[test]
    fn a_revoked_key_is_refused_for_every_host() {
        let file = format!("@revoked * {ED25519_A}\nexample.com {ED25519_A}\n");
        assert_eq!(
            status(&file, "example.com", 22, ED25519_A),
            TextStatus::Revoked
        );
    }

    #[test]
    fn comments_blank_lines_cert_authorities_and_garbage_are_skipped() {
        let file = format!(
            "# comment\n\n@cert-authority *.com {ED25519_B}\nexample.com ssh-ed25519 not-base64!\nexample.com\næøå\nexample.com {ED25519_A} a comment\n"
        );
        assert_eq!(
            status(&file, "example.com", 22, ED25519_A),
            TextStatus::Known
        );
    }

    fn files(name: &str) -> (KnownHosts, PathBuf) {
        let folder =
            std::env::temp_dir().join(format!("parley-ssh-known-{name}-{}", std::process::id()));
        // A previous run may have left it.
        let _ = fs::remove_dir_all(&folder);
        let openssh = folder.join("openssh_known_hosts");
        (
            KnownHosts::new(
                folder.join("own").join("known_hosts"),
                Some(openssh.clone()),
            ),
            openssh,
        )
    }

    #[test]
    fn learn_writes_parley_sshs_file_and_check_finds_it_r_ssh_3() {
        let (known, _) = files("learn");
        assert_eq!(
            known.check("example.com", 2222, &key(ED25519_A)),
            HostKeyStatus::Unknown
        );
        known
            .learn("example.com", 2222, &key(ED25519_A))
            .expect("learn");
        assert_eq!(
            known.check("example.com", 2222, &key(ED25519_A)),
            HostKeyStatus::Known
        );
        let text = fs::read_to_string(known.own_file()).expect("written");
        assert!(
            text.starts_with("[example.com]:2222 ssh-ed25519 "),
            "{text}"
        );
    }

    #[test]
    fn openssh_keys_count_but_parley_sshs_file_decides_when_it_names_the_host_r_ssh_3() {
        let (known, openssh) = files("precedence");
        fs::create_dir_all(openssh.parent().expect("folder")).expect("folder");
        fs::write(&openssh, format!("example.com {ED25519_A}\n")).expect("write");
        assert_eq!(
            known.check("example.com", 22, &key(ED25519_A)),
            HostKeyStatus::Known
        );
        let HostKeyStatus::Changed { saved } = known.check("example.com", 22, &key(ED25519_B))
        else {
            panic!("changed")
        };
        assert!(
            !saved[0].removable(&known),
            "OpenSSH's file is never changed"
        );
        known
            .learn("example.com", 22, &key(ED25519_B))
            .expect("learn");
        assert_eq!(
            known.check("example.com", 22, &key(ED25519_B)),
            HostKeyStatus::Known
        );
        assert_eq!(
            fs::read_to_string(&openssh).expect("read"),
            format!("example.com {ED25519_A}\n")
        );
    }

    #[test]
    fn forget_removes_only_this_hosts_keys_from_parley_sshs_file_r_ssh_3() {
        let (known, _) = files("forget");
        known
            .learn("example.com", 22, &key(ED25519_A))
            .expect("learn");
        known
            .learn("other.com", 22, &key(ED25519_A))
            .expect("learn");
        fs::write(
            known.own_file(),
            fs::read_to_string(known.own_file()).expect("read")
                + &format!("example.com,alias {ECDSA}\n"),
        )
        .expect("write");
        assert_eq!(known.forget("example.com", 22).expect("forget"), 2);
        let text = fs::read_to_string(known.own_file()).expect("read");
        assert!(!text.contains("example.com"), "{text}");
        assert!(
            text.contains("other.com ") && text.contains("alias ecdsa"),
            "{text}"
        );
        assert_eq!(known.forget("nowhere", 22).expect("forget"), 0);
    }

    #[test]
    fn wildcards_match_runs_and_single_characters() {
        assert!(wildcard("*", ""));
        assert!(wildcard("a*c", "abbbc"));
        assert!(wildcard("a?c", "abc"));
        assert!(!wildcard("a?c", "ac"));
        assert!(wildcard("*.example.com", "x.example.com"));
        assert!(!wildcard("*.example.com", "example.com"));
        assert!(wildcard("æ*", "æøå"));
    }
}
