//! Windows açılışında otomatik başlatma.
//!
//! # Gizlenmez
//!
//! Kayıt defteri girdisi standart `Run` anahtarına, uygulamanın kendi adıyla
//! yazılır. Görev Zamanlayıcı'ya gizli görev eklemek, adı başka bir şeymiş
//! gibi göstermek veya kaldırmayı zorlaştırmak gibi yöntemler **bilinçli
//! olarak kullanılmaz**: ajanın varlığı makinede açıkça görünür olmalıdır.
//!
//! # Kapsam
//!
//! Girdi geçerli kullanıcı için yazılır (`HKEY_CURRENT_USER`). Makine geneli
//! (`HKEY_LOCAL_MACHINE`) yönetici yetkisi ister; laboratuvar dağıtımında
//! bunu kurulum paketi yapar (bkz. `docs/WINDOWS-KURULUM.md`). Çalışma
//! anında yetki yükseltmeye çalışılmaz.

use anyhow::Result;

/// Görev Zamanlayıcı'daki görevin adı.
///
/// Oturum açılışında uygulamayı "en yüksek ayrıcalıkla" başlatır (onlogon +
/// `/rl highest`). Böylece yönetici gerektiren komutlar UAC istemeden çalışır.
/// Ad, başka bir şeymiş gibi gizlenmez; varlığı Görev Zamanlayıcı'da açıktır.
#[cfg(windows)]
const TASK_NAME: &str = "OpenClass Student";

/// İkincil denetçinin (watchdog) kendini tanıdığı komut satırı bayrağı.
const WATCHDOG_ARG: &str = "--openclass-watch";

/// Otomatik başlatma bu platformda yapılandırılabilir mi?
pub fn is_supported() -> bool {
    cfg!(windows)
}

/// Şu anda etkin mi?
pub fn is_enabled() -> bool {
    platform::is_enabled()
}

/// Açar veya kapatır.
pub fn set_enabled(enabled: bool) -> Result<()> {
    platform::set_enabled(enabled)
}

/// Uygulamanın ikincil denetçi (watchdog) modunda başlatılıp
/// başlatılmadığı.
///
/// Denetçi, ana ajanı gözetleyen ayrı bir süreçtir ve kendi bayrağıyla
/// çalışır. Platformdan bağımsızdır (yalnızca komut satırı argümanına bakar);
/// denetçinin gerçek döngüsü Windows'a özgüdür.
pub fn is_watchdog_mode() -> bool {
    std::env::args().any(|a| a == WATCHDOG_ARG)
}

/// Süreç henüz yönetici (admin) olarak çalışmıyorsa yükseltilmiş bir kopya
/// başlatır.
///
/// Dönüş değeri akış biçiminde yorumlanır: `Ok(true)` çağıranın mevcut
/// süreçte devam etmesi (ya zaten eleve, ya da yükseltme başlatılamadı),
/// `Ok(false)` yükseltilmiş kopya başlatıldıysa ve mevcut sürecin **kapanması**
/// gerekiyorsa. Windows dışı her zaman `Ok(true)`.
pub fn promote_to_admin() -> Result<bool> {
    platform::promote_to_admin()
}

/// İkincil denetçiyi (watchdog) hazırlar ve zaten çalışmıyorsa başlatır.
///
/// Ana sürecin kimliğini kalıcı bir dosyaya yazar. Denetçi, bu kimliğin
/// öldüğünü görünce ajanı yeniden başlatır. Windows dışı no-op.
pub fn ensure_watchdog() -> Result<()> {
    platform::ensure_watchdog()
}

/// İkincil denetçi döngüsünü çalıştırır (yalnızca `is_watchdog_mode()` iken).
pub fn run_watchdog() {
    platform::run_watchdog()
}

/// Düzgün kapanışın ardından denetçinin de durmasını sağlayan temizliği yapar.
pub fn notify_graceful_exit() {
    platform::notify_graceful_exit();
}

#[cfg(windows)]
mod platform {
    use super::{TASK_NAME, WATCHDOG_ARG};
    use anyhow::{bail, Result};

    pub fn is_enabled() -> bool {
        // Run anahtarı yerine Task Scheduler görevi kullanılır: oturum açılışında
        // "en yüksek ayrıcalıkla" (onlogon + /rl highest) başlatılan bir görev
        // UAC istemeden yönetici süreci başlatır. Run anahtarı bunu yapamaz.
        let status = std::process::Command::new("schtasks")
            .arg("/query")
            .arg("/tn")
            .arg(TASK_NAME)
            .output();
        matches!(status, Ok(out) if out.status.success())
    }

    pub fn set_enabled(enabled: bool) -> Result<()> {
        // Görevin yolu her seferinde güncel exe'ye işaret etsin.
        let exe = std::env::current_exe()?;
        let tr = format!("\"{}\"", exe.display()); // yolda boşluk olabilir

        let status = if enabled {
            std::process::Command::new("schtasks")
                .arg("/create")
                .arg("/f")
                .arg("/tn")
                .arg(TASK_NAME)
                .arg("/tr")
                .arg(&tr)
                .arg("/sc")
                .arg("onlogon")
                .arg("/rl")
                .arg("highest")
                .status()
        } else {
            // Görevi sil; hata yönetici eksikse/görev yoksa zararsız say.
            std::process::Command::new("schtasks")
                .arg("/delete")
                .arg("/f")
                .arg("/tn")
                .arg(TASK_NAME)
                .status()
        };

        match status {
            Ok(st) if st.success() => {}
            Ok(st) if !st.success() && !enabled && !is_enabled() => {
                // Görev zaten yok/izinsiz: kaldırmanın "başarısızlığı" zararsız.
            }
            Ok(st) => bail!(
                "schtasks komutu başarısız oldu (kod {:?}) — yönetici oturumda olmalı",
                st.code()
            ),
            Err(err) => bail!("schtasks çalıştırılamadı: {err}"),
        }

        tracing::info!(
            enabled,
            "otomatik başlatma (Task Scheduler, yüksek ayrıcalık) ayarlandı"
        );
        Ok(())
    }

    // ----------------------------------------------------------- yönetici

    /// Süreç yönetici (elevated) olarak mı çalışıyor?
    fn is_elevated() -> bool {
        use windows_sys::Win32::Security::{
            GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY,
        };
        use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

        let mut token: windows_sys::Win32::Foundation::HANDLE = std::ptr::null_mut();
        // Geçerli sürecin erişim simgesini açar.
        if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
            return false;
        }

        let mut elevation = TOKEN_ELEVATION { TokenIsElevated: 0 };
        let mut size: u32 = 0;
        let ok = unsafe {
            GetTokenInformation(
                token,
                TokenElevation,
                &mut elevation as *mut _ as *mut core::ffi::c_void,
                std::mem::size_of::<TOKEN_ELEVATION>() as u32,
                &mut size,
            )
        };
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(token);
        }
        ok != 0 && elevation.TokenIsElevated != 0
    }

    pub fn promote_to_admin() -> Result<bool> {
        use windows_sys::Win32::UI::Shell::ShellExecuteW;
        use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

        if is_elevated() {
            return Ok(true);
        }

        // Henüz yönetici değiliz: kendimizi `runas` (yönetici) sözcüğüyle
        // yeniden başlat. Başarılı başlatma için ShellExecuteW 32'den büyük
        // bir değer döner; kullanıcı UAC'yi reddettiğinde hata koduna düşer.
        let exe = std::env::current_exe()?;
        let exe_wide: Vec<u16> = exe
            .to_string_lossy()
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        // "runas" sözcüğü yönetici ayrıcalığıyla çalıştırma ister.
        let verb: Vec<u16> = "runas".encode_utf16().chain(std::iter::once(0)).collect();
        let dir_wide: Vec<u16> = exe
            .parent()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default()
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        let result = unsafe {
            ShellExecuteW(
                std::ptr::null_mut(),
                verb.as_ptr(),
                exe_wide.as_ptr(),
                std::ptr::null(), // normal başlatma, argümansız
                dir_wide.as_ptr(),
                SW_SHOWNORMAL,
            )
        };

        if result as isize > 32 {
            // Yükseltilmiş kopya başlatıldı; mevcut süreç kapanmalı.
            tracing::info!("yönetici kopyası başlatıldı; mevcut süreç kapanıyor");
            return Ok(false);
        }

        // Kullanıcı reddetti ya da hata oldu: ajan yine de çalışsın, ancak
        // sınırlı yetkiyle (kapatma/internet engeli gerektiren işler düşer).
        tracing::warn!(
            sonuc = result as isize,
            "yönetici yükseltmesi başlatılamadı; sınırlı yetkiyle sürülüyor"
        );
        Ok(true)
    }

    // ------------------------------------------------------- ikincil denetçi

    /// Denetçinin durum dosyalarının tutulduğu klasör.
    fn supervisor_dir() -> anyhow::Result<std::path::PathBuf> {
        use anyhow::Context;
        let base = openclass_common::paths::data_dir(openclass_common::paths::App::Student)?;
        let dir = base.join("supervisor");
        std::fs::create_dir_all(&dir)
            .with_context(|| format!("denetçi klasörü oluşturulamadı: {}", dir.display()))?;
        Ok(dir)
    }

    /// Verilen süreç kimliğine ait canlı bir süreç var mı?
    fn process_alive(pid: u32) -> bool {
        use windows_sys::Win32::System::Threading::{
            GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
        };
        if pid == 0 {
            return false;
        }
        let handle =
            unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
        if handle.is_null() {
            return false;
        }
        let mut code: u32 = 0;
        let ok = unsafe { GetExitCodeProcess(handle, &mut code) };
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(handle);
        }
        // STILL_ACTIVE = 259: süreç hâlâ çalışıyor demektir.
        ok != 0 && code == 259
    }

    /// Süreci penceresiz, ana süreçten bagımsız olarak başlatır.
    fn spawn_detached(exe: &std::path::Path, args: &[&str]) -> anyhow::Result<u32> {
        use std::process::Command;
        use windows_sys::Win32::System::Threading::{CREATE_NO_WINDOW, DETACHED_PROCESS};

        let mut cmd = Command::new(exe);
        cmd.args(args)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS);
        }
        let child = cmd.spawn()?;
        Ok(child.id())
    }

    pub fn ensure_watchdog() -> Result<()> {
        use anyhow::Context;

        let dir = supervisor_dir()?;
        // Ana sürecin kimliğini kalıcı yap: denetçi bunu gözetler.
        std::fs::write(dir.join("main.pid"), std::process::id().to_string())
            .with_context(|| "ana süreç kimliği yazılamadı")?;

        // Zaten çalışan bir denetçi varsa yenisini çıkarma.
        let watchdog_pid = dir.join("watchdog.pid");
        if let Ok(text) = std::fs::read_to_string(&watchdog_pid) {
            if let Ok(pid) = text.trim().parse::<u32>() {
                if process_alive(pid) {
                    return Ok(());
                }
            }
        }

        let exe = std::env::current_exe()?;
        let pid = spawn_detached(&exe, &[WATCHDOG_ARG])
            .with_context(|| "ikincil denetçi başlatılamadı")?;
        std::fs::write(&watchdog_pid, pid.to_string())?;
        tracing::info!(pid, "ikincil denetçi başlatıldı");
        Ok(())
    }

    /// Denetçi döngüsü: ana ajan ölürse yeniden başlat, düzgün kapanınca dur.
    pub fn run_watchdog() {
        use std::time::Duration;

        tracing::info!("ikincil denetçi başladı");
        let Ok(dir) = supervisor_dir() else {
            return;
        };
        let main_pid = dir.join("main.pid");
        let Ok(exe) = std::env::current_exe() else {
            return;
        };

        loop {
            // Ana süreç dosyası silindiyse ajan usulüne uygun kapandı demektir;
            // denetçi de kendini sonlandırır.
            let Ok(text) = std::fs::read_to_string(&main_pid) else {
                tracing::info!("ana ajan düzgün kapandı; denetçi duruyor");
                return;
            };
            let Ok(pid) = text.trim().parse::<u32>() else {
                // Bozuk dosya: bir sonraki turda tekrar bak.
                std::thread::sleep(Duration::from_secs(3));
                continue;
            };

            if process_alive(pid) {
                std::thread::sleep(Duration::from_secs(3));
                continue;
            }

            // Ana ajan öldü (zorla sonlandırıldı); yeniden başlat.
            tracing::warn!(pid, "ana ajan çalışmıyor; yeniden başlatılıyor");
            match spawn_detached(&exe, &[]) {
                Ok(new_pid) => {
                    tracing::info!(new_pid, "ajan yeniden başlatıldı");
                    // Yeni ana süreç kendi kimliğini yazar; ona nefes payı ver.
                    std::thread::sleep(Duration::from_secs(5));
                }
                Err(err) => {
                    tracing::error!(hata = %format!("{err:#}"), "ajan yeniden başlatılamadı");
                    std::thread::sleep(Duration::from_secs(10));
                }
            }
        }
    }

    /// Düzgün kapanışın ardından `main.pid`'i siler; denetçi böylece durur.
    pub fn notify_graceful_exit() {
        let Ok(dir) = supervisor_dir() else {
            return;
        };
        let _ = std::fs::remove_file(dir.join("main.pid"));
        tracing::info!("ana ajan kapandı, denetçi durduruluyor");
    }
}

#[cfg(not(windows))]
mod platform {
    use anyhow::{bail, Result};

    pub fn is_enabled() -> bool {
        false
    }

    pub fn set_enabled(_enabled: bool) -> Result<()> {
        bail!("Otomatik başlatma yalnızca Windows'ta yapılandırılabilir.")
    }

    pub fn promote_to_admin() -> Result<bool> {
        Ok(true)
    }

    pub fn ensure_watchdog() -> Result<()> {
        Ok(())
    }

    pub fn run_watchdog() {}

    pub fn notify_graceful_exit() {}
}
