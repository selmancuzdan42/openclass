//! Çökme kaydı.
//!
//! Uygulama beklenmedik biçimde durduğunda (Rust `panic`) nedenini küçük bir
//! dosyaya yazar. Sürüm derlemesi `panic = "abort"` ile derlenir; kanca
//! süreç sonlanmadan hemen önce çalışır, bu yüzden kayıt burada alınmazsa
//! çökmenin hiçbir izi kalmaz.
//!
//! # Yalnızca neden, kimlik yok
//!
//! Kayıt gönderilmek üzere tasarlanmıştır, bu yüzden **yalnızca hatanın
//! nedenini** taşır: hata mesajı, koddaki yeri, uygulama ve sürüm, işletim
//! sistemi. Bilgisayar adı, kullanıcı adı, cihaz kimliği ve log satırları
//! kayda **hiç girmez**. Hata mesajının içine sızmış olabilecek IP adresleri,
//! cihaz kimlikleri ve kullanıcı klasörü adları [`sanitize`] ile maskelenir.
//!
//! Yakalanamayanlar: yerel koddaki (WebView, sürücü) bellek hataları ve
//! sürecin dışarıdan sonlandırılması bir `panic` değildir, kanca çalışmaz.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::paths::{self, App};

/// Kayıttaki metin alanlarının azami uzunluğu.
const MAX_TEXT: usize = 2_000;

/// Diskte tutulan azami çökme kaydı; eskiler silinir.
const MAX_REPORTS: usize = 20;

/// Tek bir çökmenin kaydı.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CrashReport {
    /// `teacher` ya da `student`.
    pub app: String,
    pub version: String,
    /// İşletim sistemi ve mimari (`windows x86_64`).
    pub os: String,
    /// Maskelenmiş hata mesajı.
    pub message: String,
    /// Hatanın koddaki yeri (`dosya:satır`), maskelenmiş.
    pub location: String,
    /// Çökme anı (Unix saniye).
    pub at: u64,
}

/// Çökme kancasını kurar. Uygulama açılışında bir kez çağrılır.
pub fn install(app: App) {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let message = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| (*s).to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "bilinmeyen hata".to_string());
        let location = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_default();
        let report = build(app, &message, &location);
        tracing::error!(mesaj = %report.message, yer = %report.location, "uygulama çöktü");
        // Kancanın kendisi asla çökmemeli; yazılamazsa sessizce geçilir.
        if let Ok(dir) = reports_dir(app) {
            let _ = write(&dir, &report);
        }
        previous(info);
    }));
}

/// Gönderilmeyi bekleyen kayıtlar, eskiden yeniye.
pub fn pending(app: App) -> Vec<(PathBuf, CrashReport)> {
    reports_dir(app)
        .map(|dir| read_all(&dir))
        .unwrap_or_default()
}

fn build(app: App, message: &str, location: &str) -> CrashReport {
    CrashReport {
        app: match app {
            App::Teacher => "teacher",
            App::Student => "student",
        }
        .to_string(),
        version: crate::APP_VERSION.to_string(),
        os: format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
        message: sanitize(message),
        location: sanitize(location),
        at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
    }
}

fn reports_dir(app: App) -> anyhow::Result<PathBuf> {
    let dir = paths::data_dir(app)?.join("crashes");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn write(dir: &Path, report: &CrashReport) -> std::io::Result<()> {
    let text = serde_json::to_string(report).map_err(std::io::Error::other)?;
    std::fs::write(dir.join(format!("crash-{}.json", report.at)), text)?;
    // Sürekli çöken bir kurulum diski doldurmasın.
    let mut all = read_all(dir);
    while all.len() > MAX_REPORTS {
        let (old, _) = all.remove(0);
        let _ = std::fs::remove_file(old);
    }
    Ok(())
}

fn read_all(dir: &Path) -> Vec<(PathBuf, CrashReport)> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<(PathBuf, CrashReport)> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .filter_map(|p| {
            let report = serde_json::from_str(&std::fs::read_to_string(&p).ok()?).ok()?;
            Some((p, report))
        })
        .collect();
    out.sort_by_key(|(_, r)| r.at);
    out
}

/// Metinden kimlik sayılabilecek parçaları maskeler ve uzunluğu sınırlar.
///
/// * IPv4 adresleri → `<ip>`
/// * UUID'ler (cihaz ve öğretmen kimlikleri) → `<kimlik>`
/// * Kullanıcı klasörü adı (`C:\Users\ad\…`, `/home/ad/…`) → `<kullanıcı>`
pub fn sanitize(text: &str) -> String {
    let masked = mask_user_dirs(&mask_tokens(text));
    masked.chars().take(MAX_TEXT).collect()
}

/// Harf/rakam/nokta/tire dizilerini tek tek inceler; IP ya da UUID olanı maskeler.
fn mask_tokens(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut token = String::new();
    let flush = |token: &mut String, out: &mut String| {
        if is_ipv4(token) {
            out.push_str("<ip>");
        } else if is_uuid(token) {
            out.push_str("<kimlik>");
        } else {
            out.push_str(token);
        }
        token.clear();
    };
    for c in text.chars() {
        if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
            token.push(c);
        } else {
            flush(&mut token, &mut out);
            out.push(c);
        }
    }
    flush(&mut token, &mut out);
    out
}

fn is_ipv4(token: &str) -> bool {
    let parts: Vec<&str> = token.trim_end_matches('.').split('.').collect();
    parts.len() == 4
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.len() <= 3 && p.parse::<u8>().is_ok())
}

fn is_uuid(token: &str) -> bool {
    let parts: Vec<&str> = token.split('-').collect();
    parts.len() == 5
        && parts
            .iter()
            .zip([8, 4, 4, 4, 12])
            .all(|(p, n)| p.len() == n && p.chars().all(|c| c.is_ascii_hexdigit()))
}

/// `Users\<ad>` ve `home/<ad>` yol parçalarındaki adı maskeler.
fn mask_user_dirs(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    loop {
        let hit = ["Users\\", "Users/", "home/"]
            .iter()
            .filter_map(|m| rest.find(m).map(|i| (i, m.len())))
            .min();
        let Some((at, len)) = hit else {
            out.push_str(rest);
            return out;
        };
        let name_start = at + len;
        let name_len = rest[name_start..]
            .find(['\\', '/'])
            .unwrap_or(rest.len() - name_start);
        out.push_str(&rest[..name_start]);
        out.push_str("<kullanıcı>");
        rest = &rest[name_start + name_len..];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ip_adresleri_maskelenir() {
        assert_eq!(
            sanitize("192.168.1.42:7643 adresine bağlanılamadı"),
            "<ip>:7643 adresine bağlanılamadı"
        );
        assert_eq!(sanitize("hedef 10.0.0.5."), "hedef <ip>");
        // Sürüm numarası ve ondalık sayı IP değildir.
        assert_eq!(sanitize("sürüm 0.5.1, oran 1.25"), "sürüm 0.5.1, oran 1.25");
        assert_eq!(sanitize("999.1.1.1"), "999.1.1.1");
    }

    #[test]
    fn cihaz_kimlikleri_maskelenir() {
        assert_eq!(
            sanitize("cihaz 96beeb6b-9f34-4a30-8b1c-ae14ac0fff58 bulunamadı"),
            "cihaz <kimlik> bulunamadı"
        );
    }

    #[test]
    fn kullanici_klasoru_adi_maskelenir() {
        assert_eq!(
            sanitize(r"C:\Users\ayse.yilmaz\AppData\Roaming\OpenClass\config.json okunamadı"),
            r"C:\Users\<kullanıcı>\AppData\Roaming\OpenClass\config.json okunamadı"
        );
        assert_eq!(
            sanitize("/home/selman/.cargo/registry/src/x/lib.rs:42"),
            "/home/<kullanıcı>/.cargo/registry/src/x/lib.rs:42"
        );
    }

    #[test]
    fn nedeni_anlatan_metin_korunur() {
        let msg = "called `Option::unwrap()` on a `None` value";
        assert_eq!(sanitize(msg), msg);
        assert_eq!(
            sanitize("apps/teacher/src-tauri/src/net/server.rs:812"),
            "apps/teacher/src-tauri/src/net/server.rs:812"
        );
    }

    #[test]
    fn uzun_metin_kirpilir() {
        assert_eq!(sanitize(&"a".repeat(5_000)).chars().count(), MAX_TEXT);
    }

    #[test]
    fn kayit_kimlik_alani_tasimaz() {
        let report = build(App::Student, "192.168.1.9 yanıt vermedi", "src/x.rs:1");
        let json = serde_json::to_string(&report).unwrap();
        assert!(!json.contains("192.168"));
        // Alan kümesi sabit: bilgisayar adı, kullanıcı ya da cihaz kimliği yok.
        let keys: Vec<String> = serde_json::from_str::<serde_json::Value>(&json)
            .unwrap()
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect();
        assert_eq!(keys, ["app", "at", "location", "message", "os", "version"]);
    }

    #[test]
    fn kayitlar_yazilir_okunur_ve_sinirlanir() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..(MAX_REPORTS as u64 + 5) {
            let mut r = build(App::Teacher, "hata", "src/x.rs:1");
            r.at = 1_000 + i;
            write(dir.path(), &r).unwrap();
        }
        let all = read_all(dir.path());
        assert_eq!(all.len(), MAX_REPORTS);
        // En eskiler silinmiş, sıra eskiden yeniye.
        assert_eq!(all.first().unwrap().1.at, 1_005);
        assert_eq!(all.last().unwrap().1.at, 1_000 + MAX_REPORTS as u64 + 4);
    }
}
