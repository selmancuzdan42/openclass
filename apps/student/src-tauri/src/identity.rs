//! Öğrenci cihazının kalıcı kimliği.
//!
//! Kimlik iki parçadan oluşur: rastgele bir `device_id` ve bir Ed25519
//! imzalama anahtarı. İkisi birlikte tek bir korumalı dosyada saklanır.
//!
//! # Neden hostname'e güvenilmiyor?
//!
//! Bilgisayar adı taklit edilebilir ve okul içinde değişebilir. Cihazın
//! yönetilebilir olmasının dayanağı bu özel anahtardır; bilgisayar adı
//! yalnızca öğretmene gösterilen bir etikettir.
//!
//! # Çoklu kullanıcı notu
//!
//! Dosya [`Scope::LocalMachine`] ile korunur. Laboratuvar bilgisayarında
//! farklı Windows hesaplarıyla oturum açan öğrenciler aynı cihaz kimliğini
//! görür; aksi hâlde her hesap ayrı bir cihaz gibi davranır ve öğretmen
//! aynı makineyi defalarca eşleştirmek zorunda kalırdı.

use std::path::Path;

use anyhow::{bail, Context, Result};
use openclass_common::secrets::{self, Scope};
use openclass_protocol::auth::DeviceIdentity;
use uuid::Uuid;

/// Saklanan kimlik verisinin uzunluğu: 16 bayt UUID + 32 bayt tohum.
const BLOB_LEN: usize = 48;

pub struct StudentIdentity {
    pub device_id: Uuid,
    pub keys: DeviceIdentity,
}

impl StudentIdentity {
    /// Kimliği diskten yükler; yoksa üretip kaydeder.
    pub fn load_or_create(secrets_dir: &Path) -> Result<Self> {
        let path = secrets_dir.join("device-identity.bin");

        // Okunamayan kimlik dosyası uygulamayı kilitlememeli. İki ayrı
        // başarısızlık var ve **ikisi de** yenisini üretmekle çözülür:
        //
        // 1. Dosya okunamıyor veya şifresi çözülemiyor. Windows'ta kimlik
        //    DPAPI ile makineye bağlı şifrelenir; disk imajı kopyalanarak
        //    kurulan laboratuvar bilgisayarlarında blob **başka makineye
        //    ait** olur ve çözülemez. Okulların kurulum yöntemi tam olarak
        //    budur, yani bu beklenen bir durum.
        // 2. Çözülüyor ama içeriği geçersiz (yarım yazılmış, bozulmuş).
        //
        // Eskiden yalnızca ikincisi ele alınıyordu; birincisi hatayı yukarı
        // fırlatıp ajanın hiç açılmamasına yol açıyordu.
        let stored = match secrets::read_file(&path) {
            Ok(blob) => blob,
            Err(err) => {
                tracing::error!(
                    error = %format!("{err:#}"),
                    "cihaz kimliği okunamadı, yenisi üretiliyor — yeniden eşleştirme gerekecek"
                );
                None
            }
        };

        if let Some(blob) = stored {
            match Self::from_blob(&blob) {
                Ok(identity) => {
                    tracing::info!(cihaz = %identity.device_id, "cihaz kimliği yüklendi");
                    return Ok(identity);
                }
                Err(err) => {
                    tracing::error!(
                        error = %format!("{err:#}"),
                        "cihaz kimliği bozuk, yenisi üretiliyor — yeniden eşleştirme gerekecek"
                    );
                }
            }
        }

        let identity = Self {
            device_id: Uuid::new_v4(),
            keys: DeviceIdentity::generate(),
        };
        secrets::write_file(&path, &identity.to_blob(), Scope::LocalMachine)
            .context("cihaz kimliği kaydedilemedi")?;
        tracing::info!(cihaz = %identity.device_id, "yeni cihaz kimliği üretildi");
        Ok(identity)
    }

    fn to_blob(&self) -> Vec<u8> {
        let mut blob = Vec::with_capacity(BLOB_LEN);
        blob.extend_from_slice(self.device_id.as_bytes());
        blob.extend_from_slice(&self.keys.seed());
        blob
    }

    fn from_blob(blob: &[u8]) -> Result<Self> {
        if blob.len() != BLOB_LEN {
            bail!(
                "kimlik dosyası {} bayt, {BLOB_LEN} bekleniyordu",
                blob.len()
            );
        }
        let mut id_bytes = [0u8; 16];
        id_bytes.copy_from_slice(&blob[..16]);
        let mut seed = [0u8; 32];
        seed.copy_from_slice(&blob[16..]);
        Ok(Self {
            device_id: Uuid::from_bytes(id_bytes),
            keys: DeviceIdentity::from_seed(&seed),
        })
    }

    pub fn public_key(&self) -> [u8; 32] {
        self.keys.public_key()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kimlik_diskten_ayni_sekilde_geri_yuklenir() {
        let dir = std::env::temp_dir().join(format!("openclass-id-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        let first = StudentIdentity::load_or_create(&dir).unwrap();
        let second = StudentIdentity::load_or_create(&dir).unwrap();
        assert_eq!(first.device_id, second.device_id);
        assert_eq!(first.public_key(), second.public_key());

        std::fs::remove_dir_all(&dir).ok();
    }

    /// Okunamayan kimlik dosyası ajanı kilitlememeli.
    ///
    /// Windows'ta bu, başka bir makinede şifrelenmiş blob'u (disk imajı
    /// kopyalanmış laboratuvar bilgisayarı) temsil eder: DPAPI çözemez.
    /// Diğer platformlarda yalnızca bozuk içerik olur. Her iki durumda da
    /// beklenen davranış aynı: yeni kimlik üret, çökme.
    #[test]
    fn okunamayan_kimlik_dosyasi_yenisiyle_degistirilir() {
        let dir = std::env::temp_dir().join(format!("openclass-id-bozuk-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("device-identity.bin"), b"bozuk").unwrap();

        let identity = StudentIdentity::load_or_create(&dir).unwrap();
        assert_ne!(identity.device_id, Uuid::nil());

        // Yenisi diske yazılmış ve tekrar okunabiliyor olmalı.
        let again = StudentIdentity::load_or_create(&dir).unwrap();
        assert_eq!(identity.device_id, again.device_id);

        std::fs::remove_dir_all(&dir).ok();
    }

    /// Doğru şifrelenmiş ama içeriği geçersiz dosya da yenisiyle değişmeli.
    #[test]
    fn gecersiz_icerikli_kimlik_yenisiyle_degistirilir() {
        let dir = std::env::temp_dir().join(format!("openclass-id-kisa-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("device-identity.bin");

        // Platformun kendi yazma yolundan geçir ki Windows'ta DPAPI ile
        // şifrelensin; bozukluk yalnızca uzunlukta olsun.
        secrets::write_file(&path, b"kisa", Scope::LocalMachine).unwrap();

        let identity = StudentIdentity::load_or_create(&dir).unwrap();
        assert_ne!(identity.device_id, Uuid::nil());

        let again = StudentIdentity::load_or_create(&dir).unwrap();
        assert_eq!(identity.device_id, again.device_id);

        std::fs::remove_dir_all(&dir).ok();
    }
}
