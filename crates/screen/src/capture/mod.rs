//! Ekran yakalama.
//!
//! Yakalama arka ucu platforma ve ortam değişkenine göre seçilir. Windows
//! dışında **hiçbir sahte görüntü üretilmez**; yakalama desteklenmiyorsa açık
//! bir hata döner ve öğretmen konsolunda "ekran alınamıyor" olarak görünür.
//!
//! # Arka uçlar
//!
//! | Arka uç | Ne zaman | Ne üretir |
//! |---|---|---|
//! | Windows Graphics Capture | Windows, birincil | Gerçek ekran |
//! | DXGI Desktop Duplication | Windows, WGC başarısızsa | Gerçek ekran |
//! | Test deseni | `OPENCLASS_CAPTURE=test` | **Sentetik desen** |
//! | Desteklenmiyor | Diğer platformlar | Hata |
//!
//! Test deseni her platformda çalışır ve kare hattının tamamını Windows
//! olmadan sınamayı sağlar. Adı arayüzde açıkça "Test deseni (gerçek ekran
//! değil)" olarak görünür; hiçbir koşulda gerçek ekran gibi sunulmaz.
//!
//! # Windows güvenlik sınırları
//!
//! Güvenli masaüstü (UAC yükseltme istemi, Ctrl+Alt+Del ekranı, kilit ekranı)
//! bu API'lerle yakalanamaz. Bu bilinçli bir işletim sistemi korumasıdır ve
//! **aşılmaya çalışılmaz**. Bu anlarda kare akışı durur; öğretmen konsolu son
//! kareyi dondurmak yerine durumu bildirir.

use anyhow::Result;

use crate::encode::RawFrame;

mod test_pattern;

#[cfg(windows)]
mod windows_backend;

#[cfg(not(windows))]
mod unsupported;

#[cfg(windows)]
use windows_backend as platform;

#[cfg(not(windows))]
use unsupported as platform;

/// Test deseni arka ucunu açan ortam değişkeni.
pub const CAPTURE_OVERRIDE_VAR: &str = "OPENCLASS_CAPTURE";

/// Yakalama iş parçacığı sona erdiğinde çalışma bayrağını düşüren koruma.
///
/// İş parçacığı normal bitişle de, panikle de sonlanabilir. Bayrak açık
/// kalırsa `is_running()` sonsuza kadar `true` döner; istemci hiç kare
/// gelmediği hâlde hata bildirmez ve öğretmen konsolunda kart sürekli
/// "İlk kare bekleniyor…" gösterir. Bu koruma sessiz başarısızlığı
/// görünür hataya çevirir.
pub(crate) struct RunningGuard(pub(crate) std::sync::Arc<std::sync::atomic::AtomicBool>);

impl Drop for RunningGuard {
    fn drop(&mut self) {
        self.0.store(false, std::sync::atomic::Ordering::Relaxed);
    }
}

/// Çalışan bir yakalama oturumu.
///
/// Arka uç kendi iş parçacığında sürekli kare üretir; en son kare paylaşımlı
/// bir yuvada tutulur. Gönderici tarafın okuma hızı yakalama hızından düşükse
/// aradaki kareler düşer — bellekte birikme olmaz.
pub struct CaptureSession {
    inner: Backend,
}

enum Backend {
    Platform(platform::Session),
    TestPattern(test_pattern::Session),
}

impl CaptureSession {
    /// Birincil ekranın yakalanmasını başlatır.
    ///
    /// `label` yalnızca test deseninde kullanılır: desende yazılır ve rengini
    /// belirler, böylece aynı makinedeki sahte öğrenciler ayırt edilir.
    pub fn start_primary_monitor(label: &str) -> Result<Self> {
        if test_mode_enabled() {
            return Ok(Self {
                inner: Backend::TestPattern(test_pattern::Session::start(label)?),
            });
        }
        Ok(Self {
            inner: Backend::Platform(platform::Session::start_primary_monitor()?),
        })
    }

    /// En son yakalanan kareyi alır ve yuvayı boşaltır.
    ///
    /// Yeni kare yoksa `None` döner — bu bir hata değildir, ekran değişmemiş
    /// olabilir.
    pub fn take_latest(&self) -> Option<RawFrame> {
        match &self.inner {
            Backend::Platform(s) => s.take_latest(),
            Backend::TestPattern(s) => s.take_latest(),
        }
    }

    /// Arka ucun adı (loglama ve arayüzde gösterim için).
    pub fn backend_name(&self) -> &'static str {
        match &self.inner {
            Backend::Platform(s) => s.backend_name(),
            Backend::TestPattern(s) => s.backend_name(),
        }
    }

    /// Yakalama iş parçacığı hâlâ çalışıyor mu?
    pub fn is_running(&self) -> bool {
        match &self.inner {
            Backend::Platform(s) => s.is_running(),
            Backend::TestPattern(s) => s.is_running(),
        }
    }

    /// Yakalamayı durdurur.
    pub fn stop(self) {
        match self.inner {
            Backend::Platform(s) => s.stop(),
            Backend::TestPattern(s) => s.stop(),
        }
    }
}

/// `OPENCLASS_CAPTURE=test` ayarlı mı?
fn test_mode_enabled() -> bool {
    std::env::var(CAPTURE_OVERRIDE_VAR)
        .map(|v| v.trim().eq_ignore_ascii_case("test"))
        .unwrap_or(false)
}

/// Bu platformda ekran yakalama mümkün mü?
///
/// Test deseni etkinse her platformda `true` döner; arayüz bunu "destekleniyor"
/// olarak gösterir ama arka ucun adı sentetik olduğunu açıkça yazar.
pub fn is_supported() -> bool {
    cfg!(windows) || test_mode_enabled()
}

/// Ölçüm testleri için test deseni çizimine erişim.
///
/// Yalnızca test derlemesinde görünür; ölçümün gerçekçi bir masaüstü karesi
/// üretebilmesi için vardır.
#[cfg(test)]
pub(crate) fn test_pattern_for_bench(
    pixels: &mut [u8],
    width: u32,
    height: u32,
    hue: f32,
    frame: u64,
    label: &str,
) {
    test_pattern::draw_for_bench(pixels, width, height, hue, frame, label);
}
