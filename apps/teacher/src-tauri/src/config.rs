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
/// değildir; ön eki öğretmen belirler.
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
    pub setup_completed: bool,
    /// Öğretmenin tanımladığı laboratuvar grupları.
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
            groups: Vec::new(),
        }
    }
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
