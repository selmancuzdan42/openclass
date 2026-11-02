//! Cihaz eşleştirme ve kimlik doğrulama ilkelleri.
//!
//! # Tehdit modeli
//!
//! Aynı yerel ağda, OpenClass Student ikilisini elde edebilen ve kendini
//! `LAB101` olarak duyurabilen bir saldırgan varsayılır. Amaç:
//!
//! * Yetkisiz bir cihazın öğretmen konsoluna "öğrenci" olarak kabul edilmemesi,
//! * Yetkisiz bir konsolun gerçek öğrenci cihazlarını yönetememesi,
//! * Ağı dinleyen birinin ekran görüntülerini okuyamaması.
//!
//! # Tasarım
//!
//! Her öğrenci cihazı eşleştirme anında bir **Ed25519 anahtar çifti** üretir.
//! Özel anahtar cihazdan hiç çıkmaz (Windows'ta DPAPI ile sarmalanır).
//! Öğretmen yalnızca açık anahtarı saklar.
//!
//! Hem eşleştirme kanıtı hem de oturum challenge'ı, kullanılan TLS
//! sertifikasının parmak izini içerir (**kanal bağlama / channel binding**).
//! Böylece araya giren bir aktör kanıtı yakalayıp başka bir TLS oturumuna
//! taşıyamaz.
//!
//! ## Eşleştirme kodunun sınırı — dürüst değerlendirme
//!
//! Eşleştirme kodu 6 hanelidir ve bu bir PAKE (ör. SPAKE2) değildir. Kanıtı
//! yakalayan biri teorik olarak 10^6 kodu çevrimdışı deneyebilir. Buna karşı
//! savunma şudur: kod **tek kullanımlıktır**, kısa ömürlüdür ve öğretmen
//! tarafında deneme sayısı sınırlıdır (bkz. `PairingCode`). Gerçek koruma
//! TLS'in gizliliğidir; kod yalnızca "bu cihazı gerçekten ben ekliyorum"
//! onayıdır. İleride SPAKE2'ye geçiş `docs/ARCHITECTURE.md` içinde
//! açık bir iyileştirme maddesidir.

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use hmac::{Hmac, KeyInit, Mac};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use uuid::Uuid;

/// Eşleştirme kanıtı için alan ayırma etiketi.
const DOMAIN_PAIR: &[u8] = b"OPENCLASS/v1/pairing-proof";

/// Oturum challenge imzası için alan ayırma etiketi.
const DOMAIN_AUTH: &[u8] = b"OPENCLASS/v1/session-challenge";

/// Bir TLS sertifikasının SHA-256 parmak izi.
pub type CertFingerprint = [u8; 32];

/// Oturum açarken imzalanan rastgele değer.
pub type Challenge = [u8; 32];

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("açık anahtar geçersiz")]
    InvalidPublicKey,

    #[error("imza geçersiz")]
    InvalidSignature,

    #[error("eşleştirme kanıtı doğrulanamadı")]
    InvalidPairingProof,

    #[error("eşleştirme kodu süresi doldu veya daha önce kullanıldı")]
    PairingCodeExpired,

    #[error("çok fazla hatalı eşleştirme denemesi")]
    TooManyAttempts,

    #[error("beklenen uzunluk {expected}, gelen {got}")]
    BadLength { expected: usize, got: usize },
}

/// Bir X.509 sertifikasının DER baytlarından SHA-256 parmak izini hesaplar.
pub fn cert_fingerprint(der: &[u8]) -> CertFingerprint {
    let mut hasher = Sha256::new();
    hasher.update(der);
    hasher.finalize().into()
}

/// Parmak izini kullanıcıya gösterilebilir biçime çevirir: `AB:CD:EF:...`
pub fn fingerprint_to_hex(fp: &CertFingerprint) -> String {
    fp.iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(":")
}

// --------------------------------------------------------------------------
// Cihaz kimliği
// --------------------------------------------------------------------------

/// Öğrenci cihazının kalıcı imzalama anahtarı.
pub struct DeviceIdentity {
    signing_key: SigningKey,
}

impl DeviceIdentity {
    /// Yeni bir kimlik üretir.
    pub fn generate() -> Self {
        Self {
            signing_key: SigningKey::from_bytes(&crate::random_bytes::<32>()),
        }
    }

    /// Diskten okunan 32 baytlık tohumdan kimliği geri yükler.
    pub fn from_seed(seed: &[u8; 32]) -> Self {
        Self {
            signing_key: SigningKey::from_bytes(seed),
        }
    }

    /// Diske yazılacak 32 baytlık tohum. **Asla loglanmaz, asla ağa çıkmaz.**
    pub fn seed(&self) -> [u8; 32] {
        self.signing_key.to_bytes()
    }

    /// Öğretmene bildirilen açık anahtar.
    pub fn public_key(&self) -> [u8; 32] {
        self.signing_key.verifying_key().to_bytes()
    }

    /// Öğretmenin gönderdiği challenge'ı imzalar.
    pub fn sign_challenge(&self, cert_fp: &CertFingerprint, challenge: &Challenge) -> [u8; 64] {
        let msg = challenge_message(cert_fp, challenge);
        self.signing_key.sign(&msg).to_bytes()
    }
}

/// İmzalanacak challenge mesajını oluşturur (kanal bağlama dahil).
fn challenge_message(cert_fp: &CertFingerprint, challenge: &Challenge) -> Vec<u8> {
    let mut msg = Vec::with_capacity(DOMAIN_AUTH.len() + 64);
    msg.extend_from_slice(DOMAIN_AUTH);
    msg.extend_from_slice(cert_fp);
    msg.extend_from_slice(challenge);
    msg
}

/// Öğretmen tarafı: cihazın challenge imzasını doğrular.
pub fn verify_challenge(
    public_key: &[u8; 32],
    cert_fp: &CertFingerprint,
    challenge: &Challenge,
    signature: &[u8; 64],
) -> Result<(), AuthError> {
    let vk = VerifyingKey::from_bytes(public_key).map_err(|_| AuthError::InvalidPublicKey)?;
    let sig = Signature::from_bytes(signature);
    vk.verify(&challenge_message(cert_fp, challenge), &sig)
        .map_err(|_| AuthError::InvalidSignature)
}

// --------------------------------------------------------------------------
// Eşleştirme
// --------------------------------------------------------------------------

type HmacSha256 = Hmac<Sha256>;

/// Öğrenci tarafı: eşleştirme kanıtı üretir.
///
/// Kanıt, kodu bilen birinin *bu* TLS oturumunda *bu* açık anahtarı sunduğunu
/// gösterir. Kod, parmak izi veya açık anahtar değişirse kanıt tutmaz.
pub fn pairing_proof(
    code: &str,
    cert_fp: &CertFingerprint,
    device_id: &Uuid,
    public_key: &[u8; 32],
) -> [u8; 32] {
    let mut mac = HmacSha256::new_from_slice(normalize_code(code).as_bytes())
        .expect("HMAC-SHA256 her uzunlukta anahtarı kabul eder");
    mac.update(DOMAIN_PAIR);
    mac.update(cert_fp);
    mac.update(device_id.as_bytes());
    mac.update(public_key);
    mac.finalize().into_bytes().into()
}

/// Öğretmen tarafı: kanıtı sabit zamanlı karşılaştırır.
pub fn verify_pairing_proof(
    code: &str,
    cert_fp: &CertFingerprint,
    device_id: &Uuid,
    public_key: &[u8; 32],
    proof: &[u8; 32],
) -> Result<(), AuthError> {
    let expected = pairing_proof(code, cert_fp, device_id, public_key);
    if expected.ct_eq(proof).into() {
        Ok(())
    } else {
        Err(AuthError::InvalidPairingProof)
    }
}

/// Kodu karşılaştırmadan önce normalleştirir: boşluk ve tire atılır, büyük harfe çevrilir.
/// Öğretmen ekranda `428 913` gösterse de öğrenci `428913` yazabilir.
fn normalize_code(code: &str) -> String {
    code.chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .collect::<String>()
        .to_ascii_uppercase()
}

/// Kullanıcıya gösterilen 6 haneli eşleştirme kodunu üretir.
///
/// Rakamlar düzgün dağılım için reddetme örneklemesi (rejection sampling) ile
/// seçilir; `% 10` kullanılsaydı küçük bir sapma oluşurdu.
pub fn generate_pairing_code() -> String {
    let mut digits = String::with_capacity(6);
    while digits.len() < 6 {
        for b in crate::random_bytes::<32>() {
            if b < 250 {
                digits.push(char::from(b'0' + (b % 10)));
                if digits.len() == 6 {
                    break;
                }
            }
        }
    }
    digits
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fp() -> CertFingerprint {
        [0xAB; 32]
    }

    #[test]
    fn dogru_imza_kabul_edilir() {
        let id = DeviceIdentity::generate();
        let challenge: Challenge = [9u8; 32];
        let sig = id.sign_challenge(&fp(), &challenge);
        assert!(verify_challenge(&id.public_key(), &fp(), &challenge, &sig).is_ok());
    }

    #[test]
    fn baska_challenge_icin_uretilen_imza_reddedilir() {
        let id = DeviceIdentity::generate();
        let sig = id.sign_challenge(&fp(), &[1u8; 32]);
        assert!(verify_challenge(&id.public_key(), &fp(), &[2u8; 32], &sig).is_err());
    }

    #[test]
    fn baska_sertifikaya_tasinan_imza_reddedilir() {
        // Kanal bağlamanın asıl amacı: relay saldırısını kırmak.
        let id = DeviceIdentity::generate();
        let challenge: Challenge = [3u8; 32];
        let sig = id.sign_challenge(&fp(), &challenge);
        assert!(verify_challenge(&id.public_key(), &[0xCD; 32], &challenge, &sig).is_err());
    }

    #[test]
    fn baska_cihazin_imzasi_reddedilir() {
        let a = DeviceIdentity::generate();
        let b = DeviceIdentity::generate();
        let challenge: Challenge = [4u8; 32];
        let sig = a.sign_challenge(&fp(), &challenge);
        assert!(verify_challenge(&b.public_key(), &fp(), &challenge, &sig).is_err());
    }

    #[test]
    fn kimlik_tohumdan_geri_yuklenebilir() {
        let a = DeviceIdentity::generate();
        let b = DeviceIdentity::from_seed(&a.seed());
        assert_eq!(a.public_key(), b.public_key());
    }

    #[test]
    fn eslestirme_kaniti_dogru_kodla_gecer() {
        let id = DeviceIdentity::generate();
        let dev = Uuid::new_v4();
        let proof = pairing_proof("428913", &fp(), &dev, &id.public_key());
        assert!(verify_pairing_proof("428913", &fp(), &dev, &id.public_key(), &proof).is_ok());
    }

    #[test]
    fn eslestirme_kaniti_yanlis_kodla_reddedilir() {
        let id = DeviceIdentity::generate();
        let dev = Uuid::new_v4();
        let proof = pairing_proof("428913", &fp(), &dev, &id.public_key());
        assert!(verify_pairing_proof("428914", &fp(), &dev, &id.public_key(), &proof).is_err());
    }

    #[test]
    fn kanit_baska_acik_anahtara_tasinamaz() {
        let a = DeviceIdentity::generate();
        let b = DeviceIdentity::generate();
        let dev = Uuid::new_v4();
        let proof = pairing_proof("428913", &fp(), &dev, &a.public_key());
        assert!(verify_pairing_proof("428913", &fp(), &dev, &b.public_key(), &proof).is_err());
    }

    #[test]
    fn kod_bosluk_ve_tireden_bagimsizdir() {
        let id = DeviceIdentity::generate();
        let dev = Uuid::new_v4();
        let proof = pairing_proof("428 913", &fp(), &dev, &id.public_key());
        assert!(verify_pairing_proof("428-913", &fp(), &dev, &id.public_key(), &proof).is_ok());
    }

    #[test]
    fn uretilen_kod_alti_hanelidir() {
        for _ in 0..50 {
            let c = generate_pairing_code();
            assert_eq!(c.len(), 6);
            assert!(c.chars().all(|ch| ch.is_ascii_digit()));
        }
    }
}
