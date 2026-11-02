//! Parmak izine sabitlenmiş (pinned) TLS doğrulaması.
//!
//! OpenClass'ta sertifika otoritesi yoktur: öğretmen konsolu kendi kendine
//! imzalı bir sertifika üretir. Güven, sertifikanın SHA-256 parmak izinin
//! eşleştirme anında sabitlenmesine dayanır.
//!
//! * **Eşleştirme sırasında** parmak izi henüz bilinmiyorsa, sunucunun
//!   sunduğu sertifika kabul edilir ve parmak izi *öğrenilir*. Bu tek başına
//!   güvenli değildir; güvenliği sağlayan, aynı anda doğrulanan **eşleştirme
//!   kodudur** — kod, parmak izini de kapsayan bir HMAC ile bağlanır
//!   (bkz. `openclass_protocol::auth::pairing_proof`). Araya giren biri
//!   kendi sertifikasını sunarsa kanıt tutmaz ve eşleştirme reddedilir.
//! * **Sonraki bağlantılarda** parmak izi bilinir ve birebir eşleşmezse
//!   bağlantı reddedilir.
//!
//! Sertifikanın süresi, zinciri ve alan adı **kasıtlı olarak** kontrol
//! edilmez: bunlar bir CA hiyerarşisi varsayar, burada yoktur. Kimlik
//! tamamen parmak izinden gelir.

use std::sync::Arc;

use openclass_protocol::auth::cert_fingerprint;
use parking_lot::Mutex;
use tokio_rustls::rustls::client::danger::{
    HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier,
};
use tokio_rustls::rustls::crypto::{
    verify_tls12_signature, verify_tls13_signature, CryptoProvider,
};
use tokio_rustls::rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use tokio_rustls::rustls::{DigitallySignedStruct, Error as TlsError, SignatureScheme};

/// Sunucu sertifikasını yalnızca parmak izine göre doğrulayan verifier.
#[derive(Debug)]
pub struct PinnedFingerprintVerifier {
    /// Beklenen parmak izi. `None` ise ilk görülen sertifika öğrenilir.
    expected: Option<[u8; 32]>,
    /// Doğrulama sırasında görülen parmak izi (öğrenme modu için).
    observed: Mutex<Option<[u8; 32]>>,
    provider: Arc<CryptoProvider>,
}

impl PinnedFingerprintVerifier {
    /// Bilinen bir parmak izine sabitler.
    pub fn pinned(expected: [u8; 32], provider: Arc<CryptoProvider>) -> Arc<Self> {
        Arc::new(Self {
            expected: Some(expected),
            observed: Mutex::new(None),
            provider,
        })
    }

    /// Öğrenme modu: ilk görülen sertifikayı kabul eder ve kaydeder.
    /// Yalnızca eşleştirme akışında, eşleştirme kodu doğrulamasıyla birlikte
    /// kullanılır.
    pub fn learning(provider: Arc<CryptoProvider>) -> Arc<Self> {
        Arc::new(Self {
            expected: None,
            observed: Mutex::new(None),
            provider,
        })
    }

    /// El sıkışma sırasında görülen parmak izi.
    pub fn observed(&self) -> Option<[u8; 32]> {
        *self.observed.lock()
    }
}

impl ServerCertVerifier for PinnedFingerprintVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, TlsError> {
        let actual = cert_fingerprint(end_entity.as_ref());
        *self.observed.lock() = Some(actual);

        match self.expected {
            None => Ok(ServerCertVerified::assertion()),
            Some(expected) if expected == actual => Ok(ServerCertVerified::assertion()),
            Some(_) => Err(TlsError::General(
                "Öğretmen konsolunun sertifikası eşleştirme anındakinden farklı. \
                 Bağlantı güvenlik nedeniyle reddedildi. \
                 Öğretmen uygulaması yeniden kurulmuş veya verisi sıfırlanmış \
                 olabilir; bu beklenen bir durumsa Yapılandırma penceresinden \
                 'Eşleştirmeyi kaldır' deyip yeniden eşleşin."
                    .to_string(),
            )),
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
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
    ) -> Result<HandshakeSignatureValid, TlsError> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use openclass_common::tls::TlsIdentity;
    use tokio_rustls::rustls::pki_types::{IpAddr, Ipv4Addr};

    fn provider() -> Arc<CryptoProvider> {
        Arc::new(tokio_rustls::rustls::crypto::ring::default_provider())
    }

    fn check(
        verifier: &PinnedFingerprintVerifier,
        cert_der: &[u8],
    ) -> Result<ServerCertVerified, TlsError> {
        let cert = CertificateDer::from(cert_der.to_vec());
        let name = ServerName::IpAddress(IpAddr::V4(Ipv4Addr::from([127, 0, 0, 1])));
        verifier.verify_server_cert(&cert, &[], &name, &[], UnixTime::now())
    }

    #[test]
    fn dogru_parmak_izi_kabul_edilir() {
        let id = TlsIdentity::generate().unwrap();
        let v = PinnedFingerprintVerifier::pinned(id.fingerprint, provider());
        assert!(check(&v, &id.certificate_der).is_ok());
    }

    #[test]
    fn farkli_sertifika_reddedilir() {
        // Ortadaki adam senaryosu: saldırgan kendi sertifikasını sunuyor.
        let gercek = TlsIdentity::generate().unwrap();
        let sahte = TlsIdentity::generate().unwrap();
        let v = PinnedFingerprintVerifier::pinned(gercek.fingerprint, provider());
        assert!(check(&v, &sahte.certificate_der).is_err());
    }

    #[test]
    fn ogrenme_modu_ilk_sertifikayi_kaydeder() {
        let id = TlsIdentity::generate().unwrap();
        let v = PinnedFingerprintVerifier::learning(provider());
        assert!(v.observed().is_none());
        assert!(check(&v, &id.certificate_der).is_ok());
        assert_eq!(v.observed(), Some(id.fingerprint));
    }
}
