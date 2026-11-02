//! Öğretmenden gelen dosyaların alınması.
//!
//! # Sessiz bırakma yok
//!
//! Her aktarım öğrenciye bildirilir: teklif geldiğinde, tamamlandığında ve
//! başarısız olduğunda. Makineye arkadan dosya bırakmak bu ürünün yapmadığı
//! bir şeydir.
//!
//! # Sabit hedef klasör
//!
//! Dosyalar yalnızca [`inbox_dir`] altına yazılır. Öğretmen hedef yol
//! seçemez: aksi hâlde sistem dizinlerine yazma kapısı açılırdı. Gelen ad
//! [`sanitize_name`] ile temizlenir; yol bileşeni, sürücü harfi ve
//! `..` kabul edilmez.
//!
//! # Doğrulama
//!
//! Parçalar geçici `.part` dosyasına yazılır. Yalnızca SHA-256 özeti
//! eşleşirse son ada taşınır; böylece yarım veya bozuk bir dosya hiçbir
//! zaman tamamlanmış gibi görünmez.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// Kabul edilen en büyük dosya boyutu.
///
/// Okul laboratuvarında ders materyali birkaç on megabayttır. Üst sınır
/// hem diski hem de yanlışlıkla gönderilen dev dosyaları engeller.
pub const MAX_FILE_SIZE: u64 = 512 * 1024 * 1024;

/// Yazmaya başlamak için gereken en az boş disk payı.
///
/// Dosya boyutunun üstüne bu kadar daha yer olmalı; diski son bayta kadar
/// doldurmak makineyi kullanılamaz hâle getirir.
pub const DISK_HEADROOM: u64 = 256 * 1024 * 1024;

/// Gelen dosyaların yazıldığı klasör.
///
/// Kullanıcının **Belgeler** klasörü altına yazılır, uygulamanın gizli veri
/// dizinine değil: öğrenci gelen dosyayı kendi başına bulabilmeli.
/// Belgeler klasörü okunamazsa veri dizinine düşülür.
pub fn inbox_dir() -> Result<PathBuf> {
    let base = directories::UserDirs::new()
        .and_then(|u| u.document_dir().map(Path::to_path_buf))
        .map(|d| d.join("OpenClass"))
        .map_or_else(
            || openclass_common::paths::data_dir(openclass_common::paths::App::Student),
            Ok,
        )?;

    std::fs::create_dir_all(&base)
        .with_context(|| format!("klasör oluşturulamadı: {}", base.display()))?;
    Ok(base)
}

/// Dosya adını güvenli bir ada indirger.
///
/// Yol ayırıcıları, sürücü harfi, `..` ve Windows'ta yasak karakterler
/// temizlenir. Sonuç her zaman tek bir dosya adıdır.
pub fn sanitize_name(raw: &str) -> String {
    // Yalnızca son bileşeni al: "..\\..\\Windows\\x.dll" → "x.dll"
    let base = raw
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(raw)
        .trim()
        .trim_start_matches('.');

    let cleaned: String = base
        .chars()
        .map(|c| match c {
            // Windows'ta dosya adında yasak olan karakterler.
            '<' | '>' | ':' | '"' | '|' | '?' | '*' | '\0' => '_',
            c if (c as u32) < 0x20 => '_',
            c => c,
        })
        .collect();

    let cleaned = cleaned.trim_matches(['.', ' ']).to_string();
    if cleaned.is_empty() {
        return "dosya".to_string();
    }

    // Windows'un ayrılmış aygıt adları dosya adı olarak kullanılamaz.
    const RESERVED: [&str; 22] = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    let stem = cleaned.split('.').next().unwrap_or("").to_ascii_uppercase();
    if RESERVED.contains(&stem.as_str()) {
        return format!("_{cleaned}");
    }

    // Aşırı uzun adlar dosya sistemi sınırını aşabilir.
    if cleaned.chars().count() > 150 {
        let short: String = cleaned.chars().take(150).collect();
        return short;
    }
    cleaned
}

/// Var olan bir adı ezmemek için sonuna sayı ekler.
///
/// `ders.pdf` varsa `ders (2).pdf` denenir. Öğretmenin gönderdiği dosya
/// öğrencinin kendi dosyasının üzerine yazmamalı.
pub fn unique_path(dir: &Path, name: &str) -> PathBuf {
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

/// Yürüyen bir alım.
pub struct Incoming {
    pub transfer_id: Uuid,
    /// Öğrenciye gösterilecek temizlenmiş ad.
    pub name: String,
    pub size: u64,
    expected: [u8; 32],
    hasher: Sha256,
    writer: BufWriter<File>,
    part_path: PathBuf,
    final_path: PathBuf,
    /// Şimdiye kadar yazılan bayt.
    pub received: u64,
    /// Beklenen sonraki parça numarası; boşluk tespiti için.
    next_seq: u32,
}

impl Incoming {
    /// Teklifi değerlendirir ve kabul edilirse yazmaya hazırlanır.
    ///
    /// Boyut sınırı ve disk payı burada kontrol edilir: reddedilecek bir
    /// aktarıma tek bayt bile yazılmaz.
    pub fn open(transfer_id: Uuid, raw_name: &str, size: u64, expected: [u8; 32]) -> Result<Self> {
        if size == 0 {
            bail!("Boş dosya gönderilemez.");
        }
        if size > MAX_FILE_SIZE {
            bail!(
                "Dosya çok büyük ({:.1} MB); en fazla {} MB kabul ediliyor.",
                size as f64 / 1_048_576.0,
                MAX_FILE_SIZE / 1_048_576
            );
        }

        let dir = inbox_dir()?;

        if let Some(free) = free_space(&dir) {
            if free < size.saturating_add(DISK_HEADROOM) {
                bail!(
                    "Diskte yeterli yer yok ({:.1} MB boş, {:.1} MB gerekiyor).",
                    free as f64 / 1_048_576.0,
                    (size + DISK_HEADROOM) as f64 / 1_048_576.0
                );
            }
        }

        let name = sanitize_name(raw_name);
        let final_path = unique_path(&dir, &name);
        // Geçici ad aktarım kimliğini taşır: iki aktarım çakışmaz.
        let part_path = dir.join(format!(".{}.part", transfer_id.simple()));
        let file = File::create(&part_path)
            .with_context(|| format!("dosya oluşturulamadı: {}", part_path.display()))?;

        Ok(Self {
            transfer_id,
            name: final_path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or(name),
            size,
            expected,
            hasher: Sha256::new(),
            writer: BufWriter::with_capacity(256 * 1024, file),
            part_path,
            final_path,
            received: 0,
            next_seq: 0,
        })
    }

    /// Bir parçayı yazar.
    pub fn write_chunk(&mut self, seq: u32, data: &[u8]) -> Result<()> {
        if seq != self.next_seq {
            bail!(
                "parça sırası bozuk: {} beklenirken {} geldi",
                self.next_seq,
                seq
            );
        }
        if self.received + data.len() as u64 > self.size {
            bail!("gönderilen veri bildirilen boyutu aştı");
        }

        self.writer.write_all(data).context("dosyaya yazılamadı")?;
        self.hasher.update(data);
        self.received += data.len() as u64;
        self.next_seq = self.next_seq.wrapping_add(1);
        Ok(())
    }

    /// Aktarımı bitirir: özeti doğrular ve dosyayı son adına taşır.
    ///
    /// Özet eşleşmezse geçici dosya silinir. Bozuk bir dosyayı öğrencinin
    /// klasöründe bırakmak, hiç bırakmamaktan kötüdür.
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
            bail!("dosya özeti eşleşmedi, aktarım bozuk");
        }

        std::fs::rename(&self.part_path, &self.final_path)
            .with_context(|| format!("dosya yerine taşınamadı: {}", self.final_path.display()))?;
        Ok(self.final_path)
    }

    /// Yarım kalan aktarımı temizler.
    pub fn abort(self) {
        drop(self.writer);
        let _ = std::fs::remove_file(&self.part_path);
    }
}

/// Klasörün bulunduğu birimdeki boş alan.
///
/// Öğrenilemezse `None` döner ve kontrol atlanır: disk payını okuyamamak
/// aktarımı reddetmek için yeterli sebep değil.
#[cfg(windows)]
fn free_space(path: &Path) -> Option<u64> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;

    let wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let mut free: u64 = 0;
    let ok = unsafe {
        GetDiskFreeSpaceExW(
            wide.as_ptr(),
            &mut free,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };

    if ok == 0 {
        None
    } else {
        Some(free)
    }
}

/// Windows dışında boş alan sorgulanmaz; kontrol atlanır.
#[cfg(not(windows))]
fn free_space(_path: &Path) -> Option<u64> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yol_bilesenleri_temizlenir() {
        assert_eq!(sanitize_name("../../Windows/System32/evil.dll"), "evil.dll");
        assert_eq!(sanitize_name(r"C:\Windows\x.exe"), "x.exe");
        assert_eq!(sanitize_name("ders.pdf"), "ders.pdf");
    }

    #[test]
    fn yasak_karakterler_degistirilir() {
        assert_eq!(sanitize_name("rapor<>:\"|?*.txt"), "rapor_______.txt");
    }

    #[test]
    fn gizli_dosya_adi_uretilemez() {
        // Baştaki noktalar atılır; ".bashrc" üzerine yazılamaz.
        assert_eq!(sanitize_name("...gizli"), "gizli");
    }

    #[test]
    fn bos_ad_varsayilana_duser() {
        assert_eq!(sanitize_name("   "), "dosya");
        assert_eq!(sanitize_name("..."), "dosya");
        assert_eq!(sanitize_name("/"), "dosya");
    }

    #[test]
    fn windows_aygit_adlari_kacirilir() {
        assert_eq!(sanitize_name("CON"), "_CON");
        assert_eq!(sanitize_name("lpt1.txt"), "_lpt1.txt");
        // Aygıt adı olmayan benzer adlar dokunulmadan geçer.
        assert_eq!(sanitize_name("console.log"), "console.log");
    }

    #[test]
    fn uzun_ad_kisaltilir() {
        let uzun = "a".repeat(400);
        assert_eq!(sanitize_name(&uzun).chars().count(), 150);
    }

    #[test]
    fn ayni_ad_uzerine_yazilmaz() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("ders.pdf"), b"x").unwrap();

        let p = unique_path(dir.path(), "ders.pdf");
        assert_eq!(p.file_name().unwrap(), "ders (2).pdf");

        // Uzantısız ad da doğru numaralanır.
        std::fs::write(dir.path().join("notlar"), b"x").unwrap();
        let p = unique_path(dir.path(), "notlar");
        assert_eq!(p.file_name().unwrap(), "notlar (2)");
    }

    #[test]
    fn bos_ve_cok_buyuk_dosya_reddedilir() {
        let id = Uuid::new_v4();
        assert!(Incoming::open(id, "a.bin", 0, [0; 32]).is_err());
        assert!(Incoming::open(id, "a.bin", MAX_FILE_SIZE + 1, [0; 32]).is_err());
    }
}
