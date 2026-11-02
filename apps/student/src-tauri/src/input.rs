//! Öğretmenden gelen girdi olaylarının yerel makineye uygulanması.
//!
//! # Sınırlar
//!
//! * Yalnızca öğretmen kontrolü açıkça
//!   devraldığında çalışır.
//! * Ders bittiğinde, kontrol bırakıldığında veya bağlantı koptuğunda
//!   kendiliğinden durur.
//! * Windows'un **güvenli masaüstüne** (UAC yükseltme istemi,
//!   Ctrl+Alt+Del, kilit ekranı) girdi gönderilemez. `SendInput` bu
//!   ekranlarda etkisizdir ve bu kısıt **aşılmaya çalışılmaz**.
//! * Öğrenci ekranında kontrol süresince kapatılamayan bir gösterge durur.
//!
//! # Koordinatlar
//!
//! Fare konumu normalleştirilmiş (0.0–1.0) gelir ve burada sanal ekranın
//! mutlak koordinatlarına çevrilir. Böylece öğretmenin gördüğü ölçeklenmiş
//! görüntü ile öğrencinin gerçek çözünürlüğü arasındaki fark sorun olmaz.

use openclass_protocol::control::InputEvent;

/// Girdi olaylarını uygular.
///
/// Windows dışında hiçbir şey yapmaz; bu platformlarda uzaktan kontrol
/// desteklenmez.
pub fn apply(events: &[InputEvent]) {
    // İlk yığını bildir: "girdi hiç geliyor mu?" sorusunun cevabı günlükte
    // dursun. Sonrası sessiz, yoksa saniyede onlarca satır yazardı.
    static SEEN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if !SEEN.swap(true, std::sync::atomic::Ordering::Relaxed) {
        tracing::info!(
            olay = events.len(),
            destekleniyor = is_supported(),
            "ilk uzaktan girdi yığını alındı"
        );
    }
    platform::apply(events);
}

/// Bu platformda uzaktan kontrol uygulanabilir mi?
pub fn is_supported() -> bool {
    cfg!(windows)
}

#[cfg(windows)]
mod platform {
    use super::InputEvent;
    use openclass_protocol::control::MouseButton;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT, KEYEVENTF_KEYUP,
        MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_HWHEEL, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP,
        MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_MOVE, MOUSEEVENTF_RIGHTDOWN,
        MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_VIRTUALDESK, MOUSEEVENTF_WHEEL, MOUSEINPUT,
    };

    /// Tekerleğin bir adımı (Windows sabiti).
    const WHEEL_DELTA: i32 = 120;

    pub fn apply(events: &[InputEvent]) {
        if events.is_empty() {
            return;
        }

        let mut inputs: Vec<INPUT> = Vec::with_capacity(events.len());
        for event in events {
            match *event {
                InputEvent::MouseMove { x, y } => {
                    // Mutlak koordinat 0..65535 aralığındadır.
                    let dx = (x.clamp(0.0, 1.0) * 65_535.0) as i32;
                    let dy = (y.clamp(0.0, 1.0) * 65_535.0) as i32;
                    inputs.push(mouse(
                        dx,
                        dy,
                        0,
                        MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK,
                    ));
                }
                InputEvent::MouseButton { button, pressed } => {
                    let flags = match (button, pressed) {
                        (MouseButton::Left, true) => MOUSEEVENTF_LEFTDOWN,
                        (MouseButton::Left, false) => MOUSEEVENTF_LEFTUP,
                        (MouseButton::Right, true) => MOUSEEVENTF_RIGHTDOWN,
                        (MouseButton::Right, false) => MOUSEEVENTF_RIGHTUP,
                        (MouseButton::Middle, true) => MOUSEEVENTF_MIDDLEDOWN,
                        (MouseButton::Middle, false) => MOUSEEVENTF_MIDDLEUP,
                    };
                    inputs.push(mouse(0, 0, 0, flags));
                }
                InputEvent::MouseWheel { dx, dy } => {
                    if dy != 0 {
                        inputs.push(mouse(0, 0, dy * WHEEL_DELTA, MOUSEEVENTF_WHEEL));
                    }
                    if dx != 0 {
                        inputs.push(mouse(0, 0, dx * WHEEL_DELTA, MOUSEEVENTF_HWHEEL));
                    }
                }
                InputEvent::Key { code, pressed } => {
                    inputs.push(key(code, pressed));
                }
            }
        }

        let sent = unsafe {
            SendInput(
                inputs.len() as u32,
                inputs.as_ptr(),
                std::mem::size_of::<INPUT>() as i32,
            )
        };

        if sent as usize != inputs.len() {
            // Güvenli masaüstü etkinken (UAC istemi, kilit ekranı) bu
            // beklenen bir sonuçtur; aşılmaya çalışılmaz.
            tracing::debug!(
                gonderilen = sent,
                istenen = inputs.len(),
                "girdi olaylarının tamamı uygulanamadı"
            );
        }
    }

    fn mouse(dx: i32, dy: i32, data: i32, flags: u32) -> INPUT {
        INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx,
                    dy,
                    mouseData: data as u32,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        }
    }

    fn key(code: u16, pressed: bool) -> INPUT {
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: code,
                    wScan: 0,
                    dwFlags: if pressed { 0 } else { KEYEVENTF_KEYUP },
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        }
    }
}

#[cfg(not(windows))]
mod platform {
    use super::InputEvent;

    /// Windows dışında uzaktan kontrol yoktur.
    ///
    /// Sessizce yok sayılır: öğretmen tarafı zaten `is_supported()` ile
    /// durumu öğrenir ve kontrolü başlatmaz.
    pub fn apply(events: &[InputEvent]) {
        if !events.is_empty() {
            tracing::debug!(
                adet = events.len(),
                "uzaktan kontrol bu platformda desteklenmiyor, olaylar yok sayıldı"
            );
        }
    }
}
