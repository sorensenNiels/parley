//! Signing on with public keys (R-SSH-12, R-SSH-13; RFC 4252 §7): the running agent's keys first, then each key
//! file. A key file's passphrase is asked each time and never kept; the key itself is dropped once tried.

use std::path::Path;
use std::sync::Arc;

use russh::MethodKind;
use russh::client::{self as ssh, AuthResult};
use russh::keys::ssh_key::{HashAlg, PrivateKey, PublicKey};
use russh::keys::{Algorithm, PrivateKeyWithHashAlg};

use super::client::Client;
use super::sign_on::{Outcome, ask_or_skip};
use super::{Commands, Event, Link, Options, Prompt, PromptField, Target, agent};
use crate::Error;
use crate::known_hosts::fingerprint;

/// How often a wrong passphrase may be typed for one key.
const PASSPHRASE_TRIES: usize = 3;

/// One round of public keys. `Outcome::Failure` carries the methods the server named last.
pub(super) async fn sign_on(
    handle: &mut ssh::Handle<Client>,
    target: &Target,
    options: &Options,
    commands: &Commands,
    link: &Link,
) -> Result<Outcome, Error> {
    // RSA keys sign with SHA-2 when the server says it can (RFC 8332, RFC 8308); `None` asks for nothing better.
    let rsa_hash = handle.best_supported_rsa_hash().await?.flatten();
    // Empty until a key is refused: the earlier methods then stand (`methods_after_failure`), so a round with no
    // keys to try leaves password and keyboard-interactive open.
    let mut remaining = Vec::new();
    if let Some(address) = &options.agent {
        match agent_keys(
            handle,
            target,
            address,
            options.agent_key.as_deref(),
            rsa_hash,
            link,
        )
        .await?
        {
            AgentOutcome::SignedOn => return Ok(Outcome::Success),
            AgentOutcome::Refused(methods) => remaining = methods,
            AgentOutcome::NoKeys => {}
        }
    }
    for path in &options.key_files {
        let Some(key) = load(path, target, commands, link).await? else {
            continue;
        };
        let public = key.public_key().clone();
        let hash = hash_for(&public, rsa_hash);
        let shown = path.display();
        match handle
            .authenticate_publickey(
                target.user.as_str(),
                PrivateKeyWithHashAlg::new(Arc::new(key), hash),
            )
            .await?
        {
            AuthResult::Success => {
                link.log(format!(
                    "Signed on with the key {shown} ({})",
                    fingerprint(&public)
                ));
                return Ok(Outcome::Success);
            }
            AuthResult::Failure {
                remaining_methods, ..
            } => {
                link.log(format!("The server refused the key {shown}"));
                remaining = remaining_methods.to_vec();
            }
        }
    }
    Ok(Outcome::Failure(remaining))
}

enum AgentOutcome {
    SignedOn,
    Refused(Vec<MethodKind>),
    NoKeys,
}

/// Tries the agent's keys, or only the one named; an agent that cannot be reached is said in the log, not an error.
async fn agent_keys(
    handle: &mut ssh::Handle<Client>,
    target: &Target,
    address: &agent::AgentAddress,
    only: Option<&str>,
    rsa_hash: Option<HashAlg>,
    link: &Link,
) -> Result<AgentOutcome, Error> {
    let mut client = match agent::connect(address).await {
        Ok(client) => client,
        Err(e) => {
            link.log(format!("No agent to sign on with: {e}"));
            return Ok(AgentOutcome::NoKeys);
        }
    };
    let identities = match client.request_identities().await {
        Ok(identities) => identities,
        Err(e) => {
            link.log(format!("The agent did not list its keys: {e}"));
            return Ok(AgentOutcome::NoKeys);
        }
    };
    let mut outcome = AgentOutcome::NoKeys;
    for identity in identities {
        let public = identity.public_key().into_owned();
        let print = fingerprint(&public);
        if only.is_some_and(|wanted| wanted != print) {
            continue;
        }
        let hash = hash_for(&public, rsa_hash);
        match handle
            .authenticate_publickey_with(target.user.as_str(), public, hash, &mut client)
            .await
        {
            Ok(AuthResult::Success) => {
                link.log(format!("Signed on with the agent's key {print}"));
                return Ok(AgentOutcome::SignedOn);
            }
            Ok(AuthResult::Failure {
                remaining_methods, ..
            }) => {
                link.log(format!("The server refused the agent's key {print}"));
                outcome = AgentOutcome::Refused(remaining_methods.to_vec());
            }
            Err(e) => link.log(format!("The agent could not sign with {print}: {e}")),
        }
    }
    Ok(outcome)
}

/// SHA-2 for RSA keys as the server allows; other keys have one signature algorithm.
fn hash_for(key: &PublicKey, rsa_hash: Option<HashAlg>) -> Option<HashAlg> {
    matches!(key.algorithm(), Algorithm::Rsa { .. })
        .then_some(rsa_hash)
        .flatten()
}

/// Reads the key file at `path`: OpenSSH's format, PKCS #8 or `PuTTY`'s `.ppk` (versions 2 and 3). An encrypted key
/// asks for its passphrase, up to [`PASSPHRASE_TRIES`] times. A file that cannot be read, or Cancel, skips the key
/// with a word to the user.
async fn load(
    path: &Path,
    target: &Target,
    commands: &Commands,
    link: &Link,
) -> Result<Option<PrivateKey>, Error> {
    let shown = path.display().to_string();
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) => {
            link.send(Event::Problem(format!(
                "The key file {shown} could not be read: {e}"
            )));
            return Ok(None);
        }
    };
    let mut wrong = false;
    match russh::keys::decode_secret_key(&text, None) {
        Ok(key) => return Ok(Some(key)),
        Err(russh::keys::Error::KeyIsEncrypted) => {}
        // PuTTY's encrypted keys say so only once a passphrase is missing from the decoding.
        Err(_) if is_encrypted_ppk(&text) => {}
        Err(e) => {
            link.send(Event::Problem(format!(
                "The key file {shown} is not a key Parley SSH can use: {e}"
            )));
            return Ok(None);
        }
    }
    for _ in 0..PASSPHRASE_TRIES {
        let instructions = if wrong {
            format!("That passphrase did not open {shown}. Try again.")
        } else {
            format!("The key {shown} is protected by a passphrase.")
        };
        let prompt = Prompt {
            host: target.label(),
            name: "Key passphrase".to_owned(),
            instructions,
            fields: vec![PromptField {
                label: "Passphrase".to_owned(),
                echo: false,
            }],
        };
        let Some(answers) = ask_or_skip(prompt, commands, link).await? else {
            link.log(format!("Skipped the key {shown}"));
            return Ok(None);
        };
        let passphrase = answers
            .first()
            .map(crate::Secret::expose)
            .unwrap_or_default();
        match russh::keys::decode_secret_key(&text, Some(passphrase)) {
            Ok(key) => return Ok(Some(key)),
            Err(_) => wrong = true,
        }
    }
    link.send(Event::Problem(format!(
        "The key {shown} was not opened: the passphrase was wrong three times."
    )));
    Ok(None)
}

/// A `.ppk` file whose `Encryption:` line names a cipher.
fn is_encrypted_ppk(text: &str) -> bool {
    text.trim_start().starts_with("PuTTY-User-Key-File-")
        && text.lines().any(|l| {
            l.strip_prefix("Encryption:")
                .is_some_and(|v| v.trim() != "none")
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rsa_keys_sign_with_the_hash_the_server_allows_and_others_with_none_r_ssh_12() {
        let ed25519 = PublicKey::from_openssh(
            "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIJ716A+0k496dPNeQhXQ4B0zYVVNT46pAYePSB/EGfUK",
        )
        .expect("test key");
        assert_eq!(hash_for(&ed25519, Some(HashAlg::Sha512)), None);
    }

    #[test]
    fn an_encrypted_ppk_is_known_by_its_encryption_line() {
        assert!(is_encrypted_ppk(
            "PuTTY-User-Key-File-3: ssh-ed25519\nEncryption: aes256-cbc\n"
        ));
        assert!(!is_encrypted_ppk(
            "PuTTY-User-Key-File-3: ssh-ed25519\nEncryption: none\n"
        ));
        assert!(!is_encrypted_ppk("-----BEGIN OPENSSH PRIVATE KEY-----\n"));
    }
}
