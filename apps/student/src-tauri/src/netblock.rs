//! İnternet engelleme (yalnızca Windows).
//!
//! Öğretmen bağlantısı (aynı LAN) kapanmaz: lokalin dışı giden trafiği
//! keserken yerel alt ağ için açık bir izin kuralı tutulur. Böylece öğretmen
//! engeli uzaktan geri açabilir — NetSupport'un "İnterneti kes" davranışı.
//!
//! # Neden blok kuralına `remoteip=0.0.0.0/0` yerine profil varsayılanı
//!
//! Windows Defender Güvenlik Duvarı'nda **açık blok kuralları çakışan izin
//! kurallarından üstündür** (Microsoft kural önceliği: blok > izin, ve daha
//! spesifik izin bile geniş bir bloğa yenilir). Bu yüzden "her yeri blokla +
//! öğretmeni/alt ağı izin ver" şeklinde tek kural işe yaramaz — geniş blok,
//! öğretmen kuralını da boğardı. Bunun yerine profilin giden varsayılanını
//! `block` yapıp yerel alt ağa (`localsubnet`) bir izin kuralı ekliyoruz:
//! LAN (öğretmen dahil) izinli kalır, geri kalan her şey (internet) kesilir.

/// Bu modülün ürettiği tüm kural adlarının ortak öneki.
#[cfg(windows)]
const LAN_RULE: &str = "OpenClassNetBlock-Lan";

#[cfg(windows)]
use std::process::{Command, Output};

/// İnternet erişimini keser (`block=true`) veya geri açar (`block=false`).
///
/// Başarıda güncel engel durumunu `Ok(block)` olarak döner. Yönetim
/// bağlantısı (aynı LAN) açık kalır. Yönetici (admin) yetkisi yoksa ya da
/// `netsh` çalıştırılamazsa hata metni `Err` içinde dönülür.
pub fn set_internet_blocked(block: bool) -> Result<bool, String> {
    #[cfg(windows)]
    {
        if block {
            block_internet_windows()
        } else {
            unblock_internet_windows()
        }
    }
    #[cfg(not(windows))]
    {
        let _ = block;
        Err("İnternet engelleme yalnızca Windows'ta desteklenir.".to_string())
    }
}

/// Şu an internet engelinin aktif olup olmadığını döndürür.
///
/// Engel, LAN-izin kuralının varlığıyla izlenir: kural varken engel aktiftir.
pub fn is_internet_blocked() -> Result<bool, String> {
    #[cfg(windows)]
    {
        is_internet_blocked_windows()
    }
    #[cfg(not(windows))]
    {
        Ok(false)
    }
}

/// İnterneti keser: LAN-izin kuralı ekler, profilin giden varsayılanını bloklar.
#[cfg(windows)]
fn block_internet_windows() -> Result<bool, String> {
    // Tekrar engellemek idempotent olsun diye önce kalıntı izin kuralını temizle.
    let _ = run_netsh(&[
        "advfirewall",
        "firewall",
        "delete",
        "rule",
        &format!("name={LAN_RULE}"),
    ]);

    // Öğretmen/yerel alt ağ her zaman açık kalsın.
    run_netsh(&[
        "advfirewall",
        "firewall",
        "add",
        "rule",
        &format!("name={LAN_RULE}"),
        "dir=out",
        "action=allow",
        "remoteip=localsubnet",
        "profile=any",
    ])?;

    // Lokalin dışı tüm giden trafiği (internet) blokla.
    run_netsh(&[
        "advfirewall",
        "set",
        "allprofiles",
        "firewallpolicy",
        "blockinbound,blockoutbound",
    ])
    .map_err(|_| {
        "İnternet engellenemedi. Yönetici (admin) olarak çalıştırılmıyor olabilir.".to_string()
    })?;

    Ok(true)
}

/// İnterneti geri açar: LAN-izin kuralını siler, profilin giden varsayılanını döndürür.
#[cfg(windows)]
fn unblock_internet_windows() -> Result<bool, String> {
    let _ = run_netsh(&[
        "advfirewall",
        "firewall",
        "delete",
        "rule",
        &format!("name={LAN_RULE}"),
    ]);

    // HTTPS/DNS gibi her şeyi dışarı vererek önceki ayara dön.
    run_netsh(&[
        "advfirewall",
        "set",
        "allprofiles",
        "firewallpolicy",
        "blockinbound,allowoutbound",
    ])?;

    Ok(false)
}

/// LAN-izin kuralının varlığını sorgulayarak engel durumunu belirler.
#[cfg(windows)]
fn is_internet_blocked_windows() -> Result<bool, String> {
    let out = run_netsh(&[
        "advfirewall",
        "firewall",
        "show",
        "rule",
        &format!("name={LAN_RULE}"),
    ])?;
    Ok(out.contains(LAN_RULE))
}

/// `netsh` komutunu çalıştırır; başarıda stdout'u, başarısızlıkta hata döndürür.
#[cfg(windows)]
fn run_netsh(args: &[&str]) -> Result<String, String> {
    match Command::new("netsh").args(args).output() {
        Ok(Output { status, stdout, .. }) if status.success() => {
            Ok(String::from_utf8_lossy(&stdout).into_owned())
        }
        Ok(Output {
            status,
            stdout,
            stderr,
            ..
        }) => {
            let err = String::from_utf8_lossy(&stderr).into_owned();
            let out = String::from_utf8_lossy(&stdout).into_owned();
            Err(format!(
                "netsh başarısız (kod: {:?}): {err} {out}",
                status.code()
            ))
        }
        Err(err) => Err(format!("netsh çalıştırılamadı: {err}")),
    }
}
