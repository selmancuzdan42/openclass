//! Öğretmen ekranı yayınının UDP multicast taşıması.
//!
//! # Neden multicast?
//!
//! TLS unicast ile 30 öğrenciye yayın yaparken aynı kare 30 kez gönderilir.
//! Tam hareketli ekranda bu 918 KB/s × 30 ≈ 27 MB/s eder ve 100 Mbit'lik
//! laboratuvar anahtarını doldurur. Multicast'te kare **bir kez** telden
//! geçer; anahtar kopyalamayı kendisi yapar.
//!
//! # Güvenlik
//!
//! UDP çoklu yayını ağdaki herkes dinleyebilir. Bu yüzden yük
//! **ChaCha20-Poly1305** ile şifrelenir. Oturum anahtarı yalnızca
//! kimliği doğrulanmış öğrencilere TLS kanalından iletilir; anahtarı
//! olmayan bir dinleyici için paketler anlamsızdır.
//!
//! Her yayının kendi anahtarı vardır ve yayın bittiğinde anahtar düşer.
//!
//! # Paket biçimi
//!
//! ```text
//! +---------+--------------------------------+
//! | 30 bayt | başlık (açık, AAD olarak doğrulanır)
//! +---------+--------------------------------+
//! |   ...   | şifreli parça + 16 baytlık etiket
//! +---------+--------------------------------+
//! ```
//!
//! Başlık açık gider: alıcı şifre çözmeden hangi kareye ve kaçıncı parçaya
//! baktığını bilmeli. AAD olarak doğrulandığı için **değiştirilemez** —
//! kurcalanan bir başlık şifre çözmeyi başarısız kılar.
//!
//! # Kayıp tespiti
//!
//! UDP teslim garantisi vermez. Her paket `(frame_seq, packet_index,
//! packet_count)` taşır; alıcı bir karenin tüm parçalarını toplayamazsa
//! kareyi atar ve TLS kanalından anahtar kare ister. Yeniden iletim
//! denenmez: ekran görüntüsünde eski bir parçayı kurtarmaktansa bir sonraki
//! kareye geçmek daha doğrudur.

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use uuid::Uuid;

/// Paketleri ayırt eden imza.
pub const MAGIC: [u8; 4] = *b"OCMC";

/// Biçim sürümü. Uyuşmayan sürümdeki paketler atılır.
pub const VERSION: u8 = 1;

/// Başlığın bayt uzunluğu.
pub const HEADER_LEN: usize = 30;

/// AEAD etiketinin uzunluğu.
pub const TAG_LEN: usize = 16;

/// Şifrelenmemiş parça yükünün üst sınırı.
///
/// 1500 baytlık Ethernet MTU'sundan IP (20) + UDP (8) + başlık (30) +
/// etiket (16) çıkarılır ve IP seçenekleri için pay bırakılır. Amaç IP
/// parçalanmasından kaçınmak: parçalanan bir datagramda tek parçanın
/// kaybı datagramın tamamını düşürür.
pub const MAX_FRAGMENT: usize = 1_380;

/// Bir datagramın alabileceği en büyük boyut.
pub const MAX_DATAGRAM: usize = HEADER_LEN + MAX_FRAGMENT + TAG_LEN;

/// Varsayılan multicast grup adresi.
///
/// 239.0.0.0/8 "yönetim kapsamlı" bloktur: yönlendiriciler bu adresleri
/// yerel ağın dışına taşımaz, yani yayın okul ağından çıkmaz.
pub const DEFAULT_GROUP: &str = "239.255.42.99";

/// Varsayılan multicast portu.
pub const DEFAULT_PORT: u16 = 7646;

/// Bir multicast paketinin başlığı.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PacketHeader {
    pub broadcast_id: Uuid,
    /// Karenin sıra numarası.
    pub frame_seq: u32,
    /// Bu parçanın karedeki sırası (0 tabanlı).
    pub packet_index: u16,
    /// Karenin toplam parça sayısı.
    pub packet_count: u16,
    /// Kare bağımsız (anahtar kare) mi?
    pub keyframe: bool,
}

impl PacketHeader {
    /// Başlığı bayta çevirir.
    pub fn encode(&self) -> [u8; HEADER_LEN] {
        let mut out = [0u8; HEADER_LEN];
        out[0..4].copy_from_slice(&MAGIC);
        out[4] = VERSION;
        out[5..21].copy_from_slice(self.broadcast_id.as_bytes());
        out[21..25].copy_from_slice(&self.frame_seq.to_be_bytes());
        out[25..27].copy_from_slice(&self.packet_index.to_be_bytes());
        out[27..29].copy_from_slice(&self.packet_count.to_be_bytes());
        out[29] = u8::from(self.keyframe);
        out
    }

    /// Başlığı baytlardan okur.
    ///
    /// İmza veya sürüm uymuyorsa `None` döner: ağdaki başka bir uygulamanın
    /// paketini çözmeye çalışmanın anlamı yok.
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < HEADER_LEN {
            return None;
        }
        if bytes[0..4] != MAGIC || bytes[4] != VERSION {
            return None;
        }

        let mut id = [0u8; 16];
        id.copy_from_slice(&bytes[5..21]);

        let packet_count = u16::from_be_bytes([bytes[27], bytes[28]]);
        let packet_index = u16::from_be_bytes([bytes[25], bytes[26]]);
        // Tutarsız başlık (sıfır parça, sınır dışı sıra) kabul edilmez.
        if packet_count == 0 || packet_index >= packet_count {
            return None;
        }

        Some(Self {
            broadcast_id: Uuid::from_bytes(id),
            frame_seq: u32::from_be_bytes([bytes[21], bytes[22], bytes[23], bytes[24]]),
            packet_index,
            packet_count,
            keyframe: bytes[29] != 0,
        })
    }

    /// Bu paketin nonce'u.
    ///
    /// `frame_seq` ve `packet_index` birlikte yayın içinde **benzersizdir**;
    /// her paket kendi nonce'unu taşır ve nonce tekrarı olmaz. Nonce'un
    /// tekrarı ChaCha20-Poly1305'te anahtarı tehlikeye atardı.
    fn nonce(&self) -> Nonce {
        let mut raw = [0u8; 12];
        raw[0..4].copy_from_slice(&self.frame_seq.to_be_bytes());
        raw[4..6].copy_from_slice(&self.packet_index.to_be_bytes());
        // Kalan baytlar yayın kimliğinin ilk parçasıyla doldurulur: iki
        // yayın aynı anahtarı kullanmasa da fazladan ayrışma zarar vermez.
        raw[6..12].copy_from_slice(&self.broadcast_id.as_bytes()[0..6]);
        Nonce::from(raw)
    }
}

/// Yayının simetrik oturum anahtarı.
///
/// TLS kanalından yalnızca kimliği doğrulanmış öğrencilere gönderilir.
#[derive(Clone)]
pub struct SessionKey(ChaCha20Poly1305);

impl SessionKey {
    /// 32 baytlık ham anahtardan kurar.
    pub fn new(raw: &[u8; 32]) -> Self {
        Self(ChaCha20Poly1305::new(&Key::from(*raw)))
    }

    /// Bir paketi şifreleyip tam datagramı üretir.
    pub fn seal(&self, header: &PacketHeader, fragment: &[u8]) -> Option<Vec<u8>> {
        let aad = header.encode();
        let sealed = self
            .0
            .encrypt(
                &header.nonce(),
                Payload {
                    msg: fragment,
                    aad: &aad,
                },
            )
            .ok()?;

        let mut out = Vec::with_capacity(HEADER_LEN + sealed.len());
        out.extend_from_slice(&aad);
        out.extend_from_slice(&sealed);
        Some(out)
    }

    /// Bir datagramı çözer.
    ///
    /// Başlık kurcalanmışsa veya anahtar yanlışsa `None` döner.
    pub fn open(&self, datagram: &[u8]) -> Option<(PacketHeader, Vec<u8>)> {
        let header = PacketHeader::decode(datagram)?;
        let aad = &datagram[0..HEADER_LEN];
        let body = &datagram[HEADER_LEN..];

        let plain = self
            .0
            .decrypt(&header.nonce(), Payload { msg: body, aad })
            .ok()?;
        Some((header, plain))
    }
}

impl std::fmt::Debug for SessionKey {
    /// Anahtar günlüğe veya hata çıktısına sızmamalı.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SessionKey(gizli)")
    }
}

/// Bir kare yükünü multicast datagramlarına böler.
///
/// Yük [`MAX_FRAGMENT`] boyunda parçalara ayrılır ve her parça ayrı
/// şifrelenir. Parça sayısı `u16`'ya sığmazsa `None` döner — böyle bir kare
/// zaten 90 MB'ın üzerinde olurdu.
pub fn split(
    key: &SessionKey,
    broadcast_id: Uuid,
    frame_seq: u32,
    keyframe: bool,
    payload: &[u8],
) -> Option<Vec<Vec<u8>>> {
    if payload.is_empty() {
        return None;
    }
    let count = payload.len().div_ceil(MAX_FRAGMENT);
    let packet_count = u16::try_from(count).ok()?;

    let mut out = Vec::with_capacity(count);
    for (i, fragment) in payload.chunks(MAX_FRAGMENT).enumerate() {
        let header = PacketHeader {
            broadcast_id,
            frame_seq,
            packet_index: i as u16,
            packet_count,
            keyframe,
        };
        out.push(key.seal(&header, fragment)?);
    }
    Some(out)
}

/// Bir karenin parçalarını toplayan tampon.
///
/// Eksik kalan kare **yeniden istenmez**: ekran görüntüsünde eski bir
/// parçayı kurtarmaktansa bir sonraki kareye geçmek daha doğrudur. Eksik
/// kare atılır ve çağıran TLS kanalından anahtar kare ister.
#[derive(Debug)]
pub struct FrameAssembler {
    broadcast_id: Uuid,
    frame_seq: u32,
    keyframe: bool,
    packet_count: u16,
    fragments: Vec<Option<Vec<u8>>>,
    have: u16,
}

impl FrameAssembler {
    /// İlk paketle yeni bir kare başlatır.
    pub fn start(header: &PacketHeader, fragment: Vec<u8>) -> Self {
        let mut fragments = vec![None; header.packet_count as usize];
        fragments[header.packet_index as usize] = Some(fragment);
        Self {
            broadcast_id: header.broadcast_id,
            frame_seq: header.frame_seq,
            keyframe: header.keyframe,
            packet_count: header.packet_count,
            fragments,
            have: 1,
        }
    }

    /// Toplanan karenin sıra numarası.
    pub fn frame_seq(&self) -> u32 {
        self.frame_seq
    }

    /// Kare bağımsız (anahtar kare) mi?
    pub fn is_keyframe(&self) -> bool {
        self.keyframe
    }

    /// Kaç parça eksik?
    pub fn missing(&self) -> u16 {
        self.packet_count - self.have
    }

    /// Bir parçayı ekler.
    ///
    /// Paket başka bir kareye veya yayına aitse yok sayılır ve `false`
    /// döner; çağıran yeni bir tampon başlatmalıdır.
    pub fn push(&mut self, header: &PacketHeader, fragment: Vec<u8>) -> bool {
        if header.broadcast_id != self.broadcast_id
            || header.frame_seq != self.frame_seq
            || header.packet_count != self.packet_count
        {
            return false;
        }
        let slot = &mut self.fragments[header.packet_index as usize];
        if slot.is_none() {
            *slot = Some(fragment);
            self.have += 1;
        }
        true
    }

    /// Tüm parçalar geldi mi?
    pub fn is_complete(&self) -> bool {
        self.have == self.packet_count
    }

    /// Kareyi birleştirir.
    ///
    /// Eksik parça varsa `None` döner.
    pub fn finish(self) -> Option<Vec<u8>> {
        if !self.is_complete() {
            return None;
        }
        let mut out = Vec::new();
        for fragment in self.fragments.into_iter() {
            out.extend_from_slice(&fragment?);
        }
        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key() -> SessionKey {
        SessionKey::new(&[7u8; 32])
    }

    #[test]
    fn baslik_gidip_geri_doner() {
        let h = PacketHeader {
            broadcast_id: Uuid::new_v4(),
            frame_seq: 123_456,
            packet_index: 3,
            packet_count: 9,
            keyframe: true,
        };
        assert_eq!(PacketHeader::decode(&h.encode()).unwrap(), h);
    }

    #[test]
    fn yabanci_paket_cozulmez() {
        // Başka bir uygulamanın UDP paketi.
        assert!(PacketHeader::decode(b"HTTP/1.1 200 OK................").is_none());
        // Kısa paket.
        assert!(PacketHeader::decode(b"OCMC").is_none());
    }

    #[test]
    fn tutarsiz_baslik_reddedilir() {
        let mut h = PacketHeader {
            broadcast_id: Uuid::nil(),
            frame_seq: 1,
            packet_index: 0,
            packet_count: 1,
            keyframe: false,
        }
        .encode();
        // packet_count = 0
        h[27] = 0;
        h[28] = 0;
        assert!(PacketHeader::decode(&h).is_none());

        // packet_index >= packet_count
        let mut h2 = h;
        h2[27] = 0;
        h2[28] = 2;
        h2[25] = 0;
        h2[26] = 5;
        assert!(PacketHeader::decode(&h2).is_none());
    }

    #[test]
    fn farkli_surum_atilir() {
        let mut raw = PacketHeader {
            broadcast_id: Uuid::nil(),
            frame_seq: 0,
            packet_index: 0,
            packet_count: 1,
            keyframe: false,
        }
        .encode();
        raw[4] = VERSION + 1;
        assert!(PacketHeader::decode(&raw).is_none());
    }

    #[test]
    fn sifreleme_gidip_geri_doner() {
        let k = key();
        let id = Uuid::new_v4();
        let payload = vec![42u8; 5_000];

        let packets = split(&k, id, 11, true, &payload).unwrap();
        // 5000 / 1380 = 4 paket.
        assert_eq!(packets.len(), 4);
        assert!(packets.iter().all(|p| p.len() <= MAX_DATAGRAM));

        let mut assembler: Option<FrameAssembler> = None;
        for packet in &packets {
            let (header, fragment) = k.open(packet).unwrap();
            match assembler.as_mut() {
                Some(a) => {
                    assert!(a.push(&header, fragment));
                }
                None => assembler = Some(FrameAssembler::start(&header, fragment)),
            }
        }

        let a = assembler.unwrap();
        assert!(a.is_complete());
        assert!(a.is_keyframe());
        assert_eq!(a.frame_seq(), 11);
        assert_eq!(a.finish().unwrap(), payload);
    }

    #[test]
    fn yanlis_anahtarla_cozulemez() {
        let id = Uuid::new_v4();
        let packets = split(&key(), id, 1, false, b"gizli veri").unwrap();
        let baska = SessionKey::new(&[9u8; 32]);
        assert!(baska.open(&packets[0]).is_none());
    }

    #[test]
    fn kurcalanan_baslik_cozulemez() {
        let id = Uuid::new_v4();
        let mut packet = split(&key(), id, 1, false, b"veri").unwrap().remove(0);
        // Kare numarasını değiştir: AAD doğrulaması bunu yakalar.
        packet[21] ^= 0xFF;
        assert!(key().open(&packet).is_none());
    }

    #[test]
    fn kurcalanan_yuk_cozulemez() {
        let id = Uuid::new_v4();
        let mut packet = split(&key(), id, 1, false, b"veri").unwrap().remove(0);
        let last = packet.len() - 1;
        packet[last] ^= 0xFF;
        assert!(key().open(&packet).is_none());
    }

    #[test]
    fn eksik_parcali_kare_birlestirilmez() {
        let k = key();
        let id = Uuid::new_v4();
        let payload = vec![1u8; 3_000];
        let packets = split(&k, id, 5, false, &payload).unwrap();
        assert_eq!(packets.len(), 3);

        // Ortadaki paket kaybolsun.
        let (h0, f0) = k.open(&packets[0]).unwrap();
        let (h2, f2) = k.open(&packets[2]).unwrap();
        let mut a = FrameAssembler::start(&h0, f0);
        a.push(&h2, f2);

        assert!(!a.is_complete());
        assert_eq!(a.missing(), 1);
        assert!(a.finish().is_none());
    }

    #[test]
    fn baska_kareye_ait_paket_eklenmez() {
        let k = key();
        let id = Uuid::new_v4();
        let a_packets = split(&k, id, 1, false, &vec![1u8; 2_000]).unwrap();
        let b_packets = split(&k, id, 2, false, &vec![2u8; 2_000]).unwrap();

        let (ha, fa) = k.open(&a_packets[0]).unwrap();
        let (hb, fb) = k.open(&b_packets[0]).unwrap();

        let mut assembler = FrameAssembler::start(&ha, fa);
        // 2 numaralı kareye ait paket reddedilir.
        assert!(!assembler.push(&hb, fb));
        assert_eq!(assembler.frame_seq(), 1);
    }

    #[test]
    fn tekrarlanan_parca_sayilmaz() {
        let k = key();
        let id = Uuid::new_v4();
        let packets = split(&k, id, 1, false, &vec![3u8; 2_000]).unwrap();

        let (h0, f0) = k.open(&packets[0]).unwrap();
        let (h0b, f0b) = k.open(&packets[0]).unwrap();
        let mut a = FrameAssembler::start(&h0, f0);
        a.push(&h0b, f0b);

        // Aynı paket iki kez gelse de eksik parça sayısı değişmez.
        assert_eq!(a.missing(), 1);
    }

    #[test]
    fn tek_parcalik_kare_calisir() {
        let k = key();
        let id = Uuid::new_v4();
        let packets = split(&k, id, 0, true, b"kucuk").unwrap();
        assert_eq!(packets.len(), 1);

        let (h, f) = k.open(&packets[0]).unwrap();
        let a = FrameAssembler::start(&h, f);
        assert!(a.is_complete());
        assert_eq!(a.finish().unwrap(), b"kucuk");
    }

    #[test]
    fn bos_yuk_paketlenmez() {
        assert!(split(&key(), Uuid::new_v4(), 0, false, b"").is_none());
    }

    #[test]
    fn anahtar_gunluge_sizmaz() {
        assert_eq!(format!("{:?}", key()), "SessionKey(gizli)");
    }
}
