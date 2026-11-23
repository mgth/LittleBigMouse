//! Mutual TLS 1.3 between two machines that have no CA in common.
//!
//! Both sides present a self-signed certificate and both accept any. What the
//! handshake still proves is that each side holds the key of the certificate it
//! showed — the signatures are verified as usual — so the fingerprint read afterwards
//! names the machine at the other end. Whether that machine is trusted is the
//! caller's decision, made against its pairing store: this is the pinning the TV
//! remotes already do (`lbm-agent/src/remote/network.rs`), done in both directions.

use std::io;
use std::sync::Arc;

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{verify_tls13_signature, CryptoProvider};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::server::danger::{ClientCertVerified, ClientCertVerifier};
use rustls::{
    ClientConfig, DigitallySignedStruct, DistinguishedName, Error, ServerConfig, SignatureScheme,
};

use crate::identity::Identity;

/// The name a client asks the server for. Nobody checks it — the fingerprint is the
/// identity — but TLS needs one.
pub const SERVER_NAME: &str = "littlebigmouse";

/// The crypto provider, named rather than taken from a process-wide default, so that
/// whatever else the process links cannot change it or leave it ambiguous.
pub fn provider() -> Arc<CryptoProvider> {
    Arc::new(rustls::crypto::ring::default_provider())
}

/// The configuration a listening node accepts connections with: TLS 1.3 only, and a
/// client certificate is required.
pub fn server_config(identity: &Identity) -> io::Result<Arc<ServerConfig>> {
    let provider = provider();
    let config = ServerConfig::builder_with_provider(provider.clone())
        .with_protocol_versions(&[&rustls::version::TLS13])
        .map_err(tls_error)?
        .with_client_cert_verifier(Arc::new(AnyCertificate(provider)))
        .with_single_cert(vec![identity.certificate().clone()], identity.key())
        .map_err(tls_error)?;
    Ok(Arc::new(config))
}

/// The configuration a node connects with: TLS 1.3 only, presenting its own
/// certificate.
pub fn client_config(identity: &Identity) -> io::Result<Arc<ClientConfig>> {
    let provider = provider();
    let config = ClientConfig::builder_with_provider(provider.clone())
        .with_protocol_versions(&[&rustls::version::TLS13])
        .map_err(tls_error)?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(AnyCertificate(provider)))
        .with_client_auth_cert(vec![identity.certificate().clone()], identity.key())
        .map_err(tls_error)?;
    Ok(Arc::new(config))
}

/// The fingerprint of the certificate the other end presented, once the handshake is
/// done. `None` cannot happen with these configurations (both sides require one) and
/// is reported rather than assumed.
pub fn peer_fingerprint(connection: &rustls::CommonState) -> Option<String> {
    connection
        .peer_certificates()
        .and_then(|chain| chain.first())
        .map(|certificate| crate::identity::fingerprint(certificate))
}

fn tls_error(error: Error) -> io::Error {
    io::Error::other(format!("TLS configuration: {error}"))
}

/// Accepts any certificate, checks every signature.
#[derive(Debug)]
struct AnyCertificate(Arc<CryptoProvider>);

impl AnyCertificate {
    fn verify13(
        &self,
        message: &[u8],
        certificate: &CertificateDer<'_>,
        signature: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        verify_tls13_signature(
            message,
            certificate,
            signature,
            &self.0.signature_verification_algorithms,
        )
    }

    fn schemes(&self) -> Vec<SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }
}

impl ServerCertVerifier for AnyCertificate {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _certificate: &CertificateDer<'_>,
        _signature: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        // Unreachable with TLS 1.3 only; refusing keeps it so if that ever changes.
        Err(Error::PeerIncompatible(
            rustls::PeerIncompatible::Tls12NotOffered,
        ))
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        certificate: &CertificateDer<'_>,
        signature: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        self.verify13(message, certificate, signature)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.schemes()
    }
}

impl ClientCertVerifier for AnyCertificate {
    fn root_hint_subjects(&self) -> &[DistinguishedName] {
        &[]
    }

    fn client_auth_mandatory(&self) -> bool {
        true
    }

    fn verify_client_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _now: UnixTime,
    ) -> Result<ClientCertVerified, Error> {
        Ok(ClientCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _certificate: &CertificateDer<'_>,
        _signature: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        Err(Error::PeerIncompatible(
            rustls::PeerIncompatible::Tls12NotOffered,
        ))
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        certificate: &CertificateDer<'_>,
        signature: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        self.verify13(message, certificate, signature)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.schemes()
    }
}
