//! USB depolama kısıtlaması (yalnızca Windows).
//!
//! Sınav gibi durumlarda öğretmen USB bellekleri geçici olarak kapatabilir.
//! Kısıtlama **oturuma bağlıdır**: öğretmen bağlantısı koptuğunda ya da ajan
//! yeniden başladığında kendiliğinden geri alınır; ders bittikten sonra
//! bilgisayar kısıtlı kalmaz.
//!
//! # Nasıl
//!
//! Windows'un taşınabilir depolama ilkesi (`RemovableStorageDevices\Deny_All`)
//! açılır ve USB yığın depolama sürücüsü (`USBSTOR`) devre dışı bırakılır.
//! Klavye, fare gibi diğer USB aygıtları etkilenmez.
//!
//! # Yalnızca kendi yaptığını geri alır
//!
//! Okul bu ayarı zaten kendisi kapatmış olabilir. Kısıtlama uygulanmadan
//! önceki değerler bir durum dosyasına yazılır; geri alma o değerlere döner,
//! "varsayılan"a değil. Dosya **değişiklikten önce** yazılır: ajan tam o anda
//! ölse bile bir sonraki açılış neyi geri alacağını bilir.
//!
//! Her değişiklik öğrenciye bildirilir (bkz. `net::client`); sessiz kısıtlama
//! yoktur.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const POLICY_KEY: &str = r"HKLM\SOFTWARE\Policies\Microsoft\Windows\RemovableStorageDevices";
const POLICY_VALUE: &str = "Deny_All";
const USBSTOR_KEY: &str = r"HKLM\SYSTEM\CurrentControlSet\Services\USBSTOR";
const START_VALUE: &str = "Start";

/// Windows sürücü başlatma türleri (`Start` kayıt değeri).
const START_DEMAND: u32 = 3;
const START_DISABLED: u32 = 4;

/// Durum dosyasının adı (ajanın veri dizininde).
const STATE_FILE: &str = "restrictions.json";

/// Bizim uyguladığımız kısıtlama ve öncesindeki değerler.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
struct Saved {
    usb: Option<UsbPrev>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct UsbPrev {
    /// `USBSTOR\Start`; değer yoksa `None`.
    usbstor_start: Option<u32>,
    /// `Deny_All` ilkesi; tanımlı değilse `None`.
    deny_all: Option<u32>,
}

/// İşletim sistemi işlemleri. Mantık bu arayüzün arkasında sınanır.
trait System {
    fn reg_get(&self, key: &str, value: &str) -> Result<Option<u32>, String>;
    fn reg_set(&self, key: &str, value: &str, data: u32) -> Result<(), String>;
    fn reg_delete(&self, key: &str, value: &str) -> Result<(), String>;
}

/// USB depolama kısıtlamasını uygular ya da kaldırır.
///
/// İlk değer işlem sonrası kısıtlamanın **yürürlükte olup olmadığıdır**;
/// işlem başarısız olduysa ikinci değer nedenini taşır.
pub fn set_usb_blocked(dir: &Path, block: bool) -> (bool, Option<String>) {
    set_usb_blocked_with(&platform::Os, dir, block)
}

/// Bizim uyguladığımız kısıtlamayı geri alır.
///
/// Bağlantı koptuğunda ve ajan açılırken çağrılır. Geri alınacak bir şey
/// varsa `true` döner.
pub fn release_all(dir: &Path) -> bool {
    release_all_with(&platform::Os, dir)
}

fn set_usb_blocked_with(sys: &dyn System, dir: &Path, block: bool) -> (bool, Option<String>) {
    let mut saved = load(dir);
    let result = if block {
        block_usb(sys, dir, &mut saved)
    } else {
        unblock_usb(sys, dir, &mut saved)
    };
    let detail = result
        .err()
        .map(|err| format!("USB kısıtlaması uygulanamadı: {err}"));
    (saved.usb.is_some(), detail)
}

fn release_all_with(sys: &dyn System, dir: &Path) -> bool {
    let mut saved = load(dir);
    if saved.usb.is_none() {
        return false;
    }
    if let Err(err) = unblock_usb(sys, dir, &mut saved) {
        tracing::error!(%err, "USB kısıtlaması geri alınamadı");
    }
    true
}

fn block_usb(sys: &dyn System, dir: &Path, saved: &mut Saved) -> Result<(), String> {
    if saved.usb.is_none() {
        let prev = UsbPrev {
            usbstor_start: sys.reg_get(USBSTOR_KEY, START_VALUE)?,
            deny_all: sys.reg_get(POLICY_KEY, POLICY_VALUE)?,
        };
        // Önce kaydet: değişiklikten sonra ajan ölürse geri alınabilsin.
        saved.usb = Some(prev);
        store(dir, saved)?;
    }
    let applied = sys
        .reg_set(POLICY_KEY, POLICY_VALUE, 1)
        .and_then(|()| sys.reg_set(USBSTOR_KEY, START_VALUE, START_DISABLED));
    if let Err(err) = applied {
        // Yarım kalan kısıtlama bırakılmaz.
        let _ = unblock_usb(sys, dir, saved);
        return Err(err);
    }
    Ok(())
}

fn unblock_usb(sys: &dyn System, dir: &Path, saved: &mut Saved) -> Result<(), String> {
    let Some(prev) = saved.usb.clone() else {
        // Biz kısıtlamadık; okulun kendi ayarına dokunulmaz.
        return Ok(());
    };
    match prev.deny_all {
        Some(value) => sys.reg_set(POLICY_KEY, POLICY_VALUE, value)?,
        None => sys.reg_delete(POLICY_KEY, POLICY_VALUE)?,
    }
    sys.reg_set(
        USBSTOR_KEY,
        START_VALUE,
        prev.usbstor_start.unwrap_or(START_DEMAND),
    )?;
    saved.usb = None;
    store(dir, saved)
}

fn state_path(dir: &Path) -> PathBuf {
    dir.join(STATE_FILE)
}

/// Durum dosyasını okur; yoksa ya da bozuksa "kısıtlama yok" kabul edilir.
fn load(dir: &Path) -> Saved {
    std::fs::read_to_string(state_path(dir))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn store(dir: &Path, saved: &Saved) -> Result<(), String> {
    let path = state_path(dir);
    if *saved == Saved::default() {
        return match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(format!("durum dosyası silinemedi: {err}")),
        };
    }
    let text = serde_json::to_string(saved).map_err(|e| e.to_string())?;
    std::fs::write(&path, text).map_err(|err| format!("durum dosyası yazılamadı: {err}"))
}

/// `reg query` çıktısından `REG_DWORD` değerini okur (`0x4` → 4).
#[cfg(any(windows, test))]
fn parse_reg_dword(output: &str) -> Option<u32> {
    let line = output.lines().find(|l| l.contains("REG_DWORD"))?;
    let raw = line.split_whitespace().last()?;
    match raw.strip_prefix("0x").or_else(|| raw.strip_prefix("0X")) {
        Some(hex) => u32::from_str_radix(hex, 16).ok(),
        None => raw.parse().ok(),
    }
}

#[cfg(windows)]
mod platform {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    use super::{parse_reg_dword, System};

    /// Konsol penceresi açılmasın: ajan arka planda çalışır ve her komutta
    /// bir pencerenin yanıp sönmesi öğrenciyi rahatsız eder.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    pub struct Os;

    /// Komutu çalıştırır; çıkış kodunu ve birleşik çıktıyı döner.
    fn run(program: &str, args: &[&str]) -> Result<(i32, String), String> {
        let out = Command::new(program)
            .args(args)
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map_err(|err| format!("{program} çalıştırılamadı: {err}"))?;
        let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
        text.push_str(&String::from_utf8_lossy(&out.stderr));
        Ok((out.status.code().unwrap_or(-1), text))
    }

    fn expect_ok(program: &str, args: &[&str]) -> Result<(), String> {
        let (code, text) = run(program, args)?;
        if code == 0 {
            Ok(())
        } else {
            Err(format!("{program} başarısız (kod {code}): {}", text.trim()))
        }
    }

    impl System for Os {
        fn reg_get(&self, key: &str, value: &str) -> Result<Option<u32>, String> {
            let (code, text) = run("reg", &["query", key, "/v", value])?;
            // Anahtar ya da değer yoksa `reg` sıfırdan farklı kodla döner.
            Ok(if code == 0 {
                parse_reg_dword(&text)
            } else {
                None
            })
        }

        fn reg_set(&self, key: &str, value: &str, data: u32) -> Result<(), String> {
            let data = data.to_string();
            expect_ok(
                "reg",
                &[
                    "add",
                    key,
                    "/v",
                    value,
                    "/t",
                    "REG_DWORD",
                    "/d",
                    &data,
                    "/f",
                ],
            )
        }

        fn reg_delete(&self, key: &str, value: &str) -> Result<(), String> {
            if self.reg_get(key, value)?.is_none() {
                return Ok(());
            }
            expect_ok("reg", &["delete", key, "/v", value, "/f"])
        }
    }
}

#[cfg(not(windows))]
mod platform {
    use super::System;

    const UNSUPPORTED: &str = "bu özellik yalnızca Windows'ta desteklenir";

    pub struct Os;

    impl System for Os {
        fn reg_get(&self, _: &str, _: &str) -> Result<Option<u32>, String> {
            Err(UNSUPPORTED.into())
        }
        fn reg_set(&self, _: &str, _: &str, _: u32) -> Result<(), String> {
            Err(UNSUPPORTED.into())
        }
        fn reg_delete(&self, _: &str, _: &str) -> Result<(), String> {
            Err(UNSUPPORTED.into())
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::HashMap;

    use super::*;

    /// Kayıt defterini taklit eder.
    #[derive(Default)]
    struct Fake {
        reg: RefCell<HashMap<(String, String), u32>>,
        /// Bu anahtara yazma başarısız olur.
        fail_set: Option<&'static str>,
    }

    impl Fake {
        fn with(values: &[(&str, &str, u32)]) -> Self {
            let fake = Self::default();
            for (key, value, data) in values {
                fake.reg
                    .borrow_mut()
                    .insert((key.to_string(), value.to_string()), *data);
            }
            fake
        }

        fn get(&self, key: &str, value: &str) -> Option<u32> {
            self.reg
                .borrow()
                .get(&(key.to_string(), value.to_string()))
                .copied()
        }
    }

    impl System for Fake {
        fn reg_get(&self, key: &str, value: &str) -> Result<Option<u32>, String> {
            Ok(self.get(key, value))
        }
        fn reg_set(&self, key: &str, value: &str, data: u32) -> Result<(), String> {
            if self.fail_set == Some(key) {
                return Err("erişim reddedildi".into());
            }
            self.reg
                .borrow_mut()
                .insert((key.to_string(), value.to_string()), data);
            Ok(())
        }
        fn reg_delete(&self, key: &str, value: &str) -> Result<(), String> {
            self.reg
                .borrow_mut()
                .remove(&(key.to_string(), value.to_string()));
            Ok(())
        }
    }

    fn normal_pc() -> Fake {
        Fake::with(&[(USBSTOR_KEY, START_VALUE, START_DEMAND)])
    }

    #[test]
    fn usb_kisitlanir_ve_onceki_degerlere_doner() {
        let dir = tempfile::tempdir().unwrap();
        let pc = normal_pc();

        let (blocked, detail) = set_usb_blocked_with(&pc, dir.path(), true);
        assert_eq!(detail, None);
        assert!(blocked);
        assert_eq!(pc.get(POLICY_KEY, POLICY_VALUE), Some(1));
        assert_eq!(pc.get(USBSTOR_KEY, START_VALUE), Some(START_DISABLED));

        let (blocked, detail) = set_usb_blocked_with(&pc, dir.path(), false);
        assert_eq!(detail, None);
        assert!(!blocked);
        // İlke önceden tanımlı değildi; geri alınınca yine tanımsız olmalı.
        assert_eq!(pc.get(POLICY_KEY, POLICY_VALUE), None);
        assert_eq!(pc.get(USBSTOR_KEY, START_VALUE), Some(START_DEMAND));
        assert!(!state_path(dir.path()).exists());
    }

    #[test]
    fn okulun_kendi_kisitlamasi_geri_alinmaz() {
        let dir = tempfile::tempdir().unwrap();
        let pc = Fake::with(&[
            (USBSTOR_KEY, START_VALUE, START_DISABLED),
            (POLICY_KEY, POLICY_VALUE, 1),
        ]);

        set_usb_blocked_with(&pc, dir.path(), true);
        set_usb_blocked_with(&pc, dir.path(), false);
        assert_eq!(pc.get(POLICY_KEY, POLICY_VALUE), Some(1));
        assert_eq!(pc.get(USBSTOR_KEY, START_VALUE), Some(START_DISABLED));

        // Hiç kısıtlamadan "aç" demek de okulun ayarına dokunmaz.
        set_usb_blocked_with(&pc, dir.path(), false);
        assert_eq!(pc.get(USBSTOR_KEY, START_VALUE), Some(START_DISABLED));
    }

    #[test]
    fn baglanti_kopunca_geri_alinir() {
        let dir = tempfile::tempdir().unwrap();
        let pc = normal_pc();
        set_usb_blocked_with(&pc, dir.path(), true);

        // Durum diskte durur: ajan yeniden başlasa da neyi geri alacağını bilir.
        assert!(state_path(dir.path()).exists());
        assert!(release_all_with(&pc, dir.path()));

        assert_eq!(pc.get(POLICY_KEY, POLICY_VALUE), None);
        assert_eq!(pc.get(USBSTOR_KEY, START_VALUE), Some(START_DEMAND));
        // İkinci çağrıda geri alınacak bir şey kalmamıştır.
        assert!(!release_all_with(&pc, dir.path()));
    }

    #[test]
    fn yarim_kalan_kisitlama_birakilmaz() {
        let dir = tempfile::tempdir().unwrap();
        let pc = Fake {
            fail_set: Some(USBSTOR_KEY),
            ..normal_pc()
        };
        let (blocked, detail) = set_usb_blocked_with(&pc, dir.path(), true);
        assert!(detail.is_some_and(|d| d.contains("USB")));
        // İlke yazılmış ama sürücü kapatılamamıştı; ilke geri alınmalı.
        assert_eq!(pc.get(POLICY_KEY, POLICY_VALUE), None);
        // Geri alma da aynı anahtara yazamadığı için kayıt durur ve sonraki
        // `release_all` yeniden dener.
        assert!(blocked);
    }

    #[test]
    fn desteklenmeyen_platformda_kisitlama_uygulanmaz() {
        struct Yok;
        impl System for Yok {
            fn reg_get(&self, _: &str, _: &str) -> Result<Option<u32>, String> {
                Err("desteklenmiyor".into())
            }
            fn reg_set(&self, _: &str, _: &str, _: u32) -> Result<(), String> {
                Err("desteklenmiyor".into())
            }
            fn reg_delete(&self, _: &str, _: &str) -> Result<(), String> {
                Err("desteklenmiyor".into())
            }
        }
        let dir = tempfile::tempdir().unwrap();
        let (blocked, detail) = set_usb_blocked_with(&Yok, dir.path(), true);
        assert!(!blocked);
        assert!(detail.is_some());
        assert!(!state_path(dir.path()).exists());
    }

    #[test]
    fn reg_ciktisindan_dword_okunur() {
        let out = "\r\nHKEY_LOCAL_MACHINE\\SYSTEM\\CurrentControlSet\\Services\\USBSTOR\r\n    Start    REG_DWORD    0x3\r\n\r\n";
        assert_eq!(parse_reg_dword(out), Some(3));
        assert_eq!(parse_reg_dword("    Start    REG_DWORD    0x4"), Some(4));
        assert_eq!(
            parse_reg_dword("ERROR: The system was unable to find"),
            None
        );
    }
}
