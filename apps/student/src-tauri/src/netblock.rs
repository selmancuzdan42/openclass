//! İnternet engelleme (yalnızca Windows).
//!
//! Öğretmen bağlantısı (aynı LAN) kapanmaz: lokalin dışı giden trafiği
//! keserken yerel alt ağ için açık bir izin kuralı tutulur. Böylece öğretmen
//! engeli uzaktan geri açabilir — NetSupport'un "İnterneti kes" davranışı.
//!
//! # İki katman
//!
//! 1. **Güvenlik duvarı** — profilin giden varsayılanını bloklar, yerel
//!    alt ağa izin kuralı ekler. Ağ düzeyinde internet gidişini keser.
//! 2. **[`block_web_via_hosts`]** — `hosts` dosyasına yaygın adresleri
//!    `127.0.0.1`'e yönlendirir ve yerel bir uyarı sayfası sunucusu başlatır.
//!    Böylece tarayıcı o adrese giderken engelin kendisiyle karşılaşmak
//!    yerine "internetiniz kesildi" sayfasını görür.
//!
//! İkisi birden uygulanır: duvar trafiği gerçekten keser, hosts+sunucu ise
//! kullanıcıya *nedenini* açıklayan görünür bir sayfa verir.
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

#[cfg(windows)]
use std::path::{Path, PathBuf};
#[cfg(windows)]
use std::sync::{Mutex, OnceLock};

/// Bu modülün ürettiği tüm kural adlarının ortak öneki.
#[cfg(windows)]
const LAN_RULE: &str = "OpenClassNetBlock-Lan";

/// `hosts` bloğu çifti: bu iki satır arasındaki girdiler OpenClass'a aittir.
#[cfg(windows)]
const HOSTS_START: &str = "# OpenClass internet engeli — baslangic";
#[cfg(windows)]
const HOSTS_END: &str = "# OpenClass internet engeli — son";

/// Uyarı sayfasına yönlendirilecek yaygın adresler.
///
/// Hepsini `127.0.0.1`'e çevirip yerel sunucuda "internetiniz kesildi"
/// sayfasını gösteririz. Liste aradan çoğu tarayıcı isteğini yakalar;
/// duvar zaten geri kalan her şeyi keser.
#[cfg(windows)]
const BLOCKED_HOSTS: &[&str] = &[
    "google.com", "www.google.com", "google.com.tr", "www.google.com.tr",
    "youtube.com", "www.youtube.com", "youtube-nocookie.com",
    "yandex.com", "www.yandex.com", "yandex.com.tr", "www.yandex.com.tr",
    "wikipedia.org", "www.wikipedia.org", "tr.wikipedia.org",
    "instagram.com", "www.instagram.com",
    "facebook.com", "www.facebook.com", "m.facebook.com",
    "x.com", "www.x.com", "twitter.com", "www.twitter.com",
    "gmail.com", "mail.google.com",
    "outlook.com", "www.outlook.com", "hotmail.com",
    "eba.gov.tr", "www.eba.gov.tr", "meb.gov.tr", "www.meb.gov.tr",
    "eksisozluk.com", "www.eksisozluk.com",
    "netflix.com", "www.netflix.com", "spotify.com",
    "tiktok.com", "www.tiktok.com", "reddit.com", "www.reddit.com",
    "linkedin.com", "www.linkedin.com", "whatsapp.com",
    "twitch.tv", "www.twitch.tv", "github.com", "stackoverflow.com",
];

/// Uyarı sunucusunun bağlanacağı adres.
#[cfg(windows)]
const WARNING_ADDR: &str = "127.0.0.1:80";

#[cfg(windows)]
use std::io::{Read, Write};
#[cfg(windows)]
use std::net::{TcpListener, TcpStream};

/// Yerel uyarı sunucusunun tutamacı.
#[cfg(windows)]
struct WarningServer {
    token: tokio_util::sync::CancellationToken,
    handle: Option<std::thread::JoinHandle<()>>,
}

/// Çalışan tek bir uyarı sunucusu; yeniden blokta ikincisi başlatılmaz.
#[cfg(windows)]
static SERVER: OnceLock<Mutex<Option<WarningServer>>> = OnceLock::new();

#[cfg(windows)]
fn server_lock() -> &'static Mutex<Option<WarningServer>> {
    SERVER.get_or_init(|| Mutex::new(None))
}

/// Engeli bu süreç mi uyguladı?
///
/// Bağlantı her koptuğunda `netsh` ile durum sorgulamak zayıf makinede
/// pahalıdır; engel bu oturumda uygulanmadıysa hiçbir komut çalıştırılmaz.
#[cfg(windows)]
static APPLIED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Bu oturumda uygulanan internet engelini kaldırır; kaldırdıysa `true`.
///
/// Engel oturuma bağlıdır: öğretmen bağlantısı kopunca bilgisayar internetsiz
/// kalmaz. Güvenlik duvarı ve `hosts` değişiklikleri kalıcı olduğu için bu
/// yapılmazsa engel yeniden başlatmadan sonra bile sürerdi.
pub fn release_on_disconnect() -> bool {
    #[cfg(windows)]
    {
        if !APPLIED.load(std::sync::atomic::Ordering::Relaxed) {
            return false;
        }
        match set_internet_blocked(false) {
            Ok(_) => true,
            Err(err) => {
                tracing::error!(hata = %err, "internet engeli kaldırılamadı");
                false
            }
        }
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// Önceki çalışmadan kalmış internet engelini kaldırır; kaldırdıysa `true`.
///
/// Ajan çökmüş ya da bilgisayar engel açıkken kapanmış olabilir. Açılışta
/// bir kez çağrılır.
pub fn release_leftover() -> bool {
    if !is_internet_blocked().unwrap_or(false) {
        return false;
    }
    match set_internet_blocked(false) {
        Ok(_) => true,
        Err(err) => {
            tracing::error!(hata = %err, "kalan internet engeli kaldırılamadı");
            false
        }
    }
}

/// İnternet erişimini keser (`block=true`) veya geri açar (`block=false`).
///
/// Güvenlik duvarı + hosts yönlendirmesi + yerel uyarı sayfası bir arada
/// uygulanır. Başarıda güncel engel durumunu `Ok(block)` olarak döner.
/// Yönetim bağlantısı (aynı LAN) açık kalır. Yönetici (admin) yetkisi yoksa
/// ya da `netsh` çalıştırılamazsa hata metni `Err` içinde dönülür.
pub fn set_internet_blocked(block: bool) -> Result<bool, String> {
    #[cfg(windows)]
    {
        if block {
            // Yarıda kalsa bile kopmada geri alınabilsin diye önce işaretlenir.
            APPLIED.store(true, std::sync::atomic::Ordering::Relaxed);
            block_internet_windows()?;
            block_web_via_hosts(true)
        } else {
            unblock_internet_windows()?;
            let result = block_web_via_hosts(false);
            APPLIED.store(false, std::sync::atomic::Ordering::Relaxed);
            result
        }
    }
    #[cfg(not(windows))]
    {
        let _ = block;
        Err("İnternet engelleme yalnızca Windows'ta desteklenir.".to_string())
    }
}

/// `hosts` dosyasına yönlendirme ekler/kaldırır ve yerel uyarı sunucusunu
/// açar/kapatır.
///
/// `block=true`: yaygın adresler `127.0.0.1`'e çevrilir ve `127.0.0.1:80`
/// üzerinde "İnternetiniz kesildi" sayfası yayına başlar. `block=false`:
/// yönlendirmeler kaldırılır, sunucu durdurulur.
///
/// Başarıda yeni engel durumunu `Ok(block)` döner. Windows dışı her zaman
/// `Err` (bu işlev Windows'a özgüdür).
///
/// Windows üzerindeki çağrısı [`set_internet_blocked`] içinden gelir; diğer
/// platformlarda ölü (no-op) olduğu için dead-code uyarısı bastırılır.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn block_web_via_hosts(block: bool) -> Result<bool, String> {
    #[cfg(windows)]
    {
        if block {
            apply_hosts_redirect()?;
            start_warning_server()?;
            Ok(true)
        } else {
            revert_hosts_redirect()?;
            stop_warning_server();
            Ok(false)
        }
    }
    #[cfg(not(windows))]
    {
        let _ = block;
        Err("Web yönlendirmesi yalnızca Windows'ta desteklenir.".to_string())
    }
}

/// Şu an internet engelinin aktif olup olmadığını döndürür.
///
/// Engel, LAN-izin kuralının varlığıyla ya da `hosts` engel bloğuyla izlenir.
pub fn is_internet_blocked() -> Result<bool, String> {
    #[cfg(windows)]
    {
        Ok(is_internet_blocked_windows() || hosts_marker_present())
    }
    #[cfg(not(windows))]
    {
        Ok(false)
    }
}

// ------------------------------------------------------------- hosts katmanı

/// Windows `hosts` dosyasının tam yolu.
#[cfg(windows)]
fn hosts_path() -> PathBuf {
    let root = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".into());
    Path::new(&root)
        .join("System32")
        .join("drivers")
        .join("etc")
        .join("hosts")
}

/// `hosts` içeriğinde OpenClass engel bloğu var mı?
#[cfg(windows)]
fn hosts_marker_present() -> bool {
    let content = match std::fs::read_to_string(hosts_path()) {
        Ok(c) => c,
        Err(_) => return false,
    };
    content.contains(HOSTS_START) && content.contains(HOSTS_END)
}

/// Engel bloğunu içeren yönlendirme satırlarını üretir.
#[cfg(windows)]
fn block_lines(existing: &str) -> String {
    // Kalıntı bir engel bloğu varsa önce temizle (idempotent yeniden blok).
    let cleaned = strip_block(existing);
    let mut out = cleaned;
    if !out.ends_with('\n') && !out.is_empty() {
        out.push('\n');
    }
    out.push_str(HOSTS_START);
    out.push('\n');
    for host in BLOCKED_HOSTS {
        // Hem IPv4 hem IPv6 eşleşmesi için iki satır.
        out.push_str(&format!("127.0.0.1\t{host}\n"));
        out.push_str(&format!("::1\t{host}\n"));
    }
    out.push_str(HOSTS_END);
    out.push('\n');
    out
}

/// `hosts` içeriğinden engel bloğunu çıkarır.
#[cfg(windows)]
fn strip_block(content: &str) -> String {
    let mut out = String::new();
    let mut in_block = false;
    for line in content.lines() {
        if line.trim() == HOSTS_START {
            in_block = true;
            continue;
        }
        if line.trim() == HOSTS_END {
            in_block = false;
            continue;
        }
        if in_block {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// `hosts` dosyasını atomik biçimde yazar.
#[cfg(windows)]
fn write_hosts(content: &str) -> Result<(), String> {
    use std::io::Write as _;

    let path = hosts_path();
    let tmp = path.with_extension("openclass.tmp");
    let write_result = (|| -> std::io::Result<()> {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(content.as_bytes())?;
        f.sync_all()?;
        std::fs::rename(&tmp, &path)?;
        Ok(())
    })();
    let _ = std::fs::remove_file(&tmp);
    write_result.map_err(|e| format!("hosts yazılamadı ({}): {e}", path.display()))?;

    // Tarayıcı/DNS önbelleğini tazele; yönlendirme anında etkin olsun.
    let _ = std::process::Command::new("ipconfig")
        .arg("/flushdns")
        .output();
    Ok(())
}

/// Engeli `hosts` katmanında uygular.
#[cfg(windows)]
fn apply_hosts_redirect() -> Result<(), String> {
    let existing = std::fs::read_to_string(hosts_path()).unwrap_or_default();
    let next = block_lines(&existing);
    if next != existing {
        write_hosts(&next)?;
    }
    tracing::info!(adet = BLOCKED_HOSTS.len(), "hosts yönlendirme uygulandı");
    Ok(())
}

/// Engeli `hosts` katmanından kaldırır.
#[cfg(windows)]
fn revert_hosts_redirect() -> Result<(), String> {
    let existing = std::fs::read_to_string(hosts_path()).unwrap_or_default();
    let next = strip_block(&existing);
    if next != existing {
        write_hosts(&next)?;
    }
    tracing::info!("hosts yönlendirme kaldırıldı");
    Ok(())
}

// ------------------------------------------------------ yerel uyarı sunucusu

/// "İnternetiniz kesildi" sayfasının tamamı.
#[cfg(windows)]
const WARNING_HTML: &str = "<!DOCTYPE html>
<html lang=\"tr\">
<head>
<meta charset=\"utf-8\">
<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">
<title>İnternet Bağlantısı Kesildi</title>
<style>
  body{font-family:'Segoe UI',system-ui,sans-serif;background:#f2f4f8;color:#1f2933;
       display:flex;align-items:center;justify-content:center;height:100vh;margin:0}
  .kutu{background:#fff;padding:48px 56px;border-radius:16px;box-shadow:0 12px 40px rgba(0,0,0,.12);
        max-width:520px;text-align:center}
  .ikon{font-size:64px;line-height:1}
  h1{font-size:24px;margin:16px 0 8px}
  p{color:#52606d;line-height:1.6;margin:8px 0}
  .bayrak{margin-top:24px;padding:12px 16px;background:#f8d7da;color:#a12b38;
          border-radius:8px;font-size:14px;text-align:center}
</style>
</head>
<body>
<div class=\"kutu\">
  <div class=\"ikon\">🚫</div>
  <h1>İnternet bağlantısı kesildi</h1>
  <p>Öğretmeniniz ders süresince internet erişimini kapatmış olabilir.
     Bu sayfa OpenClass sınıf yönetimi tarafından gösterilmektedir.</p>
  <div class=\"bayrak\">Ağ erişimi, öğretmen tarafından yeniden açılana kadar
     kısıtlıdır. Yerel ağ (Öğretmen bağlantısı) etkilenmez.</div>
</div>
</body>
</html>";

/// Tek bir bağlantıya uyarı sayfasını yanıtlar.
#[cfg(windows)]
fn serve_warning(stream: &mut TcpStream) {
    // İstek başlığını okumaya çalış (best-effort); yanıt hemen verilir.
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_millis(200)));
    let _ = stream.set_write_timeout(Some(std::time::Duration::from_secs(2)));
    let mut buf = [0u8; 1024];
    let _ = stream.read(&mut buf);

    let response = format!(
        "HTTP/1.1 200 OK\r\n\
         Content-Type: text/html; charset=utf-8\r\n\
         Cache-Control: no-store\r\n\
         Connection: close\r\n\
         Content-Length: {}\r\n\r\n{}",
        WARNING_HTML.len(),
        WARNING_HTML
    );
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
}

/// Yerel uyarı sunucusunu başlatır. Zaten çalışıyorsa yeniden başlatmaz.
#[cfg(windows)]
fn start_warning_server() -> Result<(), String> {
    let mut guard = server_lock().lock().expect("sunucu kilit düzgün");
    if guard.is_some() {
        return Ok(());
    }

    let listener = TcpListener::bind(WARNING_ADDR)
        .map_err(|e| format!("uyarı sayfası sunucusu bağlanamadı ({WARNING_ADDR}): {e}"))?;
    // Dinleyiciyi engellemesiz yap: iptal denetimi beklemeyi keser.
    listener
        .set_nonblocking(true)
        .map_err(|e| format!("sunucu engellemesiz moda geçemedi: {e}"))?;

    let token = tokio_util::sync::CancellationToken::new();
    let t = token.clone();
    let handle = std::thread::Builder::new()
        .name("openclass-warning-server".into())
        .spawn(move || {
            loop {
                if t.is_cancelled() {
                    break;
                }
                match listener.accept() {
                    Ok((mut stream, _)) => serve_warning(&mut stream),
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(std::time::Duration::from_millis(50));
                    }
                    Err(_) => break,
                }
            }
        })
        .map_err(|e| format!("uyarı sunucusu iş parçacığı başlatılamadı: {e}"))?;

    *guard = Some(WarningServer {
        token,
        handle: Some(handle),
    });
    tracing::info!("yerel uyarı sayfası sunucusu başladı ({WARNING_ADDR})");
    Ok(())
}

/// Yerel uyarı sunucusunu durdurur (varsa).
#[cfg(windows)]
fn stop_warning_server() {
    let mut guard = server_lock().lock().expect("sunucu kilit düzgün");
    if let Some(server) = guard.take() {
        server.token.cancel();
        if let Some(h) = server.handle {
            let _ = h.join();
        }
        tracing::info!("yerel uyarı sayfası sunucusu durdu");
    }
}

// ------------------------------------------------------------- duvar katmanı

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
fn is_internet_blocked_windows() -> bool {
    let Ok(out) = run_netsh(&[
        "advfirewall",
        "firewall",
        "show",
        "rule",
        &format!("name={LAN_RULE}"),
    ]) else {
        return false;
    };
    out.contains(LAN_RULE)
}

/// `netsh` komutunu çalıştırır; başarıda stdout'u, başarısızlıkta hata döndürür.
#[cfg(windows)]
fn run_netsh(args: &[&str]) -> Result<String, String> {
    use std::process::{Command, Output};
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

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn engel_blok_satirlar_uretilir_ve_temizlenir() {
        let ornek = "127.0.0.1 localhost\n";
        let eklendi = block_lines(ornek);
        assert!(eklendi.starts_with("127.0.0.1 localhost\n"));
        assert!(eklendi.contains(HOSTS_START));
        assert!(eklendi.contains("127.0.0.1\tgoogle.com"));
        assert!(eklendi.contains("::1\tyoutube.com"));
        assert!(eklendi.contains(HOSTS_END));

        // Tekrar eklemek idempotent: blok varsa üzerine ikincisi yığılmaz.
        let tekrar = block_lines(&eklendi);
        assert_eq!(tekrar.matches(HOSTS_START).count(), 1);

        // Temizleme bloğu ve hepsini geri alır.
        let temiz = strip_block(&eklendi);
        assert!(!temiz.contains(HOSTS_START));
        assert!(!temiz.contains("google.com"));
        assert!(temiz.contains("127.0.0.1 localhost\n"));
    }

    #[test]
    fn hosts_marker_sadece_ikisi_degilse_yok_sayilir() {
        // Yalnızca başlangıç satırı engel anlamına gelmez.
        let yarim = format!("{HOSTS_START}\n127.0.0.1\tx");
        assert!(!(yarim.contains(HOSTS_START) && yarim.contains(HOSTS_END)));
        let tam = format!("{HOSTS_START}\ny\n{HOSTS_END}");
        assert!(tam.contains(HOSTS_START) && tam.contains(HOSTS_END));
    }
}
