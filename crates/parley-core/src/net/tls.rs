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
