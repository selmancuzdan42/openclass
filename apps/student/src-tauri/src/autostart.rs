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

#[cfg(windows)]
mod platform {
    use super::VALUE_NAME;
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
}
