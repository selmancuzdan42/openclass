//! Yerel ağ keşif katmanı (şifresiz UDP).
//!
//! # Akış
//!
//! 1. Öğretmen konsolu `TeacherProbe` paketini yayın (broadcast) adresine gönderir.
//! 2. Keşif portunu dinleyen öğrenci ajanları, probe'un geldiği adrese **unicast**
//!    `StudentAnnounce` cevabı döner.
//! 3. Öğretmen cevapları toplar, hostname'e göre filtreler ve listeler.
//!
//! # Güvenlik sınırı
//!
//! Bu katman **hiçbir yetki vermez** ve hiçbir gizli veri taşımaz. Buradaki
//! hostname beyanı doğrulanmamış bir iddiadır; cihazın gerçekten yönetilebilir
//! olması için `auth` katmanından geçmesi gerekir.
//!
//! Bazı okul ağları broadcast/multicast paketlerini engeller. Bu durumda öğrenci
//! ajanı yapılandırılmış sabit öğretmen adresine doğrudan bağlanır; keşif katmanı
//! tamamen atlanabilir.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Paketlerin başına konan sihirli imza. Yanlış porta düşen trafiği ucuza eler.
pub const MAGIC: &[u8; 4] = b"OCL1";

/// Bir probe/announce çiftini eşleştiren rastgele değer.
pub type Nonce = [u8; 16];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "t")]
pub enum DiscoveryPacket {
    /// Öğretmen → yayın adresi. "Buradayım, kimler var?"
    TeacherProbe {
        protocol: u16,
        /// Öğretmen konsolunun kalıcı kimliği.
        teacher_id: Uuid,
        /// Kullanıcıya gösterilecek ad (ör. "Cihan Güngör — Lab 1").
        teacher_name: String,
        /// Kontrol kanalının dinlendiği TCP portu.
        control_port: u16,
        /// Öğretmen TLS sertifikasının SHA-256 parmak izi.
        /// Öğrenci bunu bağlantı kurarken sabitler (pinning).
        #[serde(with = "serde_bytes")]
        cert_fingerprint: Vec<u8>,
        nonce: Nonce,
        /// Konsolun yönettiği laboratuvar/oda adı.
        ///
        /// Öğrenci, kapsamına girmediği konsola kendiliğinden başvurmaz
        /// (bkz. [`scope_match`]). `#[serde(default)]` zorunlu: eski konsollar
        /// bu alanı göndermez ve kapsamı "tümü" sayılır.
        #[serde(default)]
        room: Option<String>,
        /// Konsolun yönettiği bilgisayar adı ön eki (`LAB1`).
        #[serde(default)]
        prefix: Option<String>,
    },

    /// Öğrenci → öğretmen (unicast cevap). "Ben LAB101'im."
    StudentAnnounce {
        protocol: u16,
        /// Cihazın kalıcı kimliği. IP değişse de sabit kalır.
        device_id: Uuid,
        /// Windows bilgisayar adı. **Doğrulanmamış beyandır.**
        hostname: String,
        app_version: String,
        /// Probe'daki nonce aynen geri döner.
        nonce: Nonce,
        /// Cihaz daha önce bir öğretmenle eşleştiyse o öğretmenin kimliği.
        /// Konsol "yetkili / yetki bekliyor / başka öğretmene bağlı" ayrımı için kullanır.
        paired_to: Option<Uuid>,
        /// Öğrencinin bildirdiği laboratuvar/oda adı. **Doğrulanmamış beyandır.**
        ///
        /// Keşif paketinde taşınması, öğretmenin bir odayı daha bağlantı
        /// kurmadan süzebilmesi içindir: "Lab 1'i yönet" diyen konsol, o
        /// odayı bildiren cihazları listeler. `Hello` ile gelen değerle aynı
        /// kaynaktan okunur.
        ///
        /// `#[serde(default)]` zorunlu: 0.1.0 ajanları bu alanı göndermez.
        #[serde(default)]
        room: Option<String>,
    },
}

impl DiscoveryPacket {
    /// Paketi `MAGIC` ön eki ile birlikte kodlar.
    pub fn encode(&self) -> Result<Vec<u8>, crate::ProtocolError> {
        let body = crate::encode(self, crate::MAX_DISCOVERY_PACKET - MAGIC.len())?;
        let mut out = Vec::with_capacity(MAGIC.len() + body.len());
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&body);
        Ok(out)
    }

    /// Gelen datagramı çözer. İmzası tutmayan paketler `None` döner —
    /// bunlar hata değil, yalnızca bizim olmayan trafiktir.
    pub fn decode(datagram: &[u8]) -> Option<Result<Self, crate::ProtocolError>> {
        let body = datagram.strip_prefix(MAGIC.as_slice())?;
        Some(crate::decode(body, crate::MAX_DISCOVERY_PACKET))
    }

    /// Paketin taşıdığı protokol sürümü.
    pub fn protocol(&self) -> u16 {
        match self {
            Self::TeacherProbe { protocol, .. } | Self::StudentAnnounce { protocol, .. } => {
                *protocol
            }
        }
    }
}

/// Hostname filtresi.
///
/// Okulların adlandırma düzenleri farklıdır (`LAB101`, `BIL101`, `PC-101`,
/// `ATOLYE01`, `SINIF-A-01`), bu yüzden hiçbir desen koda gömülmez.
/// Varsayılan davranış büyük/küçük harf duyarsız ön ek eşleşmesidir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostnameFilter {
    /// Boş sorgu: her şey eşleşir.
    Any,
    /// `LAB1` → `LAB101`, `LAB102`, ... (ve `LAB1`'in kendisi).
    Prefix(String),
    /// Yalnızca birebir aynı ad.
    Exact(String),
    /// Adın herhangi bir yerinde geçmesi yeterli.
    Contains(String),
}

/// Bir cihazın, öğretmen konsolunun yönettiği kapsama girip girmediği.
///
/// NetSupport'un "Room" ve "Browse" kipleriyle aynı mantık: konsol bir oda
/// adı ve/veya bilgisayar adı ön eki seçer; yalnızca bunlara uyan cihazlar
/// o konsola aittir. Ön ek, bilgisayar adında ya da öğrencinin girdiği oda
/// adında aranır. İkisi birlikte verilirse ikisi de sağlanmalıdır; ikisi
/// de boşsa kapsam "tümü"dür ve her cihaz uyar.
///
/// Aynı kural üç yerde uygulanır ve aynı kalmalıdır: öğrencinin hangi konsola
/// başvuracağı, konsolun kimi kendiliğinden yetkilendireceği ve arayüzün
/// listede kimi göstereceği.
///
/// Uymuyorsa `None`; uyuyorsa kapsamın ne kadar dar olduğunu gösteren bir
/// puan döner. Birden çok konsol uyduğunda öğrenci en dar kapsamlı olanı
/// seçer: `LAB1001`, `LAB1` yerine `LAB10` konsoluna gider.
pub fn scope_match(
    room: Option<&str>,
    prefix: Option<&str>,
    hostname: &str,
    device_room: Option<&str>,
) -> Option<usize> {
    let norm = |s: &str| s.trim().to_uppercase();
    let room = room.map(norm).filter(|r| !r.is_empty());
    let prefix = prefix.map(norm).filter(|p| !p.is_empty());

    let mut score = 0;
    if let Some(room) = room {
        if device_room.map(norm).as_deref() != Some(room.as_str()) {
            return None;
        }
        // Oda eşleşmesi her ön ekten daha belirleyicidir.
        score += 1_000;
    }
    if let Some(prefix) = prefix {
        // Ön ek bilgisayar adında aranır; uymuyorsa öğrencinin girdiği oda
        // adına da bakılır. Windows bilgisayar adını değiştirmek yeniden
        // başlatma ister ve her okulda yapılamaz; ajan penceresine `LAB2`
        // yazmak aynı sonucu vermelidir.
        let by_name = norm(hostname).starts_with(&prefix);
        let by_room = device_room.is_some_and(|r| norm(r).starts_with(&prefix));
        if !by_name && !by_room {
            return None;
        }
        score += prefix.chars().count();
    }
    Some(score)
}

/// Bir cihazın arama sorgusuyla eşleşip eşleşmediğine karar verir.
///
/// Öğretmen arama kutusuna hem bilgisayar adı (`LAB1`) hem IP parçası
/// (`192.168.1.`) yazabilir. Hangisi olduğunu önceden sormak yerine ikisi de
/// denenir: sorgu rakam ve noktadan oluşuyorsa adreste, değilse bilgisayar
/// adında aranır. Her iki durumda da diğerine de bakılır, çünkü `192` gibi
/// bir sorgu her ikisine de uyabilir.
pub fn matches_device(query: &str, hostname: &str, address: Option<&str>) -> bool {
    let q = query.trim();
    if q.is_empty() {
        return true;
    }
    if HostnameFilter::prefix(q).matches(hostname) {
        return true;
    }
    address
        .map(|a| a.to_ascii_uppercase().contains(&q.to_ascii_uppercase()))
        .unwrap_or(false)
}

impl HostnameFilter {
    /// Öğretmenin arama kutusuna yazdığı metinden varsayılan (ön ek) filtre üretir.
    pub fn prefix(query: &str) -> Self {
        let q = query.trim();
        if q.is_empty() {
            Self::Any
        } else {
            Self::Prefix(q.to_ascii_uppercase())
        }
    }

    pub fn matches(&self, hostname: &str) -> bool {
        let h = hostname.trim().to_ascii_uppercase();
        match self {
            Self::Any => true,
            Self::Prefix(p) => h.starts_with(p),
            Self::Exact(e) => h == *e,
            Self::Contains(c) => h.contains(c.as_str()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ip_parcasiyla_arama_calisir() {
        assert!(matches_device(
            "192.168.1.",
            "LAB101",
            Some("192.168.1.15:7643")
        ));
        assert!(matches_device("1.15", "LAB101", Some("192.168.1.15:7643")));
        assert!(!matches_device(
            "10.0.",
            "LAB101",
            Some("192.168.1.15:7643")
        ));
    }

    #[test]
    fn ad_ve_ip_aramasi_birlikte_calisir() {
        // Ad eşleşiyorsa adres bakılmadan kabul edilir.
        assert!(matches_device("LAB1", "LAB101", Some("10.0.0.5:7643")));
        // Adresi olmayan cihaz yalnızca adla bulunur.
        assert!(matches_device("LAB1", "LAB101", None));
        assert!(!matches_device("192", "LAB101", None));
    }

    #[test]
    fn bos_sorgu_her_cihazi_dondurur() {
        assert!(matches_device("  ", "LAB101", None));
    }

    #[test]
    fn prefix_filtresi_lab1_ve_lab2_yi_ayirir() {
        let lab1 = HostnameFilter::prefix("LAB1");
        assert!(lab1.matches("LAB101"));
        assert!(lab1.matches("LAB120"));
        assert!(!lab1.matches("LAB201"));
        assert!(!lab1.matches("LAB220"));
    }

    #[test]
    fn arama_buyuk_kucuk_harf_duyarsizdir() {
        let f = HostnameFilter::prefix("lab1");
        assert!(f.matches("LAB105"));
        assert!(f.matches("lab105"));
        assert!(f.matches("Lab105"));
    }

    #[test]
    fn tam_bilgisayar_adi_yalnizca_kendini_bulur() {
        let f = HostnameFilter::prefix("LAB105");
        assert!(f.matches("LAB105"));
        assert!(!f.matches("LAB104"));
        // Ön ek eşleşmesi olduğu için LAB1050 da eşleşir; birebir arama için Exact kullanılır.
        assert!(f.matches("LAB1050"));
        assert!(!HostnameFilter::Exact("LAB105".into()).matches("LAB1050"));
    }

    #[test]
    fn bos_sorgu_tum_cihazlari_dondurur() {
        let f = HostnameFilter::prefix("   ");
        assert!(f.matches("LAB101"));
        assert!(f.matches("ATOLYE01"));
    }

    #[test]
    fn farkli_okul_adlandirmalari_desteklenir() {
        assert!(HostnameFilter::prefix("BIL").matches("BIL101"));
        assert!(HostnameFilter::prefix("PC-").matches("PC-101"));
        assert!(HostnameFilter::prefix("SINIF-A").matches("SINIF-A-01"));
        assert!(!HostnameFilter::prefix("SINIF-A").matches("SINIF-B-01"));
    }

    #[test]
    fn paket_kodlanip_cozulebilir() {
        let p = DiscoveryPacket::StudentAnnounce {
            protocol: crate::PROTOCOL_VERSION,
            device_id: Uuid::nil(),
            hostname: "LAB101".into(),
            app_version: "0.1.0".into(),
            nonce: [7u8; 16],
            paired_to: None,
            room: Some("Lab 1".into()),
        };
        let raw = p.encode().unwrap();
        assert!(raw.starts_with(MAGIC));
        let back = DiscoveryPacket::decode(&raw).unwrap().unwrap();
        match back {
            DiscoveryPacket::StudentAnnounce {
                hostname, nonce, ..
            } => {
                assert_eq!(hostname, "LAB101");
                assert_eq!(nonce, [7u8; 16]);
            }
            _ => panic!("yanlış paket tipi"),
        }
    }

    #[test]
    fn kapsam_on_eke_gore_laboratuvarlari_ayirir() {
        assert!(scope_match(None, Some("LAB1"), "LAB101", None).is_some());
        assert!(scope_match(None, Some("lab1"), "Lab120", None).is_some());
        assert!(scope_match(None, Some("LAB1"), "LAB201", None).is_none());
        assert!(scope_match(None, Some("BIL"), "BIL101", None).is_some());
        // Bilgisayar adı uymuyor ama öğrenci oda adına sınıfın adını yazmış.
        assert!(scope_match(None, Some("LAB2"), "DESKTOP-1QSVN3M", Some("lab2")).is_some());
        assert!(scope_match(None, Some("LAB2"), "DESKTOP-1QSVN3M", Some("LAB1")).is_none());
        assert!(scope_match(None, Some("LAB2"), "DESKTOP-1QSVN3M", None).is_none());
    }

    #[test]
    fn kapsam_odaya_gore_ayirir() {
        assert!(scope_match(Some("Lab 1"), None, "DESKTOP-X", Some(" lab 1 ")).is_some());
        assert!(scope_match(Some("Lab 1"), None, "DESKTOP-X", Some("Lab 2")).is_none());
        // Odası bilinmeyen cihaz bir odaya ait sayılmaz.
        assert!(scope_match(Some("Lab 1"), None, "LAB101", None).is_none());
    }

    #[test]
    fn oda_ve_on_ek_birlikte_verilirse_ikisi_de_aranir() {
        assert!(scope_match(Some("Lab 1"), Some("LAB1"), "LAB101", Some("Lab 1")).is_some());
        assert!(scope_match(Some("Lab 1"), Some("LAB1"), "LAB201", Some("Lab 1")).is_none());
        assert!(scope_match(Some("Lab 1"), Some("LAB1"), "LAB101", Some("Lab 2")).is_none());
    }

    #[test]
    fn bos_kapsam_herkesi_kapsar() {
        assert_eq!(scope_match(None, None, "HERHANGI", None), Some(0));
        assert_eq!(scope_match(Some("  "), Some(""), "HERHANGI", None), Some(0));
    }

    #[test]
    fn dar_kapsam_daha_yuksek_puan_alir() {
        // LAB1001 hem LAB1 hem LAB10 konsoluna uyar; LAB10 kazanmalı.
        let lab1 = scope_match(None, Some("LAB1"), "LAB1001", None).unwrap();
        let lab10 = scope_match(None, Some("LAB10"), "LAB1001", None).unwrap();
        let tumu = scope_match(None, None, "LAB1001", None).unwrap();
        assert!(lab10 > lab1 && lab1 > tumu);
        let oda = scope_match(Some("Lab 1"), None, "LAB1001", Some("Lab 1")).unwrap();
        assert!(oda > lab10);
    }

    #[test]
    fn kapsam_alani_olmayan_probe_cozulur() {
        // Eski konsollar `room`/`prefix` göndermez; kapsamları "tümü" sayılır.
        let eski = serde_json::json!({
            "t": "TeacherProbe",
            "protocol": crate::PROTOCOL_VERSION,
            "teacher_id": "00000000-0000-4000-8000-000000000001",
            "teacher_name": "Öğretmen",
            "control_port": 7643,
            "cert_fingerprint": vec![0u8; 32],
            "nonce": vec![0u8; 16],
        });
        let paket: DiscoveryPacket =
            serde_json::from_value(eski).expect("kapsam alanı olmayan probe çözülmeli");
        match paket {
            DiscoveryPacket::TeacherProbe { room, prefix, .. } => {
                assert_eq!((room, prefix), (None, None));
            }
            other => panic!("TeacherProbe beklenirken {other:?} geldi"),
        }
    }

    #[test]
    fn yabanci_trafik_sessizce_elenir() {
        assert!(DiscoveryPacket::decode(b"rastgele zirva").is_none());
    }

    #[test]
    fn oda_alani_olmayan_duyuru_cozulur() {
        // 0.1.0 ajanları `room` göndermez. Alan zorunlu olsaydı eski
        // kurulumlar keşifte hiç görünmezdi — sessiz ve teşhisi zor bir
        // bozulma olurdu.
        let eski = serde_json::json!({
            "t": "StudentAnnounce",
            "protocol": crate::PROTOCOL_VERSION,
            "device_id": "00000000-0000-4000-8000-000000000001",
            "hostname": "LAB101",
            "app_version": "0.1.0",
            "nonce": vec![0u8; 16],
            "paired_to": null,
        });
        let paket: DiscoveryPacket =
            serde_json::from_value(eski).expect("oda alanı olmayan duyuru çözülmeli");
        match paket {
            DiscoveryPacket::StudentAnnounce { room, hostname, .. } => {
                assert_eq!(room, None);
                assert_eq!(hostname, "LAB101");
            }
            other => panic!("StudentAnnounce beklenirken {other:?} geldi"),
        }
    }
}
