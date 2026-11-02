//! Uzak dosya gezgini — öğretmen tarafı.
//!
//! # Yapılmayanlar
//!
//! Gezgin **yalnızca listeler ve indirir.** Yazma, silme, taşıma,
//! yeniden adlandırma ve çalıştırma ne burada ne de protokolde vardır.
//!
//! # Sınırlar
//!
//! * Öğrencide gezgin açık olduğu sürece görünür bir gösterge durur.
//! * Aynı anda **tek** öğrenci gezilebilir.
//! * Her listeleme ve her indirme denetim kaydına yazılır.
//! * Öğrencide gezgin açık olduğu sürece görünür bir gösterge durur ve
//!   her indirme ayrıca bildirilir.
//! * Gezilebilen yerler öğrenci tarafında kısıtlanır (Masaüstü, Belgeler,
//!   İndirilenler, OpenClass klasörü). Öğretmenin istediği yolu yazması
//!   kapsamı genişletmez — karar öğrenci tarafında verilir.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use openclass_protocol::{BrowseEntry, BrowseRoot};
use serde::Serialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// Yürüyen bir indirme.
pub struct Download {
    pub request_id: Uuid,
    pub name: String,
    pub size: u64,
    pub received: u64,
    expected: [u8; 32],
    hasher: Sha256,
    writer: BufWriter<File>,
    part_path: PathBuf,
    final_path: PathBuf,
    next_seq: u32,
}

impl Download {
    /// İndirmeyi hazırlar.
    ///
    /// Dosya öğrencinin adını taşıyan bir alt klasöre yazılır: iki
    /// öğrenciden gelen `ödev.docx` birbirini ezmemeli.
    pub fn open(
        request_id: Uuid,
        hostname: &str,
        name: &str,
        size: u64,
        expected: [u8; 32],
        root: Option<&std::path::Path>,
    ) -> Result<Self> {
        // Bilgisayar adı alt klasörü seçilen kökün altında da korunur:
        // iki öğrenciden gelen aynı adlı dosya birbirini ezmemeli.
        let dir = downloads_dir(root)?.join(sanitize_component(hostname));
        std::fs::create_dir_all(&dir)
            .with_context(|| format!("klasör oluşturulamadı: {}", dir.display()))?;

        let safe = sanitize_component(name);
        let final_path = unique_path(&dir, &safe);
        let part_path = dir.join(format!(".{}.part", request_id.simple()));
        let file = File::create(&part_path)
            .with_context(|| format!("dosya oluşturulamadı: {}", part_path.display()))?;

        Ok(Self {
            request_id,
            name: final_path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or(safe),
            size,
            received: 0,
            expected,
            hasher: Sha256::new(),
            writer: BufWriter::with_capacity(256 * 1024, file),
            part_path,
            final_path,
            next_seq: 0,
        })
    }

    pub fn write_chunk(&mut self, seq: u32, data: &[u8]) -> Result<()> {
        if seq != self.next_seq {
            bail!(
                "parça sırası bozuk: {} beklenirken {} geldi",
                self.next_seq,
                seq
            );
        }
        if self.received + data.len() as u64 > self.size {
            bail!("gelen veri bildirilen boyutu aştı");
        }
        self.writer.write_all(data).context("dosyaya yazılamadı")?;
        self.hasher.update(data);
        self.received += data.len() as u64;
        self.next_seq = self.next_seq.wrapping_add(1);
        Ok(())
    }

    /// Özeti doğrular ve dosyayı son adına taşır.
    pub fn finish(mut self) -> Result<PathBuf> {
        self.writer.flush().context("dosya kapatılamadı")?;
        drop(self.writer);

        if self.received != self.size {
            let _ = std::fs::remove_file(&self.part_path);
            bail!("dosya eksik geldi ({} / {} bayt)", self.received, self.size);
        }

        let actual: [u8; 32] = self.hasher.finalize().into();
        if actual != self.expected {
            let _ = std::fs::remove_file(&self.part_path);
            bail!("dosya özeti eşleşmedi, indirme bozuk");
        }

        std::fs::rename(&self.part_path, &self.final_path)
            .with_context(|| format!("dosya taşınamadı: {}", self.final_path.display()))?;
        Ok(self.final_path)
    }

    /// Yarım kalan indirmeyi temizler.
    pub fn abort(self) {
        drop(self.writer);
        let _ = std::fs::remove_file(&self.part_path);
    }
}

/// İndirilen dosyaların yazıldığı klasör.
///
/// `custom` verilmişse o klasör kullanılır — öğretmen indirmeleri kendi
/// seçtiği yere toplayabilir. Verilmezse `Belgeler\OpenClass\Toplanan`
/// kullanılır.
///
/// Seçilen klasör öğretmenin **kendi** makinesindedir, uzak bir yol değil;
/// bu yüzden serbest yol kabul edilir. Yine de var olup olmadığı
/// doğrulanır: silinmiş bir klasöre indirmeye kalkıp aktarımın yarısında
/// patlamak yerine baştan varsayılana düşmek daha iyidir.
pub fn downloads_dir(custom: Option<&std::path::Path>) -> Result<PathBuf> {
    if let Some(kok) = custom {
        if kok.is_dir() {
            return Ok(kok.to_path_buf());
        }
        tracing::warn!(
            klasor = %kok.display(),
            "seçilen indirme klasörü yok, varsayılana dönülüyor"
        );
    }

    let base = directories::UserDirs::new()
        .and_then(|u| u.document_dir().map(std::path::Path::to_path_buf))
        .map(|d| d.join("OpenClass").join("Toplanan"))
        .map_or_else(
            || openclass_common::paths::data_dir(openclass_common::paths::App::Teacher),
            Ok,
        )?;
    std::fs::create_dir_all(&base)
        .with_context(|| format!("klasör oluşturulamadı: {}", base.display()))?;
    Ok(base)
}

/// Bir yol bileşenini güvenli hâle getirir.
///
/// Karşı taraftan gelen ad güvenilmezdir: yol ayırıcısı veya `..` içeren
/// bir ad, dosyayı hedef klasörün dışına yazdırabilirdi.
pub fn sanitize_component(raw: &str) -> String {
    let base = raw
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(raw)
        .trim()
        .trim_start_matches('.');

    let cleaned: String = base
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '|' | '?' | '*' | '\0' => '_',
            c if (c as u32) < 0x20 => '_',
            c => c,
        })
        .collect();

    let cleaned = cleaned.trim_matches(['.', ' ']).to_string();
    if cleaned.is_empty() {
        return "dosya".to_string();
    }
    if cleaned.chars().count() > 150 {
        return cleaned.chars().take(150).collect();
    }
    cleaned
}

/// Var olan bir adı ezmemek için sonuna sayı ekler.
pub fn unique_path(dir: &std::path::Path, name: &str) -> PathBuf {
    let candidate = dir.join(name);
    if !candidate.exists() {
        return candidate;
    }
    let (stem, ext) = match name.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() => (s.to_string(), format!(".{e}")),
        _ => (name.to_string(), String::new()),
    };
    for n in 2..1000 {
        let candidate = dir.join(format!("{stem} ({n}){ext}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    dir.join(format!("{stem} ({}){ext}", Uuid::new_v4().simple()))
}

/// Arayüze gönderilen indirme durumu.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadView {
    pub name: String,
    pub size: u64,
    pub received: u64,
    pub finished: bool,
    pub ok: bool,
    pub detail: String,
}

/// Açık gezgin oturumu.
pub struct BrowseSession {
    pub device_id: Uuid,
    pub hostname: String,
    /// Görüntülenen yol; boşsa kök listesi.
    pub path: String,
    pub roots: Vec<BrowseRoot>,
    pub entries: Vec<BrowseEntry>,
    pub loading: bool,
    pub error: Option<String>,
    /// Yürüyen indirme (alıcı durumu).
    pub download: Option<Download>,
    /// Arayüze gösterilen son indirme sonucu.
    pub download_view: Option<DownloadView>,
}

impl BrowseSession {
    pub fn new(device_id: Uuid, hostname: String) -> Self {
        Self {
            device_id,
            hostname,
            path: String::new(),
            roots: Vec::new(),
            entries: Vec::new(),
            loading: true,
            error: None,
            download: None,
            download_view: None,
        }
    }
}

/// Arayüze gönderilen gezgin görünümü.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowseView {
    pub device_id: String,
    pub hostname: String,
    pub path: String,
    pub roots: Vec<BrowseRoot>,
    pub entries: Vec<BrowseEntry>,
    pub loading: bool,
    pub error: Option<String>,
    pub download: Option<DownloadView>,
}

impl From<&BrowseSession> for BrowseView {
    fn from(s: &BrowseSession) -> Self {
        Self {
            device_id: s.device_id.to_string(),
            hostname: s.hostname.clone(),
            path: s.path.clone(),
            roots: s.roots.clone(),
            entries: s.entries.clone(),
            loading: s.loading,
            error: s.error.clone(),
            download: s.download_view.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yol_bilesenleri_temizlenir() {
        assert_eq!(sanitize_component("../../etc/passwd"), "passwd");
        assert_eq!(sanitize_component(r"C:\Windows\x.exe"), "x.exe");
        assert_eq!(sanitize_component("LAB101"), "LAB101");
    }

    #[test]
    fn bos_ad_varsayilana_duser() {
        assert_eq!(sanitize_component("  "), "dosya");
        assert_eq!(sanitize_component("///"), "dosya");
    }

    #[test]
    fn ayni_ad_uzerine_yazilmaz() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("odev.docx"), b"x").unwrap();
        let p = unique_path(dir.path(), "odev.docx");
        assert_eq!(p.file_name().unwrap(), "odev (2).docx");
    }

    #[test]
    fn bozuk_ozet_dosyayi_birakmaz() {
        let dir = tempfile::tempdir().unwrap();
        let part = dir.path().join(".x.part");
        let final_path = dir.path().join("a.bin");
        let file = File::create(&part).unwrap();

        let mut d = Download {
            request_id: Uuid::new_v4(),
            name: "a.bin".into(),
            size: 3,
            received: 0,
            // Kasıtlı olarak yanlış özet.
            expected: [0xAA; 32],
            hasher: Sha256::new(),
            writer: BufWriter::new(file),
            part_path: part.clone(),
            final_path: final_path.clone(),
            next_seq: 0,
        };
        d.write_chunk(0, b"abc").unwrap();

        // Özet eşleşmediği için dosya yerine taşınmaz ve geçici dosya silinir.
        assert!(d.finish().is_err());
        assert!(!final_path.exists(), "bozuk dosya bırakılmamalı");
        assert!(!part.exists(), "geçici dosya temizlenmeli");
    }

    #[test]
    fn eksik_gelen_dosya_birakilmaz() {
        let dir = tempfile::tempdir().unwrap();
        let part = dir.path().join(".y.part");
        let final_path = dir.path().join("b.bin");
        let file = File::create(&part).unwrap();

        let mut d = Download {
            request_id: Uuid::new_v4(),
            name: "b.bin".into(),
            // 10 bayt bildirildi ama 3 bayt geldi.
            size: 10,
            received: 0,
            expected: [0; 32],
            hasher: Sha256::new(),
            writer: BufWriter::new(file),
            part_path: part.clone(),
            final_path: final_path.clone(),
            next_seq: 0,
        };
        d.write_chunk(0, b"abc").unwrap();

        assert!(d.finish().is_err());
        assert!(!final_path.exists());
        assert!(!part.exists());
    }

    #[test]
    fn dogru_ozet_dosyayi_yerine_tasir() {
        let dir = tempfile::tempdir().unwrap();
        let part = dir.path().join(".z.part");
        let final_path = dir.path().join("c.bin");
        let file = File::create(&part).unwrap();

        let expected: [u8; 32] = Sha256::digest(b"abc").into();
        let mut d = Download {
            request_id: Uuid::new_v4(),
            name: "c.bin".into(),
            size: 3,
            received: 0,
            expected,
            hasher: Sha256::new(),
            writer: BufWriter::new(file),
            part_path: part.clone(),
            final_path: final_path.clone(),
            next_seq: 0,
        };
        d.write_chunk(0, b"abc").unwrap();

        let saved = d.finish().unwrap();
        assert_eq!(saved, final_path);
        assert_eq!(std::fs::read(&final_path).unwrap(), b"abc");
        assert!(!part.exists());
    }

    #[test]
    fn parca_sirasi_bozuksa_hata_verir() {
        let dir = tempfile::tempdir().unwrap();
        let part = dir.path().join(".x.part");
        let file = File::create(&part).unwrap();
        let mut d = Download {
            request_id: Uuid::new_v4(),
            name: "a.bin".into(),
            size: 10,
            received: 0,
            expected: [0; 32],
            hasher: Sha256::new(),
            writer: BufWriter::new(file),
            part_path: part,
            final_path: dir.path().join("a.bin"),
            next_seq: 0,
        };
        assert!(d.write_chunk(0, b"abc").is_ok());
        // 1 beklenirken 5 gelirse reddedilir.
        assert!(d.write_chunk(5, b"xyz").is_err());
        // Bildirilen boyutu aşan veri reddedilir.
        assert!(d.write_chunk(1, &[0u8; 20]).is_err());
    }
}
