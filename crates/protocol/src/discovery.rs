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
    fn yabanci_trafik_sessizce_elenir() {
        assert!(DiscoveryPacket::decode(b"rastgele zirva").is_none());
    }
}
