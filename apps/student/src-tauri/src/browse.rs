//! Öğretmenin öğrenci dosyalarını listelemesi ve indirmesi.
//!
//! # Yapılmayanlar
//!
//! Bu modülde **yazma, silme, taşıma, yeniden adlandırma ve çalıştırma
//! yoktur.** Protokolde böyle bir mesaj da tanımlı değil. Öğretmenin bir
//! öğrencinin dosyasını değiştirebilmesi dersle ilgili bir ihtiyaç değil;
//! ters giden her şeyin bedelini öğrenci öder.
//!
//! # Kapsam
//!
//! Gezilebilen yerler [`roots`] ile sınırlıdır: Masaüstü, Belgeler,
//! İndirilenler ve OpenClass'ın gelen dosya klasörü. Tüm disk açılmaz —
//! `C:\Windows` veya başka bir kullanıcının profilinde dolaşmanın dersle
//! ilgisi yok.
//!
//! Her yol [`resolve`] ile köke hapsedilir. `..`, sembolik bağ ve kavşak
//! (junction) ile kapsam dışına çıkma denemeleri burada durur: yol
//! `canonicalize` edilir ve gerçekten bir kökün altında olduğu doğrulanır.
//! Bu yüzden kontrol **adın şekline değil, çözülmüş gerçek yola** bakar.
//!
//! # Görünürlük
//!
//! Gezgin açıkken öğrencide kapatılamaz bir gösterge durur ve her indirme
//! ayrıca bildirilir. Arkadan dosya toplamak bu ürünün yapmadığı bir şey.

use std::path::{Component, Path, PathBuf};

use anyhow::{bail, Context, Result};
use openclass_protocol::{BrowseEntry, BrowseRoot};

/// Tek bir listelemede döndürülecek en fazla girdi.
///
/// On binlerce dosyalı bir klasör mesajı şişirir ve arayüzü kilitler.
/// Sınır aşılırsa liste kesilir; öğretmen alt klasöre inerek devam eder.
pub const MAX_ENTRIES: usize = 2_000;

/// İndirilebilecek en büyük dosya.
pub const MAX_FETCH_SIZE: u64 = 512 * 1024 * 1024;

/// Gezilmesine izin verilen kökler.
///
/// Var olmayan klasörler listeye girmez: öğretmene tıklayınca hata veren
/// bir giriş göstermenin anlamı yok.
pub fn roots() -> Vec<(String, PathBuf)> {
    let mut out: Vec<(String, PathBuf)> = Vec::new();

    if let Some(dirs) = directories::UserDirs::new() {
        let candidates: [(&str, Option<&Path>); 3] = [
            ("Masaüstü", dirs.desktop_dir()),
            ("Belgeler", dirs.document_dir()),
            ("İndirilenler", dirs.download_dir()),
        ];
        for (label, path) in candidates {
            if let Some(path) = path {
                if path.is_dir() {
                    out.push((label.to_string(), path.to_path_buf()));
                }
            }
        }
    }

    // Öğretmenin kendi gönderdiği dosyalar; geri almak isteyebilir.
    //
    // Yalnızca varsayılan (Belgeler) kök ayrı bir giriş olarak eklenir.
    // Masaüstü ve İndirilenler'e gönderilmiş dosyalar, o kökler zaten
    // listede olduğu için kendi `OpenClass` alt klasörlerinden görülür.
    if let Ok(inbox) = crate::transfer::inbox_dir(Default::default(), None) {
        if inbox.is_dir() && !out.iter().any(|(_, p)| *p == inbox) {
            out.push(("OpenClass".to_string(), inbox));
        }
    }

    out
}

/// Öğretmene gönderilecek kök listesi.
pub fn root_views() -> Vec<BrowseRoot> {
    roots()
        .into_iter()
        .map(|(label, path)| BrowseRoot {
            label,
            path: path.to_string_lossy().into_owned(),
        })
        .collect()
}

/// Gelen yolu doğrular ve gerçek yola çevirir.
///
/// # Hata
///
/// Yol bir kökün altında değilse, çözülemiyorsa veya göreli bileşen
/// içeriyorsa reddedilir. Kontrol `canonicalize` sonrası yapılır; böylece
/// sembolik bağla kapsam dışına atlamak işe yaramaz.
pub fn resolve(raw: &str) -> Result<PathBuf> {
    let raw = raw.trim();
    if raw.is_empty() {
        bail!("Yol boş.");
    }

    let candidate = PathBuf::from(raw);
    if !candidate.is_absolute() {
        bail!("Yalnızca tam yol kabul edilir.");
    }
    // Hızlı ret: açıkça göreli bileşen içeren yollar hiç çözülmeye değmez.
    if candidate
        .components()
        .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
    {
        bail!("Yol göreli bileşen içeriyor.");
    }

    let real = candidate
        .canonicalize()
        .with_context(|| format!("yol çözülemedi: {raw}"))?;

    let allowed = roots();
    let inside = allowed.iter().any(|(_, root)| {
        root.canonicalize()
            .map(|r| real == r || real.starts_with(&r))
            .unwrap_or(false)
    });

    if !inside {
        bail!("Bu konum paylaşım kapsamı dışında.");
    }
    Ok(real)
}

/// Bir klasörün içeriğini listeler.
///
/// Okunamayan girdiler atlanır; tek bozuk dosya yüzünden listenin tamamını
/// kaybetmek işe yaramaz.
pub fn list(dir: &Path) -> Result<Vec<BrowseEntry>> {
    if !dir.is_dir() {
        bail!("Klasör değil.");
    }

    let reader =
        std::fs::read_dir(dir).with_context(|| format!("klasör okunamadı: {}", dir.display()))?;

    let mut entries: Vec<BrowseEntry> = Vec::new();
    for item in reader.flatten() {
        if entries.len() >= MAX_ENTRIES {
            break;
        }
        let Ok(meta) = item.metadata() else { continue };
        let name = item.file_name().to_string_lossy().into_owned();

        entries.push(BrowseEntry {
            name,
            is_dir: meta.is_dir(),
            size: if meta.is_dir() { 0 } else { meta.len() },
            modified: meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0),
        });
    }

    // Klasörler önce, sonra ada göre — gezinmeyi kolaylaştırır.
    entries.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(entries)
}

/// İndirilecek dosyayı doğrular.
///
/// Boyut sınırı ve "gerçekten dosya mı" kontrolü okumadan önce yapılır.
pub fn check_fetch(path: &Path) -> Result<u64> {
    let meta =
        std::fs::metadata(path).with_context(|| format!("dosya okunamadı: {}", path.display()))?;
    if !meta.is_file() {
        bail!("Yalnızca dosya indirilebilir.");
    }
    let size = meta.len();
    if size == 0 {
        bail!("Dosya boş.");
    }
    if size > MAX_FETCH_SIZE {
        bail!(
            "Dosya çok büyük ({:.1} MB); en fazla {} MB indirilebilir.",
            size as f64 / 1_048_576.0,
            MAX_FETCH_SIZE / 1_048_576
        );
    }
    Ok(size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn goreli_ve_bos_yollar_reddedilir() {
        assert!(resolve("").is_err());
        assert!(resolve("   ").is_err());
        // Göreli yol kabul edilmez.
        assert!(resolve("belgeler/ders.pdf").is_err());
    }

    #[test]
    fn goreli_bilesen_iceren_tam_yol_reddedilir() {
        // Kök altında başlasa bile `..` ile yukarı çıkma denemesi durur.
        assert!(resolve("/home/kullanici/Belgeler/../../etc/passwd").is_err());
        assert!(resolve("/home/./kullanici").is_err());
    }

    #[test]
    fn kapsam_disi_yol_reddedilir() {
        // Hiçbir kökün altında olmayan bir sistem yolu.
        //
        // Hangi gerekçeyle reddedildiği platforma göre değişir: Windows'ta
        // "/etc" tam yol sayılmaz, Linux'ta çözülür ama kapsam dışı kalır.
        // Önemli olan kabul edilmemesi.
        let yol = if cfg!(windows) {
            r"C:\Windows\System32"
        } else {
            "/etc"
        };
        assert!(resolve(yol).is_err(), "{yol} kabul edilmemeliydi");
    }

    #[test]
    fn klasor_olmayan_yol_listelenemez() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.txt");
        std::fs::write(&file, b"x").unwrap();
        assert!(list(&file).is_err());
    }

    #[test]
    fn listeleme_klasorleri_basa_alir_ve_siralar() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("zebra.txt"), b"x").unwrap();
        std::fs::write(dir.path().join("Alfa.txt"), b"x").unwrap();
        std::fs::create_dir(dir.path().join("mmm")).unwrap();

        let entries = list(dir.path()).unwrap();
        assert_eq!(entries[0].name, "mmm");
        assert!(entries[0].is_dir);
        // Dosyalar büyük/küçük harfe bakılmadan sıralanır.
        assert_eq!(entries[1].name, "Alfa.txt");
        assert_eq!(entries[2].name, "zebra.txt");
    }

    #[test]
    fn klasorun_boyutu_sifir_bildirilir() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("alt")).unwrap();
        let entries = list(dir.path()).unwrap();
        assert_eq!(entries[0].size, 0);
    }

    #[test]
    fn bos_ve_cok_buyuk_dosya_indirilemez() {
        let dir = tempfile::tempdir().unwrap();
        let bos = dir.path().join("bos.txt");
        std::fs::write(&bos, b"").unwrap();
        assert!(check_fetch(&bos).is_err());

        // Klasör indirilemez.
        assert!(check_fetch(dir.path()).is_err());

        let dolu = dir.path().join("dolu.txt");
        std::fs::write(&dolu, b"abc").unwrap();
        assert_eq!(check_fetch(&dolu).unwrap(), 3);
    }
}
