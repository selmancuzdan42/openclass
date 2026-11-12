//! Klavye izleme (öğretmenin canlı tuş vuruşu görüntüsü).
//!
//! Windows'ta düşük seviyeli klavye kancası (`WH_KEYBOARD_LL`,
//! [`SetWindowsHookExA`]) kuran ayrı bir iş parçacığı kullanır. Kancanın
//! çalışması için kancayı kuran iş parçacığının bir **mesaj döngüsü** döndürmesi
//! gerekir (low-level hook'lar buradan beslenir); bu modül tam olarak bunu
//! yapar. Her tuş olayı, kancayı tıkamadan bir callback'e aktarılır.
//!
//! ## Görünürlük / etik
//!
//! İzleme, öğretmen `StartKeyMonitor` bildirimiyle açıkça başlattığında
//! kurulur ve `StopKeyMonitor` ile ya da bağlantı koptuğunda kapanır. Gizli
//! bir keylogger değildir; protokol ve arayüz itibarıyla öğrencinin bilgisi
//! dâhilinde çalışır.
//!
//! Windows dışındaki platformlarda başlatma "desteklenmiyor" hatası döner ve
//! hiçbir hook kurulmaz.

use std::sync::Arc;

/// Yakalanan tek bir tuş olayı.
#[derive(Debug, Clone, Copy)]
pub struct KeyEvent {
    /// Windows sanal tuş kodu (`VK_*`).
    pub code: u16,
    /// `true` basılma, `false` bırakma.
    pub pressed: bool,
    /// Olay zaman damgası (Unix milisaniye).
    pub ts: u64,
}

/// Başlatma/durdurma hataları.
#[derive(Debug)]
pub struct Error(pub String);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

/// Her tuş olayında çağrılan geri çağrı.
pub type Handler = Arc<dyn Fn(KeyEvent) + Send + Sync>;

/// Tuş izlemesini başlatır.
///
/// `handler` kancalı iş parçacığından çağrılır; **hızlı ve bloklamayan**
/// olmalıdır (kaçırılan tuş olmasın). İdeal kullanım, bir kanala/kuyruğa
/// tamponlamaktır.
pub fn start(handler: Handler) -> Result<(), Error> {
    #[cfg(windows)]
    {
        platform::start(handler)
    }
    #[cfg(not(windows))]
    {
        let _ = handler;
        Err(Error(
            "Klavye izleme yalnızca Windows'ta destekleniyor.".into(),
        ))
    }
}

/// Tuş izlemesini durdurur. Çalışmıyorsa hiçbir şey yapmaz.
pub fn stop() {
    #[cfg(windows)]
    platform::stop();
    #[cfg(not(windows))]
    {}
}

/// Şu anda izleme aktif mi?
pub fn is_active() -> bool {
    #[cfg(windows)]
    {
        platform::is_active()
    }
    #[cfg(not(windows))]
    {
        false
    }
}

#[cfg(windows)]
mod platform {
    use super::*;

    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc::Sender;
    use std::sync::{Mutex, OnceLock};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use windows_sys::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
    use windows_sys::Win32::System::Threading::GetCurrentThreadId;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, DispatchMessageW, GetMessageW, PostThreadMessageW, SetWindowsHookExA,
        TranslateMessage, UnhookWindowsHookEx, HHOOK, KBDLLHOOKSTRUCT, MSG, WH_KEYBOARD_LL,
        WM_KEYDOWN, WM_QUIT, WM_SYSKEYDOWN,
    };

    /// Hook bildirim kodu: her tuş olayı `HC_ACTION` ile gelir.
    const HC_ACTION: i32 = 0;

    /// Çalışan bir izleme kancası.
    struct Active {
        running: Arc<AtomicBool>,
        /// Mesaj döngüsünü uyandırmak için `WM_QUIT` gönderilecek iş parçacığının kimliği.
        thread_id: u32,
        handle: std::thread::JoinHandle<()>,
    }

    /// Tek eşzamanlı hook: aynı anda yalnızca bir izleme çalışır.
    static STATE: OnceLock<Mutex<Option<Active>>> = OnceLock::new();

    fn state() -> &'static Mutex<Option<Active>> {
        STATE.get_or_init(|| Mutex::new(None))
    }

    // Kancanın kurulduğu iş parçacığına özel geri çağrı. Low-level hook
    // callback'i serbest işlev (`extern "system"`) olmak zorunda olduğu için
    // handler buradan bir thread-local üzerinden erişilir.
    thread_local! {
        static HANDLER: std::cell::RefCell<Option<Handler>> = std::cell::RefCell::new(None);
    }

    pub(crate) fn start(handler: Handler) -> Result<(), Error> {
        let running = Arc::new(AtomicBool::new(true));
        let flag = Arc::clone(&running);
        // İş parçacığı, kendi kimliğini buradan geri bildirir; böylece
        // `stop()` durdurulacak thread_id'yi garanti biçimde bilir.
        let (tid_tx, tid_rx) = std::sync::mpsc::channel::<u32>();
        let handle = std::thread::Builder::new()
            .name("keymon-hook".into())
            .spawn(move || run_loop(handler, flag, tid_tx))
            .map_err(|e| Error(format!("izleme iş parçacığı başlatılamadı: {e}")))?;

        let thread_id = match tid_rx.recv_timeout(Duration::from_secs(3)) {
            Ok(id) => id,
            Err(_) => {
                // İş parçacığı başladı ama id'sini bildirmeden takıldı (veya
                // hemen patladı). Kalıntı kanca bırakmamak için bayrağı kapatıp
                // iş parçacığını bekleriz; GetMessage sarmalına hiç girmeden
                // dönmüş olacağı için join kilitlenmez.
                running.store(false, Ordering::Relaxed);
                let _ = handle.join();
                return Err(Error("izleme iş parçacığı yanıt vermedi".into()));
            }
        };

        let mut guard = state()
            .lock()
            .map_err(|_| Error("izleme kilidi tutulamadı".into()))?;
        *guard = Some(Active {
            running,
            thread_id,
            handle,
        });
        Ok(())
    }

    fn run_loop(handler: Handler, running: Arc<AtomicBool>, tid_tx: Sender<u32>) {
        HANDLER.with(|h| *h.borrow_mut() = Some(handler));
        // Kanca kurulmadan önce kimlik ilet; `start()` bunu bekler.
        let tid = unsafe { GetCurrentThreadId() };
        let _ = tid_tx.send(tid);

        if let Err(e) = install_and_run(&running) {
            tracing::error!(hata = %e, "klavye izleme kancası durdu");
        }
        HANDLER.with(|h| *h.borrow_mut() = None);
    }

    fn install_and_run(running: &AtomicBool) -> Result<(), Error> {
        unsafe extern "system" fn hook_proc(ncode: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
            if ncode == HC_ACTION {
                let msg = wparam as u32;
                let pressed = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;
                HANDLER.with(|h| {
                    if let Some(handler) = h.borrow().as_ref() {
                        let kb = lparam as *const KBDLLHOOKSTRUCT;
                        if !kb.is_null() {
                            let kb = unsafe { &*kb };
                            handler(KeyEvent {
                                code: kb.vkCode as u16,
                                pressed,
                                ts: now_ms(),
                            });
                        }
                    }
                });
            }
            unsafe { CallNextHookEx(std::ptr::null_mut(), ncode, wparam, lparam) }
        }

        // hmod `null`: low-level hook'un callback'i bu işlemde (DLL değil),
        // bu yüzden döngü iş parçacığındaki kod yeterli.
        let hook: HHOOK =
            unsafe { SetWindowsHookExA(WH_KEYBOARD_LL, Some(hook_proc), std::ptr::null_mut(), 0) };
        if hook.is_null() {
            return Err(Error(
                "Klavye kancası kurulamadı (SetWindowsHookExA).".into(),
            ));
        }

        let mut msg = MSG {
            hwnd: std::ptr::null_mut(),
            message: 0,
            wParam: 0,
            lParam: 0,
            time: 0,
            pt: unsafe { std::mem::zeroed() },
        };
        loop {
            // Güvenlik kapağı: normalde durdurma yine de `WM_QUIT` göndererek
            // GetMessage'i uyandırır; bayrak yalnızca ek emniyettir.
            if !running.load(Ordering::Relaxed) {
                break;
            }
            // 0 = WM_QUIT alındı, -1 = hata. İkisinde de döngüden çık.
            let r = unsafe { GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) };
            if r == 0 || r == -1 {
                break;
            }
            unsafe {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }

        unsafe {
            UnhookWindowsHookEx(hook);
        }
        Ok(())
    }

    pub(crate) fn stop() {
        let (thread_id, handle, running) = match state().lock() {
            Ok(mut guard) => match guard.take() {
                Some(active) => (active.thread_id, active.handle, active.running),
                None => return,
            },
            Err(_) => return,
        };

        running.store(false, Ordering::Relaxed);
        // `start()` thread_id'yi iletmeden dönmediği için bu her zaman sıfır
        // değildir; GetMessage'i blokta bekletmeden uyandırır.
        unsafe {
            PostThreadMessageW(thread_id, WM_QUIT, 0, 0);
        }
        // Hook iş parçacığı `UnhookWindowsHookEx`'i kendi üzerinde çağırıp
        // kapanır; onu bekleriz ki kanca kaldırılmadan geri dönmeyelim.
        let _ = handle.join();
    }

    pub(crate) fn is_active() -> bool {
        state().lock().map(|g| g.is_some()).unwrap_or(false)
    }

    /// Unix milisaniye cinsinden şimdiki zaman (olay zaman damgası için).
    fn now_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }
}
