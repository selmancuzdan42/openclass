//! Öğrenci klavye ve faresinin geçici olarak kilitlenmesi.
//!
//! # Vazgeçilmez kurallar
//!
//! * Kilit **oturuma bağlıdır**: ders bitince, öğretmen bıraktığında,
//!   bağlantı koptuğunda veya zaman aşımında kendiliğinden kalkar.
//! * Ekranda **neden kilitlendiğini açıklayan** bir pencere durur; sessizce
//!   donmuş bir makine bırakılmaz.
//! * **Ctrl+Alt+Del engellenmez.** Windows bunu zaten engelletmez ve
//!   engellemeye çalışılmaz: öğrenci her zaman makineyi kurtarabilmelidir.
//! * Ajan kapanırsa kilit de kalkar (işletim sistemi girdi engelini süreç
//!   sonlandığında kaldırır).
//! * Kalıcılık veya gizlenme numarası yoktur.
//!
//! # Üst sınır
//!
//! Öğretmen kilidi açmayı unutursa makine sonsuza kadar kilitli kalmamalıdır.
//! [`MAX_DURATION`] dolduğunda kilit kendiliğinden kalkar.

use std::time::Duration;

/// Kilidin kendiliğinden kalkacağı süre.
///
/// Bir ders saatinden uzun; ama unutulmuş bir kilidin makineyi süresiz
/// kilitlemesini engelleyecek kadar kısa.
pub const MAX_DURATION: Duration = Duration::from_secs(60 * 60);

/// Bu platformda kilit uygulanabilir mi?
pub fn is_supported() -> bool {
    cfg!(windows)
}

/// Girdi engelini açar.
pub fn engage() {
    platform::set(true);
}

/// Girdi engelini kaldırır.
pub fn release() {
    platform::set(false);
}

#[cfg(windows)]
mod platform {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::BlockInput;

    /// `BlockInput` klavye ve fare girdisini engeller.
    ///
    /// Ctrl+Alt+Del bu engelin dışındadır — Windows'un kuralı budur ve
    /// bilinçli olarak korunur.
    pub fn set(blocked: bool) {
        let ok = unsafe { BlockInput(i32::from(blocked)) };
        if ok == 0 {
            // Yükseltilmiş bir süreç girdiyi zaten engelliyorsa veya
            // güvenli masaüstü etkinse başarısız olur.
            tracing::warn!(
                blocked,
                hata = %std::io::Error::last_os_error(),
                "girdi engeli uygulanamadı"
            );
        } else {
            tracing::info!(blocked, "girdi engeli değişti");
        }
    }
}

#[cfg(not(windows))]
mod platform {
    /// Windows dışında girdi engeli yoktur.
    pub fn set(blocked: bool) {
        tracing::debug!(blocked, "girdi engeli bu platformda desteklenmiyor");
    }
}
