//! OpenClass ağ protokolü.
//!
//! Bu crate hem öğretmen (`openclass-teacher`) hem öğrenci (`openclass-student`)
//! uygulaması tarafından kullanılır ve aralarındaki *tek* sözleşmedir.
//!
//! Protokol üç katmana ayrılmıştır (bkz. `docs/ARCHITECTURE.md`):
//!
//! 1. **Keşif** (`discovery`) — şifresiz UDP. Yalnızca "kim nerede" bilgisini taşır,
//!    hiçbir gizli veri içermez ve hiçbir yetki vermez.
//! 2. **Kimlik doğrulama** (`auth`) — eşleştirme kanıtı ve challenge imzalama.
//! 3. **Kontrol/veri** (`control`) — TLS üzerinde WebSocket, MessagePack çerçeveleri.
//!
//! Keşif ile yetkilendirme bilinçli olarak ayrıdır: bir cihazın kendini `LAB101`
//! olarak duyurması onu yönetilebilir yapmaz.

pub mod auth;
pub mod control;
pub mod discovery;
pub mod multicast;

pub use control::{
    BrowseEntry, BrowseRoot, ClientMessage, InputEvent, MouseButton, MulticastInfo, ScreenRect,
    ServerMessage, StreamQuality,
};
pub use discovery::DiscoveryPacket;

/// Protokol sürümü. Uyuşmayan sürümler bağlantıyı reddeder.
pub const PROTOCOL_VERSION: u16 = 1;

/// Öğretmen konsolunun TLS + WebSocket dinlediği varsayılan TCP portu.
pub const DEFAULT_CONTROL_PORT: u16 = 7643;

/// Öğrenci ajanlarının keşif için dinlediği UDP portu.
pub const DEFAULT_DISCOVERY_PORT: u16 = 7644;

/// Öğretmen konsolunun keşif soketini bağladığı UDP portu.
///
/// Sabit olması güvenlik duvarı kuralı yazılabilmesi içindir: öğrencilerin
/// cevabı bu porta unicast döner ve gelen UDP trafiğine izin verilmesi
/// gerekir. Gelişigüzel bir port kullanılsaydı kural yazılamazdı.
///
/// Port meşgulse (ör. aynı makinede ikinci bir öğretmen konsolu) gelişigüzel
/// porta düşülür; o durumda keşif yalnızca güvenlik duvarı kapalıyken veya
/// elle adres girilerek çalışır.
pub const DEFAULT_TEACHER_DISCOVERY_PORT: u16 = 7645;

/// Bir UDP keşif datagramının kabul edilen azami boyutu.
pub const MAX_DISCOVERY_PACKET: usize = 1400;

/// Bir kontrol/veri çerçevesinin kabul edilen azami boyutu (8 MiB).
/// Tek bir ekran karesi bunun çok altında kalmalıdır; sınır kötü niyetli
/// veya bozuk göndericilere karşı bellek koruması sağlar.
pub const MAX_CONTROL_FRAME: usize = 8 * 1024 * 1024;

/// Yeniden başlatma, USB kısıtlaması gibi 0.5.1 ile gelen komutları
/// anlayan en eski ajan sürümü.
///
/// Daha eski ajanlar tanımadıkları mesajı çözemeyip bağlantıyı koparır; bu
/// yüzden konsol bu komutları yalnızca destekleyen ajanlara gönderir.
pub const MIN_AGENT_FOR_0_5_1_COMMANDS: (u32, u32, u32) = (0, 5, 1);

/// `app_version` metni (`0.5.1`, `0.6.0-beta`) verilen asgari sürümü karşılıyor mu?
///
/// Çözülemeyen sürüm "karşılamıyor" sayılır: bilinmeyen bir ajana yeni komut
/// göndermek bağlantısını koparabilir.
pub fn version_at_least(version: &str, min: (u32, u32, u32)) -> bool {
    let core = version.trim().split(['-', '+']).next().unwrap_or("");
    let mut parts = core.split('.').map(|p| p.parse::<u32>());
    let (Some(Ok(major)), Some(Ok(minor)), Some(Ok(patch)), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return false;
    };
    (major, minor, patch) >= min
}

/// WebSocket el sıkışmasında kullanılan yol.
pub const WS_PATH: &str = "/openclass/v1";

#[derive(Debug, thiserror::Error)]
pub enum ProtocolError {
    #[error("çerçeve çözümlenemedi: {0}")]
    Decode(#[from] rmp_serde::decode::Error),

    #[error("çerçeve kodlanamadı: {0}")]
    Encode(#[from] rmp_serde::encode::Error),

    #[error("çerçeve çok büyük: {size} bayt (azami {max})")]
    TooLarge { size: usize, max: usize },

    #[error("protokol sürümü uyuşmuyor: karşı taraf v{theirs}, biz v{ours}")]
    VersionMismatch { theirs: u16, ours: u16 },

    #[error("kimlik doğrulama başarısız: {0}")]
    Auth(#[from] auth::AuthError),
}

/// Bir mesajı MessagePack ile kodlar.
///
/// Base64 kullanılmaz; ekran kareleri `serde_bytes` sayesinde ham ikili olarak
/// taşınır (bkz. `control::ClientMessage::ScreenFrame`).
pub fn encode<T: serde::Serialize>(value: &T, max: usize) -> Result<Vec<u8>, ProtocolError> {
    let bytes = rmp_serde::to_vec_named(value)?;
    if bytes.len() > max {
        return Err(ProtocolError::TooLarge {
            size: bytes.len(),
            max,
        });
    }
    Ok(bytes)
}

/// MessagePack ile kodlanmış bir mesajı çözer.
pub fn decode<T: serde::de::DeserializeOwned>(
    bytes: &[u8],
    max: usize,
) -> Result<T, ProtocolError> {
    if bytes.len() > max {
        return Err(ProtocolError::TooLarge {
            size: bytes.len(),
            max,
        });
    }
    Ok(rmp_serde::from_slice(bytes)?)
}

/// Kriptografik olarak güvenli rastgele baytlar üretir.
pub fn random_bytes<const N: usize>() -> [u8; N] {
    let mut buf = [0u8; N];
    getrandom::fill(&mut buf).expect("işletim sistemi rastgelelik kaynağı kullanılamıyor");
    buf
}

#[cfg(test)]
mod tests {
    use super::version_at_least;

    #[test]
    fn surum_karsilastirmasi() {
        assert!(version_at_least("0.5.1", (0, 5, 1)));
        assert!(version_at_least("0.6.0", (0, 5, 1)));
        assert!(version_at_least("1.0.0-beta", (0, 5, 1)));
        assert!(!version_at_least("0.5.0", (0, 5, 1)));
        assert!(!version_at_least("0.4.12", (0, 5, 1)));
        // Sayısal karşılaştırma: metin sırası 0.10.0'ı 0.5.1'den küçük sayardı.
        assert!(version_at_least("0.10.0", (0, 5, 1)));
    }

    #[test]
    fn cozulemeyen_surum_desteklemiyor_sayilir() {
        assert!(!version_at_least("", (0, 5, 1)));
        assert!(!version_at_least("bilinmiyor", (0, 5, 1)));
        assert!(!version_at_least("0.5", (0, 5, 1)));
        assert!(!version_at_least("0.5.1.2", (0, 5, 1)));
    }
}
