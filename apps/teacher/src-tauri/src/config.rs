//! Öğretmen konsolunun kalıcı yapılandırması.

use std::path::Path;

use anyhow::{Context, Result};
use openclass_protocol::DEFAULT_CONTROL_PORT;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Öğretmenin tanımladığı cihaz grubu (laboratuvar).
///
/// Bir grup, bilgisayar adı ön ekinden ibarettir: `LAB1` grubu `LAB101`,
/// `LAB102` … cihazlarını kapsar. Hiçbir adlandırma düzeni koda gömülü
/// değildir; ön eki öğretmen belirler. Ön eki boş bir grup yalnızca oda
/// adına göre eşleşir (grup adı = oda adı).
///
/// Gruplar aynı zamanda açılıştaki sınıf seçim penceresinin kayıtlı
/// sınıflarıdır: seçilen grubun ön eki (yoksa adı, oda olarak) konsolun
/// yönettiği kapsam olur.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceGroup {
    /// Grup bar'ında görünen ad.
    pub name: String,
    /// Eşleşme için kullanılan bilgisayar adı ön eki (büyük harf).
    pub prefix: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TeacherConfig {
    /// Konsolun kalıcı kimliği. İlk açılışta üretilir.
    pub teacher_id: Uuid,
    /// Öğrenci ekranlarında görünecek ad.
    pub teacher_name: String,
    /// Varsayılan sınıf/laboratuvar adı.
    pub class_name: String,
    /// Kontrol kanalının dinlediği TCP portu.
    pub control_port: u16,
    /// Yayının engellendiği ağlar için elle eklenmiş keşif hedefleri
    /// (IP veya alan adı).
    pub extra_discovery_hosts: Vec<String>,
    /// Kurulum sihirbazı tamamlandı mı?
    ///
    /// `false` iken konsol açılışta hangi laboratuvarın yönetileceğini sorar.
    pub setup_completed: bool,
    /// Konsolun yönettiği laboratuvar/oda adı.
    ///
    /// `None` ise bütün cihazlar listelenir. Öğrencinin bildirdiği oda adıyla
    /// karşılaştırılır; odası bilinmeyen cihazlar filtre uygulanınca listede
    /// görünmez.
    /// Öğretmenin tanımladığı laboratuvar grupları.
    pub active_room: Option<String>,
    /// Konsolun süzdüğü bilgisayar adı ön eki (`LAB1`).
    ///
    /// Oda adı bilinmeyen laboratuvarlar için ikinci yol. Oda ve ön ek
    /// birlikte verilirse **oda önceliklidir**; ön ek yalnızca odası
    /// bilinmeyen cihazlara uygulanır.
    pub active_prefix: Option<String>,
    /// Konsol her açılışta sınıf seçim penceresini göstersin mi?
    ///
    /// NetSupport'un Class Wizard'ındaki "bu pencereyi bir daha gösterme"
    /// kutusunun tersidir. `false` iken konsol son seçilen sınıfla açılır.
    pub ask_at_startup: bool,
    pub groups: Vec<DeviceGroup>,
}

impl Default for TeacherConfig {
    fn default() -> Self {
        Self {
            teacher_id: Uuid::new_v4(),
            teacher_name: "Öğretmen".to_string(),
            class_name: "Laboratuvar".to_string(),
            control_port: DEFAULT_CONTROL_PORT,
            extra_discovery_hosts: Vec::new(),
            setup_completed: false,
            active_room: None,
            active_prefix: None,
            ask_at_startup: true,
            groups: Vec::new(),
        }
    }
}

/// Öğretmen bilgisayarının adından laboratuvar ön ekini tahmin eder.
///
/// Okullarda yaygın düzen: öğretmen `LAB1-OGR`, öğrenciler `LAB101`,
/// `LAB102` … Sondaki "öğretmen" eki atılınca kalan kısım (`LAB1`)
/// öğrencilerin ortak ön ekidir. Yalnızca bilinen eklerde öneri üretilir;
/// `DESKTOP-4F9K2A` gibi adlardan tahmin yürütülmez. Sonuç bir öneridir,
/// öğretmen onaylamadan uygulanmaz.
pub fn suggest_prefix_from_hostname(hostname: &str) -> Option<String> {
    const SUFFIXES: [&str; 8] = [
        "OGRETMEN", "TEACHER", "OGRT", "OGR", "HOCA", "TCH", "MASTER", "SERVER",
    ];
    let host = hostname.trim().to_ascii_uppercase();
    let stem = SUFFIXES.iter().find_map(|s| host.strip_suffix(s))?;
    let stem = stem.trim_end_matches(['-', '_', '.', ' ']);
    (!stem.is_empty()).then(|| stem.to_string())
}

impl TeacherConfig {
    /// Yapılandırmayı okur; dosya yoksa veya bozuksa varsayılanı üretip yazar.
    pub fn load_or_create(path: &Path) -> Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(text) => match serde_json::from_str::<Self>(&text) {
                Ok(cfg) => Ok(cfg),
                Err(err) => {
                    // Bozuk yapılandırma uygulamayı açılamaz hâle getirmemeli.
                    tracing::error!(%err, "yapılandırma bozuk, varsayılana dönülüyor");
                    let cfg = Self::default();
                    cfg.save(path)?;
                    Ok(cfg)
                }
            },
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                let cfg = Self::default();
                cfg.save(path)?;
                Ok(cfg)
            }
            Err(err) => {
                Err(err).with_context(|| format!("yapılandırma okunamadı: {}", path.display()))
            }
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let text = serde_json::to_string_pretty(self)?;
        std::fs::write(path, text)
            .with_context(|| format!("yapılandırma yazılamadı: {}", path.display()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::suggest_prefix_from_hostname;

    #[test]
    fn ogretmen_ekinden_on_ek_onerilir() {
        assert_eq!(
            suggest_prefix_from_hostname("LAB1-OGR").as_deref(),
            Some("LAB1")
        );
        assert_eq!(
            suggest_prefix_from_hostname("lab2_ogretmen").as_deref(),
            Some("LAB2")
        );
        assert_eq!(
            suggest_prefix_from_hostname("BIL-A-TEACHER").as_deref(),
            Some("BIL-A")
        );
        assert_eq!(
            suggest_prefix_from_hostname("LAB3OGR").as_deref(),
            Some("LAB3")
        );
    }

    #[test]
    fn bilinmeyen_adlardan_tahmin_yurutulmez() {
        assert_eq!(suggest_prefix_from_hostname("DESKTOP-4F9K2A"), None);
        assert_eq!(suggest_prefix_from_hostname("LAB101"), None);
        // Ekin kendisinden ibaret ad bir ön ek bırakmaz.
        assert_eq!(suggest_prefix_from_hostname("OGR"), None);
        assert_eq!(suggest_prefix_from_hostname("-OGR"), None);
    }
}
