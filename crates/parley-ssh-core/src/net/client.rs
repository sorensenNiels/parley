//! russh's client handler: the algorithms offered (R-SSH-2), the host-key check (R-SSH-3) and the banner (R-SSH-4).

use std::borrow::Cow;

use russh::client::{self as ssh, Session};
use russh::keys::PublicKeyOrCertificate;
use russh::keys::ssh_key::{Algorithm, EcdsaCurve, HashAlg, PublicKey};
use russh::{Preferred, cipher, kex, mac};

use super::{
    Algorithms, Command, Commands, Event, HostKeyAnswer, HostKeyQuestion, Link, Target, wait_for,
};
use crate::Error;
use crate::known_hosts::{self, HostKeyStatus, KnownHosts};

/// Key exchanges offered, best first (R-SSH-2). The two extensions ask for RFC 8308 extension info and for
/// OpenSSH's strict key exchange, the fix for Terrapin (CVE-2023-48795).
const KEX: &[kex::Name] = &[
    kex::CURVE25519,
    kex::CURVE25519_PRE_RFC_8731,
    kex::ECDH_SHA2_NISTP256,
    kex::ECDH_SHA2_NISTP384,
    kex::ECDH_SHA2_NISTP521,
    kex::DH_G16_SHA512,
    kex::DH_G14_SHA256,
    kex::EXTENSION_SUPPORT_AS_CLIENT,
    kex::EXTENSION_OPENSSH_STRICT_KEX_AS_CLIENT,
];

const HOST_KEYS: &[Algorithm] = &[
    Algorithm::Ed25519,
    Algorithm::Ecdsa {
        curve: EcdsaCurve::NistP256,
    },
    Algorithm::Ecdsa {
        curve: EcdsaCurve::NistP384,
    },
    Algorithm::Ecdsa {
        curve: EcdsaCurve::NistP521,
    },
    Algorithm::Rsa {
        hash: Some(HashAlg::Sha512),
    },
    Algorithm::Rsa {
        hash: Some(HashAlg::Sha256),
    },
];

const CIPHERS: &[cipher::Name] = &[
    cipher::CHACHA20_POLY1305,
    cipher::AES_256_GCM,
    cipher::AES_128_GCM,
    cipher::AES_256_CTR,
    cipher::AES_128_CTR,
];

const MACS: &[mac::Name] = &[
    mac::HMAC_SHA256_ETM,
    mac::HMAC_SHA512_ETM,
    mac::HMAC_SHA256,
    mac::HMAC_SHA512,
];

/// The client's configuration: R-SSH-2's algorithms, Nagle off for typing.
pub(super) fn config() -> ssh::Config {
    ssh::Config {
        client_id: russh::SshId::Standard(Cow::Owned(format!(
            "SSH-2.0-ParleySSH_{}",
            env!("CARGO_PKG_VERSION")
        ))),
        preferred: Preferred {
            kex: Cow::Borrowed(KEX),
            key: Cow::Borrowed(HOST_KEYS),
            cipher: Cow::Borrowed(CIPHERS),
            mac: Cow::Borrowed(MACS),
            ..Preferred::DEFAULT
        },
        nodelay: true,
        ..ssh::Config::default()
    }
}

pub(super) struct Client {
    host: String,
    port: u16,
    known_hosts: KnownHosts,
    commands: Commands,
    link: Link,
    fingerprint: String,
}

impl Client {
    pub(super) fn new(
        target: &Target,
        known_hosts: KnownHosts,
        commands: Commands,
        link: Link,
    ) -> Self {
        Self {
            host: target.host.clone(),
            port: target.port,
            known_hosts,
            commands,
            link,
            fingerprint: String::new(),
        }
    }

    /// Asks the user about a key that is not known, and saves it if they say so.
    async fn ask(&self, key: &PublicKey, status: HostKeyStatus) -> Result<bool, Error> {
        let question = HostKeyQuestion {
            host: self.host.clone(),
            port: self.port,
            key_type: known_hosts::key_type(key),
            fingerprint: known_hosts::fingerprint(key),
            status,
        };
        self.link.send(Event::HostKey(question));
        let answer = wait_for(&self.commands, |command| match command {
            Command::HostKey(answer) => Some(answer),
            _ => None,
        })
        .await?;
        match answer {
            HostKeyAnswer::AcceptAndSave => {
                self.known_hosts
                    .learn(&self.host, self.port, key)
                    .map_err(Error::SaveHostKey)?;
                self.link.log(format!(
                    "Host key saved in {}",
                    self.known_hosts.own_file().display()
                ));
                Ok(true)
            }
            HostKeyAnswer::ConnectOnce => {
                self.link.log("Host key accepted for this connection only");
                Ok(true)
            }
            HostKeyAnswer::Cancel => Err(Error::HostKeyRejected),
        }
    }
}

impl ssh::Handler for Client {
    type Error = Error;

    async fn check_server_key(
        &mut self,
        server_key: &PublicKeyOrCertificate,
    ) -> Result<bool, Error> {
        // Certificates are not offered (Preferred::host_key_certificates is empty), so a server cannot send one.
        let PublicKeyOrCertificate::PublicKey { key, .. } = server_key else {
            return Err(Error::HostKeyRejected);
        };
        self.fingerprint = known_hosts::fingerprint(key);
        self.link.log(format!(
            "Host key {} {}",
            known_hosts::key_type(key),
            self.fingerprint
        ));
        match self.known_hosts.check(&self.host, self.port, key) {
            HostKeyStatus::Known => {
                self.link.log("Host key matches the saved one");
                Ok(true)
            }
            HostKeyStatus::Revoked => Err(Error::HostKeyRevoked),
            status @ (HostKeyStatus::Unknown | HostKeyStatus::Changed { .. }) => {
                self.ask(key, status).await
            }
        }
    }

    #[expect(
        clippy::unused_async_trait_impl,
        reason = "a method of russh's async Handler trait"
    )]
    async fn kex_done(
        &mut self,
        _shared_secret: Option<&[u8]>,
        names: &russh::Names,
        _session: &mut Session,
    ) -> Result<(), Error> {
        let algorithms = Algorithms {
            kex: names.kex.as_ref().to_owned(),
            host_key: names.key.as_str().to_owned(),
            cipher: names.cipher.as_ref().to_owned(),
            mac: names.client_mac.as_ref().to_owned(),
        };
        self.link.log(format!(
            "Key exchange {}, host key {}, cipher {}, MAC {}",
            algorithms.kex, algorithms.host_key, algorithms.cipher, algorithms.mac
        ));
        self.link.send(Event::Secured {
            algorithms,
            fingerprint: self.fingerprint.clone(),
        });
        Ok(())
    }

    #[expect(
        clippy::unused_async_trait_impl,
        reason = "a method of russh's async Handler trait"
    )]
    async fn auth_banner(&mut self, banner: &str, _session: &mut Session) -> Result<(), Error> {
        self.link.send(Event::Banner(banner.to_owned()));
        Ok(())
    }
}
