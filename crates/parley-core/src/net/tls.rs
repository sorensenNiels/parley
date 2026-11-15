//! TLS for TN5250 on port 992 (R-NET-2): the operating system's trusted certificates, plus
//! an optional CA certificate file for hosts with a certificate from a company CA.

use std::net::TcpStream;
use std::sync::Arc;
use std::time::Duration;

use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, ServerName};
use rustls::{ClientConfig, ClientConnection, StreamOwned};
use rustls_platform_verifier::Verifier;

use crate::error::{Error, Result};
use crate::session::SessionConfig;

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(15);

pub(super) fn connect(
    config: &SessionConfig,
    mut tcp: TcpStream,
    poll: Duration,
) -> Result<StreamOwned<ClientConnection, TcpStream>> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let ca_file = config.ca_file.trim();
    let verifier = if ca_file.is_empty() {
        Verifier::new(Arc::clone(&provider))
    } else {
        Verifier::new_with_extra_roots(read_ca_file(ca_file)?, Arc::clone(&provider))
    }
    .map_err(|e| Error::Tls(e.to_string()))?;
    let tls_config = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|e| Error::Tls(e.to_string()))?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(verifier))
        .with_no_client_auth();
    let name = ServerName::try_from(config.host.clone())
        .map_err(|_| Error::Tls(format!("'{}' is not a valid host name", config.host)))?;
    let mut connection =
        ClientConnection::new(Arc::new(tls_config), name).map_err(|e| Error::Tls(e.to_string()))?;

    // Handshake with a long timeout, then poll like a plain connection.
    tcp.set_read_timeout(Some(HANDSHAKE_TIMEOUT))?;
    while connection.is_handshaking() {
        connection
            .complete_io(&mut tcp)
            .map_err(|e| handshake_error(&e))?;
    }
    tcp.set_read_timeout(Some(poll))?;
    Ok(StreamOwned::new(connection, tcp))
}

fn read_ca_file(path: &str) -> Result<Vec<CertificateDer<'static>>> {
    let fail = |message: String| Error::CaFile {
        path: path.to_owned(),
        message,
    };
    let certificates = CertificateDer::pem_file_iter(path)
        .map_err(|e| fail(e.to_string()))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| fail(e.to_string()))?;
    if certificates.is_empty() {
        return Err(fail("it holds no PEM certificate".to_owned()));
    }
    Ok(certificates)
}

fn handshake_error(e: &std::io::Error) -> Error {
    let text = e.to_string();
    if text.contains("certificate") {
        Error::Tls(format!(
            "{text}. If the host's certificate comes from your own CA, set its CA certificate file."
        ))
    } else {
        Error::Tls(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A file in the system's temporary folder, removed when dropped.
    struct TempFile(std::path::PathBuf);

    impl TempFile {
        fn with(name: &str, contents: &str) -> Self {
            let path = std::env::temp_dir().join(format!("parley-{}-{name}", std::process::id()));
            std::fs::write(&path, contents).expect("the temporary folder is writable");
            Self(path)
        }

        fn path(&self) -> &str {
            self.0.to_str().expect("a UTF-8 temporary path")
        }
    }

    impl Drop for TempFile {
        fn drop(&mut self) {
            // Only a leftover file in the temporary folder if this fails.
            let _ = std::fs::remove_file(&self.0);
        }
    }

    fn message(result: Result<Vec<CertificateDer<'static>>>) -> String {
        result.expect_err("not a usable CA file").to_string()
    }

    #[test]
    fn a_missing_ca_file_is_an_error_that_names_it_r_net_2() {
        let error = message(read_ca_file("/no/such/parley-ca-æøå.pem"));
        assert!(error.contains("/no/such/parley-ca-æøå.pem"), "{error}");
    }

    #[test]
    fn a_ca_file_without_a_certificate_is_an_error_r_net_2() {
        let file = TempFile::with("empty.pem", "just text, æøå\n");
        let error = message(read_ca_file(file.path()));
        assert!(error.contains("holds no PEM certificate"), "{error}");
    }

    #[test]
    fn a_ca_file_with_a_broken_certificate_is_an_error_r_net_2() {
        let file = TempFile::with(
            "broken.pem",
            "-----BEGIN CERTIFICATE-----\n!!!not base64!!!\n-----END CERTIFICATE-----\n",
        );
        let error = message(read_ca_file(file.path()));
        assert!(error.contains(file.path()), "{error}");
    }

    #[test]
    fn a_certificate_failure_suggests_the_ca_file_r_net_2() {
        let bad = std::io::Error::new(std::io::ErrorKind::InvalidData, "invalid peer certificate");
        assert!(
            handshake_error(&bad)
                .to_string()
                .contains("CA certificate file")
        );
        let other = std::io::Error::new(std::io::ErrorKind::ConnectionReset, "reset");
        assert_eq!(handshake_error(&other).to_string(), "TLS: reset");
    }
}
