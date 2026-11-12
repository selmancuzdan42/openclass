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

/// Kayıt defterindeki değerin adı.
///
/// Uygulamanın kendi adı kullanılır; başka bir şeymiş gibi gizlenmez.
#[cfg(windows)]
const VALUE_NAME: &str = "OpenClass Student";

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
    use super::{VALUE_NAME, WATCHDOG_ARG};
    use anyhow::{bail, Result};
    use windows_sys::Win32::Foundation::ERROR_SUCCESS;
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW, HKEY,
        HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_SZ,
    };

    const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

    /// UTF-16, NUL ile sonlandırılmış dizge.
    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// `Run` anahtarını açar; çağıran kapatmakla yükümlüdür.
    fn open(access: u32) -> Result<HKEY> {
        let mut key: HKEY = std::ptr::null_mut();
        let status = unsafe {
            RegOpenKeyExW(
                HKEY_CURRENT_USER,
                wide(RUN_KEY).as_ptr(),
                0,
                access,
                &mut key,
            )
        };
        if status != ERROR_SUCCESS {
            bail!("kayıt defteri anahtarı açılamadı (kod {status})");
        }
        Ok(key)
    }

    pub fn is_enabled() -> bool {
        let Ok(key) = open(KEY_READ) else {
            return false;
        };
        let mut size: u32 = 0;
        let status = unsafe {
            RegQueryValueExW(
                key,
                wide(VALUE_NAME).as_ptr(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut size,
            )
        };
        unsafe { RegCloseKey(key) };
        status == ERROR_SUCCESS
    }

    pub fn set_enabled(enabled: bool) -> Result<()> {
        let key = open(KEY_WRITE)?;
        let name = wide(VALUE_NAME);

        let status = if enabled {
            let exe = std::env::current_exe()?;
            // Yol boşluk içerebilir; tırnak içine alınır.
            let command = format!("\"{}\"", exe.display());
            let value = wide(&command);
            unsafe {
                RegSetValueExW(
                    key,
                    name.as_ptr(),
                    0,
                    REG_SZ,
                    value.as_ptr().cast::<u8>(),
                    (value.len() * 2) as u32,
                )
            }
        } else {
            let status = unsafe { RegDeleteValueW(key, name.as_ptr()) };
            // Zaten yoksa başarı sayılır.
            if status != ERROR_SUCCESS && !is_enabled() {
                ERROR_SUCCESS
            } else {
                status
            }
        };

        unsafe { RegCloseKey(key) };

        if status != ERROR_SUCCESS {
            bail!("kayıt defteri güncellenemedi (kod {status})");
        }
        tracing::info!(enabled, "otomatik başlatma ayarı değişti");
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
