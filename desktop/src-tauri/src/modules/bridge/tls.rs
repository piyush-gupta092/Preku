//! TLS for the bridge: mutual TLS between self-signed certificates, pinned by fingerprint.
//!
//! - The server *requires* a client certificate and checks the handshake signature (proof the
//!   phone holds the key), but accepts any certificate at the TLS layer. Whether that
//!   certificate is a paired phone is decided right after, by fingerprint (`server.rs`).
//! - The client (the phone; here also used by tests) pins the PC certificate's fingerprint
//!   from the QR and refuses anything else. No certificate authorities, no hostnames.

use std::sync::Arc;

use rustls::{
    client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
    crypto::{verify_tls12_signature, verify_tls13_signature, CryptoProvider},
    pki_types::{CertificateDer, PrivateKeyDer, ServerName, UnixTime},
    server::danger::{ClientCertVerified, ClientCertVerifier},
    CertificateError, ClientConfig, DigitallySignedStruct, DistinguishedName, ServerConfig,
    SignatureScheme,
};

use super::identity::{fingerprint, Identity};
use crate::foundation::errors::{PrekuError, Result};

fn provider() -> Arc<CryptoProvider> {
    Arc::new(rustls::crypto::ring::default_provider())
}

fn tls_error(err: rustls::Error) -> PrekuError {
    PrekuError::Bridge(format!("tls: {err}"))
}

/// Requires a client certificate and a valid handshake signature; pinning happens later.
#[derive(Debug)]
struct RequireClientCert(Arc<CryptoProvider>);

impl ClientCertVerifier for RequireClientCert {
    fn root_hint_subjects(&self) -> &[DistinguishedName] {
        &[]
    }

    fn verify_client_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _now: UnixTime,
    ) -> std::result::Result<ClientCertVerified, rustls::Error> {
        Ok(ClientCertVerified::assertion())
    }

    fn client_auth_mandatory(&self) -> bool {
        true
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls12_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls13_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }
}

pub fn server_config(identity: &Identity) -> Result<Arc<ServerConfig>> {
    let provider = provider();
    let config = ServerConfig::builder_with_provider(provider.clone())
        .with_safe_default_protocol_versions()
        .map_err(tls_error)?
        .with_client_cert_verifier(Arc::new(RequireClientCert(provider)))
        .with_single_cert(vec![identity.cert()], identity.key())
        .map_err(tls_error)?;
    Ok(Arc::new(config))
}

/// Accepts exactly one server certificate: the one whose SHA-256 is `fingerprint`.
#[derive(Debug)]
pub struct PinnedServer {
    fingerprint: String,
    provider: Arc<CryptoProvider>,
}

impl PinnedServer {
    pub fn new(fingerprint: &str) -> Self {
        Self {
            fingerprint: fingerprint.to_ascii_lowercase(),
            provider: provider(),
        }
    }
}

impl ServerCertVerifier for PinnedServer {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> std::result::Result<ServerCertVerified, rustls::Error> {
        if fingerprint(end_entity) == self.fingerprint {
            Ok(ServerCertVerified::assertion())
        } else {
            Err(rustls::Error::InvalidCertificate(
                CertificateError::ApplicationVerificationFailure,
            ))
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls12_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls13_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}

/// A client config that pins the server and presents `cert`/`key` (the phone's side).
pub fn client_config(
    server_fingerprint: &str,
    cert: CertificateDer<'static>,
    key: PrivateKeyDer<'static>,
) -> Result<Arc<ClientConfig>> {
    let config = ClientConfig::builder_with_provider(provider())
        .with_safe_default_protocol_versions()
        .map_err(tls_error)?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(PinnedServer::new(server_fingerprint)))
        .with_client_auth_cert(vec![cert], key)
        .map_err(tls_error)?;
    Ok(Arc::new(config))
}

/// For the browser companion (iPhone): the same certificate, but no client certificate (Safari
/// can't present one). Browsers authenticate with a session cookie instead (`web.rs`).
pub fn web_server_config(identity: &Identity) -> Result<Arc<ServerConfig>> {
    let config = ServerConfig::builder_with_provider(provider())
        .with_safe_default_protocol_versions()
        .map_err(tls_error)?
        .with_no_client_auth()
        .with_single_cert(vec![identity.cert()], identity.key())
        .map_err(tls_error)?;
    Ok(Arc::new(config))
}
