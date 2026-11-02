//! Öğretmen konsolunun TLS kimliği.
//!
//! OpenClass'ın harici sunucu, alan adı veya sertifika otoritesi gereksinimi
//! yoktur. Öğretmen uygulaması ilk açılışta kendine **kendinden imzalı** bir
//! sertifika üretir ve özel anahtarı [`crate::secrets`] ile korur.
//!
//! Öğrenci cihazları bu sertifikanın SHA-256 parmak izini eşleştirme anında
//! sabitler (pinning). Sonraki bağlantılarda parmak izi değişirse bağlantı
//! reddedilir — yani güven, bir CA'ya değil, öğretmenin eşleştirme anında
//! fiziksel olarak orada olmasına dayanır (TOFU + eşleştirme kodu onayı).

use std::path::Path;

use anyhow::{Context, Result};
use openclass_protocol::auth::{cert_fingerprint, CertFingerprint};

use crate::secrets::{self, Scope};

/// Öğretmen konsolunun TLS kimliği: sertifika zinciri + özel anahtar.
pub struct TlsIdentity {
    /// DER kodlu sertifika.
    pub certificate_der: Vec<u8>,
    /// PKCS#8 DER kodlu özel anahtar. **Loglanmaz, ağa çıkmaz.**
    pub private_key_der: Vec<u8>,
    /// Sertifikanın SHA-256 parmak izi.
    pub fingerprint: CertFingerprint,
}

impl std::fmt::Debug for TlsIdentity {
    /// Özel anahtarın yanlışlıkla loglanmasını önlemek için elle yazılmıştır.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TlsIdentity")
            .field(
                "fingerprint",
                &openclass_protocol::auth::fingerprint_to_hex(&self.fingerprint),
            )
            .field("private_key_der", &"<gizli>")
            .finish()
    }
}

impl TlsIdentity {
    /// Yeni bir kendinden imzalı sertifika üretir.
    ///
    /// Sertifikaya `localhost`, makinenin bilgisayar adı ve `127.0.0.1`
    /// eklenir. Doğrulama parmak izi üzerinden yapıldığı için SAN listesi
    /// güvenlik açısından belirleyici değildir; yalnızca TLS kütüphanelerinin
    /// biçimsel beklentisini karşılar.
    pub fn generate() -> Result<Self> {
        let mut names = vec!["localhost".to_string(), "127.0.0.1".to_string()];
        let host = crate::machine::hostname();
        if !names.contains(&host) {
            names.push(host);
        }

        let certified = rcgen::generate_simple_self_signed(names)
            .context("kendinden imzalı sertifika üretilemedi")?;

        let certificate_der = certified.cert.der().to_vec();
        let private_key_der = certified.signing_key.serialize_der();
        let fingerprint = cert_fingerprint(&certificate_der);

        Ok(Self {
            certificate_der,
            private_key_der,
            fingerprint,
        })
    }

    /// Kimliği diskten yükler; yoksa üretip kaydeder.
    ///
    /// Sertifika düz dosyada (gizli değil), özel anahtar DPAPI korumalı
    /// dosyada tutulur.
    pub fn load_or_create(dir: &Path) -> Result<Self> {
        let cert_path = dir.join("teacher-cert.der");
        let key_path = dir.join("teacher-key.bin");

        let existing_cert = match std::fs::read(&cert_path) {
            Ok(bytes) => Some(bytes),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
            Err(err) => {
                return Err(err)
                    .with_context(|| format!("sertifika okunamadı: {}", cert_path.display()))
            }
        };
        let existing_key = secrets::read_file(&key_path)?;

        if let (Some(certificate_der), Some(private_key_der)) = (existing_cert, existing_key) {
            let fingerprint = cert_fingerprint(&certificate_der);
            tracing::info!(
                parmak_izi = %openclass_protocol::auth::fingerprint_to_hex(&fingerprint),
                "mevcut TLS kimliği yüklendi"
            );
            return Ok(Self {
                certificate_der,
                private_key_der,
                fingerprint,
            });
        }

        let identity = Self::generate()?;
        std::fs::write(&cert_path, &identity.certificate_der)
            .with_context(|| format!("sertifika yazılamadı: {}", cert_path.display()))?;
        secrets::write_file(&key_path, &identity.private_key_der, Scope::CurrentUser)?;

        tracing::info!(
            parmak_izi = %openclass_protocol::auth::fingerprint_to_hex(&identity.fingerprint),
            "yeni TLS kimliği üretildi"
        );
        Ok(identity)
    }

    /// Parmak izinin kullanıcıya gösterilecek hâli.
    pub fn fingerprint_hex(&self) -> String {
        openclass_protocol::auth::fingerprint_to_hex(&self.fingerprint)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uretilen_sertifikanin_parmak_izi_tutarlidir() {
        let id = TlsIdentity::generate().unwrap();
        assert_eq!(id.fingerprint, cert_fingerprint(&id.certificate_der));
        assert!(!id.private_key_der.is_empty());
    }

    #[test]
    fn iki_ayri_kimlik_farkli_parmak_izi_uretir() {
        let a = TlsIdentity::generate().unwrap();
        let b = TlsIdentity::generate().unwrap();
        assert_ne!(a.fingerprint, b.fingerprint);
    }

    #[test]
    fn kimlik_diskten_ayni_sekilde_geri_yuklenir() {
        let dir = std::env::temp_dir().join(format!("openclass-tls-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        let first = TlsIdentity::load_or_create(&dir).unwrap();
        let second = TlsIdentity::load_or_create(&dir).unwrap();
        assert_eq!(first.fingerprint, second.fingerprint);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn debug_ciktisi_ozel_anahtari_sizdirmaz() {
        let id = TlsIdentity::generate().unwrap();
        let out = format!("{id:?}");
        assert!(out.contains("<gizli>"));
        assert!(!out.contains(&format!("{:?}", id.private_key_der)));
    }
}
