//! Gizli materyalin diskte korunması.
//!
//! Korunan değerler: öğretmen TLS özel anahtarı ve öğrenci cihaz kimliği
//! tohumu. Bunlar düz metin olarak diske **yazılmaz**.
//!
//! * **Windows:** DPAPI (`CryptProtectData`). Anahtar işletim sistemi
//!   tarafından yönetilir, uygulamanın saklayacağı ikinci bir sır yoktur.
//! * **Diğer platformlar:** yalnızca geliştirme içindir. Dosya `0600` izinle
//!   düz yazılır ve her açılışta uyarı loglanır. Üretim hedefi Windows'tur.

use std::path::Path;

use anyhow::{Context, Result};

/// DPAPI koruma kapsamı.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// Yalnızca aynı Windows kullanıcısı çözebilir.
    CurrentUser,
    /// Makinedeki herhangi bir kullanıcı çözebilir.
    ///
    /// Laboratuvar bilgisayarlarında öğrenciler farklı Windows hesaplarıyla
    /// oturum açabildiği için cihaz kimliğinin makineye bağlı olması gerekir;
    /// aksi hâlde her hesap ayrı bir cihaz gibi görünür ve yeniden eşleştirme
    /// ister. Bu kapsam, makinede oturum açabilen bir öğrencinin cihaz
    /// kimliğini okuyabileceği anlamına gelir — kabul edilen bir ödünleşimdir,
    /// çünkü aynı öğrenci zaten o makinenin kendisidir.
    LocalMachine,
}

/// Veriyi işletim sistemi korumasıyla sarmalar.
pub fn protect(plaintext: &[u8], scope: Scope) -> Result<Vec<u8>> {
    platform::protect(plaintext, scope)
}

/// [`protect`] ile sarmalanmış veriyi açar.
pub fn unprotect(blob: &[u8]) -> Result<Vec<u8>> {
    platform::unprotect(blob)
}

/// Gizli değeri korunmuş hâlde dosyaya yazar.
pub fn write_file(path: &Path, plaintext: &[u8], scope: Scope) -> Result<()> {
    let blob = protect(plaintext, scope)?;
    std::fs::write(path, &blob)
        .with_context(|| format!("gizli dosya yazılamadı: {}", path.display()))?;
    harden(path)?;
    Ok(())
}

/// Korunmuş dosyayı okur ve açar. Dosya yoksa `Ok(None)` döner.
pub fn read_file(path: &Path) -> Result<Option<Vec<u8>>> {
    match std::fs::read(path) {
        Ok(blob) => Ok(Some(unprotect(&blob).with_context(|| {
            format!("gizli dosya çözülemedi: {}", path.display())
        })?)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err).with_context(|| format!("gizli dosya okunamadı: {}", path.display())),
    }
}

fn harden(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(path)?.permissions();
        perms.set_mode(0o600);
        std::fs::set_permissions(path, perms)?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

#[cfg(windows)]
mod platform {
    use anyhow::{bail, Result};
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{
        CryptProtectData, CryptUnprotectData, CRYPTPROTECT_LOCAL_MACHINE, CRYPT_INTEGER_BLOB,
    };

    use super::Scope;

    /// Çıktı blob'unu Rust'a kopyalar ve Windows'un ayırdığı belleği serbest bırakır.
    unsafe fn take_blob(blob: &mut CRYPT_INTEGER_BLOB) -> Vec<u8> {
        let slice = std::slice::from_raw_parts(blob.pbData, blob.cbData as usize);
        let out = slice.to_vec();
        LocalFree(blob.pbData as *mut _);
        blob.pbData = std::ptr::null_mut();
        out
    }

    pub fn protect(plaintext: &[u8], scope: Scope) -> Result<Vec<u8>> {
        let input = CRYPT_INTEGER_BLOB {
            cbData: plaintext.len() as u32,
            pbData: plaintext.as_ptr() as *mut u8,
        };
        let mut output = CRYPT_INTEGER_BLOB {
            cbData: 0,
            pbData: std::ptr::null_mut(),
        };
        let flags = match scope {
            Scope::CurrentUser => 0,
            Scope::LocalMachine => CRYPTPROTECT_LOCAL_MACHINE,
        };

        let ok = unsafe {
            CryptProtectData(
                &input,
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                flags,
                &mut output,
            )
        };
        if ok == 0 {
            bail!(
                "DPAPI CryptProtectData başarısız: {}",
                std::io::Error::last_os_error()
            );
        }
        Ok(unsafe { take_blob(&mut output) })
    }

    pub fn unprotect(blob: &[u8]) -> Result<Vec<u8>> {
        let input = CRYPT_INTEGER_BLOB {
            cbData: blob.len() as u32,
            pbData: blob.as_ptr() as *mut u8,
        };
        let mut output = CRYPT_INTEGER_BLOB {
            cbData: 0,
            pbData: std::ptr::null_mut(),
        };

        let ok = unsafe {
            CryptUnprotectData(
                &input,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                0,
                &mut output,
            )
        };
        if ok == 0 {
            bail!(
                "DPAPI CryptUnprotectData başarısız: {}",
                std::io::Error::last_os_error()
            );
        }
        Ok(unsafe { take_blob(&mut output) })
    }
}

#[cfg(not(windows))]
mod platform {
    use anyhow::Result;

    use super::Scope;

    /// Windows dışında işletim sistemi destekli koruma yoktur.
    ///
    /// Bu yol yalnızca Linux/macOS üzerinde geliştirme yaparken kullanılır.
    /// Dosya düz yazılır; koruma dosya izinlerinden ibarettir.
    pub fn protect(plaintext: &[u8], _scope: Scope) -> Result<Vec<u8>> {
        tracing::warn!(
            "bu platformda DPAPI yok — gizli değer diske ŞİFRESİZ yazılıyor \
             (yalnızca geliştirme ortamı için kabul edilebilir)"
        );
        Ok(plaintext.to_vec())
    }

    pub fn unprotect(blob: &[u8]) -> Result<Vec<u8>> {
        Ok(blob.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn korunan_deger_geri_okunabilir() {
        let dir = std::env::temp_dir().join(format!("openclass-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("secret.bin");

        let secret = b"cihaz tohumu 32 bayt olurdu......";
        write_file(&path, secret, Scope::CurrentUser).unwrap();
        assert_eq!(read_file(&path).unwrap().unwrap(), secret);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn olmayan_dosya_none_doner() {
        let path = std::env::temp_dir().join("openclass-olmayan-dosya-xyz.bin");
        std::fs::remove_file(&path).ok();
        assert!(read_file(&path).unwrap().is_none());
    }
}
