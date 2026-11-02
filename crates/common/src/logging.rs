//! Yapılandırılabilir loglama.
//!
//! Loglar hem konsola hem günlük döndürülen dosyalara yazılır. Disk kullanımı
//! `MAX_LOG_FILES` ile sınırlıdır — eski dosyalar otomatik silinir.
//!
//! # Hassas veri kuralı
//!
//! Parolalar, eşleştirme kodları, özel anahtarlar, cihaz tohumları ve ekran
//! içerikleri **asla** loglanmaz. Kimlik bilgisi taşıyan alanlar `tracing`
//! çağrılarına hiç verilmez; gerekiyorsa yalnızca parmak izinin ilk baytları
//! gibi tanıtıcı özetler yazılır.

use anyhow::{Context, Result};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

use crate::paths::{self, App};

/// Diskte tutulacak azami günlük log dosyası sayısı.
const MAX_LOG_FILES: usize = 14;

/// `OPENCLASS_LOG` ortam değişkeni ile geçersiz kılınabilen varsayılan seviye.
const DEFAULT_FILTER: &str =
    "info,openclass_teacher=debug,openclass_student=debug,openclass_protocol=debug";

/// Loglamayı başlatır.
///
/// Dönen [`WorkerGuard`] uygulamanın ömrü boyunca canlı tutulmalıdır; düşerse
/// arka plan yazıcı kapanır ve son loglar diske inmez.
pub fn init(app: App) -> Result<WorkerGuard> {
    let dir = paths::log_dir(app)?;

    let file_appender = tracing_appender::rolling::Builder::new()
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .filename_prefix("openclass")
        .filename_suffix("log")
        .max_log_files(MAX_LOG_FILES)
        .build(&dir)
        .with_context(|| format!("log dosyası açılamadı: {}", dir.display()))?;

    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let filter =
        EnvFilter::try_from_env("OPENCLASS_LOG").unwrap_or_else(|_| EnvFilter::new(DEFAULT_FILTER));

    tracing_subscriber::registry()
        .with(filter)
        .with(
            fmt::layer()
                .with_target(true)
                .with_ansi(false)
                .with_writer(non_blocking),
        )
        .with(fmt::layer().with_target(true).with_writer(std::io::stderr))
        .init();

    tracing::info!(
        uygulama = ?app,
        surum = crate::APP_VERSION,
        log_dizini = %dir.display(),
        "OpenClass başlatıldı"
    );

    Ok(guard)
}
