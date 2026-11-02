//! Öğrenci ajanının kalıcı yapılandırması.

use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Eşleşilmiş öğretmen konsolu.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PairedTeacher {
    pub teacher_id: Uuid,
    pub teacher_name: String,
    /// Son bilinen adres. IP değişirse keşif ile tazelenir.
    pub host: String,
    pub port: u16,
    /// Eşleştirme anında sabitlenen TLS sertifika parmak izi (SHA-256).
    ///
    /// Bağlantı bu parmak izine göre doğrulanır. Değişirse bağlantı
    /// reddedilir — ortadaki adam saldırısına karşı asıl savunma budur.
    #[serde(with = "hex_bytes")]
    pub cert_fingerprint: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct StudentConfig {
    pub paired_teacher: Option<PairedTeacher>,
    /// Keşfin engellendiği ağlarda elle girilen öğretmen adresi (`ip` veya `ip:port`).
    pub manual_teacher_address: Option<String>,
    /// Bu bilgisayarın bulunduğu laboratuvar/oda adı.
    ///
    /// İlk açılış sihirbazında sorulur ve `Hello` ile öğretmene bildirilir;
    /// öğretmen cihazları buna göre gruplayabilir. Yapı `#[serde(default)]`
    /// taşıdığı için alanı bilmeyen eski yapılandırma dosyaları bozulmaz.
    pub room: Option<String>,
}

impl StudentConfig {
    pub fn load_or_create(path: &Path) -> Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(text) => match serde_json::from_str::<Self>(&text) {
                Ok(cfg) => Ok(cfg),
                Err(err) => {
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

/// Parmak izini JSON'da okunabilir onaltılık metin olarak saklar.
mod hex_bytes {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(bytes: &[u8], s: S) -> Result<S::Ok, S::Error> {
        let text: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        s.serialize_str(&text)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        let text = String::deserialize(d)?;
        if text.len() % 2 != 0 {
            return Err(serde::de::Error::custom(
                "parmak izi tek sayıda karakter içeriyor",
            ));
        }
        (0..text.len())
            .step_by(2)
            .map(|i| {
                u8::from_str_radix(&text[i..i + 2], 16)
                    .map_err(|_| serde::de::Error::custom("parmak izi onaltılık değil"))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parmak_izi_onaltilik_olarak_saklanir_ve_okunur() {
        let cfg = StudentConfig {
            paired_teacher: Some(PairedTeacher {
                teacher_id: Uuid::nil(),
                teacher_name: "Cihan Hoca".into(),
                host: "192.168.1.10".into(),
                port: 7643,
                cert_fingerprint: vec![0xAB, 0xCD, 0x01],
            }),
            manual_teacher_address: None,
            room: Some("Lab 1".into()),
        };
        let json = serde_json::to_string(&cfg).unwrap();
        assert!(
            json.contains("abcd01"),
            "parmak izi onaltılık yazılmalı: {json}"
        );

        let back: StudentConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.paired_teacher, cfg.paired_teacher);
    }

    #[test]
    fn bozuk_parmak_izi_reddedilir() {
        let json = r#"{"pairedTeacher":{"teacherId":"00000000-0000-0000-0000-000000000000",
            "teacherName":"x","host":"h","port":1,"certFingerprint":"zz"}}"#;
        assert!(serde_json::from_str::<StudentConfig>(json).is_err());
    }
}
