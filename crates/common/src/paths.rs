//! Uygulama veri dizinleri.
//!
//! Windows'ta her şey `%APPDATA%\OpenClass\<uygulama>\` altında toplanır.
//! Geliştirme sırasında Linux'ta `~/.local/share/openclass/<uygulama>/` kullanılır.

use std::path::PathBuf;

use anyhow::{Context, Result};

/// Uygulama türü — veri dizinleri bu sayede ayrışır.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum App {
    Teacher,
    Student,
}

impl App {
    fn dir_name(self) -> &'static str {
        match self {
            Self::Teacher => "teacher",
            Self::Student => "student",
        }
    }
}

/// Veri dizinini geçersiz kılan geliştirme değişkeni.
///
/// Yalnızca test içindir: aynı makinede birden fazla öğrenci ajanının ayrı
/// cihaz kimliği ve yapılandırması kullanabilmesi için
/// `OPENCLASS_DATA_DIR=/tmp/ogrenci-1` verilebilir.
pub const DATA_DIR_OVERRIDE_VAR: &str = "OPENCLASS_DATA_DIR";

/// Uygulamanın kök veri dizini. Yoksa oluşturulur.
pub fn data_dir(app: App) -> Result<PathBuf> {
    let base = match std::env::var(DATA_DIR_OVERRIDE_VAR) {
        Ok(root) if !root.trim().is_empty() => PathBuf::from(root.trim()).join(app.dir_name()),
        _ => directories::ProjectDirs::from("io", "OpenClass", "OpenClass")
            .context("kullanıcı veri dizini belirlenemedi")?
            .data_dir()
            .join(app.dir_name()),
    };
    std::fs::create_dir_all(&base)
        .with_context(|| format!("veri dizini oluşturulamadı: {}", base.display()))?;
    Ok(base)
}

/// Log dosyalarının dizini.
pub fn log_dir(app: App) -> Result<PathBuf> {
    let dir = data_dir(app)?.join("logs");
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("log dizini oluşturulamadı: {}", dir.display()))?;
    Ok(dir)
}

/// SQLite veritabanı dosyası (yalnızca öğretmen uygulaması kullanır).
pub fn database_file(app: App) -> Result<PathBuf> {
    Ok(data_dir(app)?.join("openclass.db"))
}

/// Yapılandırma dosyası.
pub fn config_file(app: App) -> Result<PathBuf> {
    Ok(data_dir(app)?.join("config.json"))
}

/// Gizli materyalin (TLS özel anahtarı, cihaz tohumu) saklandığı dizin.
pub fn secrets_dir(app: App) -> Result<PathBuf> {
    let dir = data_dir(app)?.join("secrets");
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("gizli dizin oluşturulamadı: {}", dir.display()))?;
    restrict_permissions(&dir)?;
    Ok(dir)
}

/// Dizini yalnızca sahibine açık hâle getirir.
///
/// Windows'ta koruma DPAPI ile içerik şifreleme üzerinden sağlanır
/// (bkz. [`crate::secrets`]); dosya sistemi izinleri oradaki asıl savunmaya ektir.
fn restrict_permissions(path: &std::path::Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(path)?.permissions();
        perms.set_mode(0o700);
        std::fs::set_permissions(path, perms)?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}
