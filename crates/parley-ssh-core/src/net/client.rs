//! russh's client handler: the algorithms offered (R-SSH-2), the host-key check (R-SSH-3) and the banner (R-SSH-4).

use std::borrow::Cow;

use russh::client::{self as ssh, Session};
use russh::keys::PublicKeyOrCertificate;
use russh::keys::ssh_key::{Algorithm, EcdsaCurve, HashAlg, PublicKey};
use russh::{Preferred, cipher, kex, mac};

use super::agent::{self, AgentAddress};
use super::{
    Algorithms, Command, Commands, Event, Hop, HostKeyAnswer, HostKeyQuestion, Link, Target,
    wait_for,
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

/// The client's configuration: R-SSH-2's algorithms, Nagle off for typing, and the keepalive, rekeying and
/// compression of `transport` (R-SSH-26, R-SSH-27).
pub(super) fn config(transport: &super::Transport) -> ssh::Config {
    // russh refuses limits above 1 GiB (a nonce must never repeat).
    let bytes = transport.rekey_bytes.min(1 << 30);
    let compression: &'static [russh::compression::Name] = if transport.compression {
        &[
            russh::compression::ZLIB_LEGACY,
            russh::compression::ZLIB,
            russh::compression::NONE,
        ]
    } else {
        &[russh::compression::NONE]
    };
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
            compression: Cow::Borrowed(compression),
            ..Preferred::DEFAULT
        },
        limits: russh::Limits::new(bytes, bytes, transport.rekey_after),
        keepalive_interval: transport.keepalive,
        keepalive_max: 3,
        nodelay: true,
        ..ssh::Config::default()
    }
}

pub(super) struct Client {
    host: String,
    port: u16,
    known_hosts: KnownHosts,
    /// The agent the server may use while the gate is open: forwarding on (R-SSH-13), or a job that asked
    /// for it running (D58).
    forward_to: Option<AgentAddress>,
    gate: super::jobs::AgentGate,
    hop: Hop,
    commands: Commands,
    link: Link,
    fingerprint: String,
    /// The host key, as an OpenSSH line (R-SSH-53).
    host_key: String,
}

impl Client {
    pub(super) fn new(
        target: &Target,
        known_hosts: KnownHosts,
        (forward_to, gate): (Option<AgentAddress>, super::jobs::AgentGate),
        hop: Hop,
        commands: Commands,
        link: Link,
    ) -> Self {
        Self {
            host: target.host.clone(),
            port: target.port,
            known_hosts,
            forward_to,
            gate,
            hop,
            commands,
            link,
            fingerprint: String::new(),
            host_key: String::new(),
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
        // An empty line only makes a later transfer to this host fail, saying why.
        self.host_key = key.to_openssh().unwrap_or_default();
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
        match self.hop {
            Hop::Target => self.link.send(Event::Secured {
                algorithms,
                fingerprint: self.fingerprint.clone(),
                host_key: self.host_key.clone(),
            }),
            // A jump host's are in the event log only (R-SSH-43).
            Hop::Jump => {}
        }
        Ok(())
    }

    /// The server opens a channel to the agent (R-SSH-13): only while the gate is open (D58), and each one is
    /// carried to the user's agent until either side closes it.
    async fn server_channel_open_agent_forward(
        &mut self,
        channel: russh::Channel<ssh::Msg>,
        reply: ssh::ChannelOpenHandle,
        _session: &mut Session,
    ) -> Result<(), Error> {
        let Some(address) = self.forward_to.clone().filter(|_| self.gate.open()) else {
            reply
                .reject(russh::ChannelOpenFailure::AdministrativelyProhibited)
                .await;
            self.link
                .log("Refused the server's use of the agent: forwarding is off and no transfer is running");
            return Ok(());
        };
        reply.accept().await;
        self.link.log("The server uses the agent");
        let link = self.link.clone();
        tokio::spawn(async move {
            match agent::connect(&address).await {
                Ok(client) => {
                    let mut agent = client.into_inner();
                    let mut channel = channel.into_stream();
                    // Either side closing ends the relay; how is of no interest.
                    let _ = tokio::io::copy_bidirectional(&mut channel, &mut agent).await;
                }
                Err(e) => link.log(format!(
                    "The server asked for the agent, which did not answer: {e}"
                )),
            }
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

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn keepalive_rekeying_and_compression_reach_russh_r_ssh_26_r_ssh_27() {
        let plain = config(&super::super::Transport::default());
        assert_eq!(plain.keepalive_interval, None);
        assert_eq!(
            plain.preferred.compression.as_ref(),
            [russh::compression::NONE]
        );
        let transport = super::super::Transport {
            keepalive: Some(Duration::from_secs(30)),
            rekey_after: Duration::from_secs(600),
            rekey_bytes: usize::MAX,
            compression: true,
            ..super::super::Transport::default()
        };
        let set = config(&transport);
        assert_eq!(set.keepalive_interval, Some(Duration::from_secs(30)));
        assert_eq!(set.keepalive_max, 3, "three unanswered end the connection");
        assert_eq!(set.limits.rekey_time_limit, Duration::from_secs(600));
        assert_eq!(
            set.limits.rekey_write_limit,
            1 << 30,
            "at most what russh allows"
        );
        assert_eq!(
            set.preferred.compression[0],
            russh::compression::ZLIB_LEGACY
        );
        assert!(set.nodelay);
    }
}
