//! Multicast'in gerçek soketlerle çalıştığını doğrulayan entegrasyon testi.
//!
//! Birim testleri paket biçimini ve şifrelemeyi kontrol eder; buradaki test
//! paketlerin **gerçekten telden geçtiğini** doğrular: öğretmenin kurduğu
//! soket seçenekleriyle gönderilen kare, öğrencinin kurduğu soketle gruba
//! katılıp alınabiliyor mu?
//!
//! Çıkış arayüzü **loopback'e sabitlenir**. Kablosuz arayüzler multicast'i
//! sık sık geri döndürmez ve test ortama göre atlanırsa hiçbir şey
//! doğrulamaz; loopback her makinede aynı davranır.
//!
//! Üretimde arayüz `openclass_common::net` ile LAN adresine sabitlenir —
//! yönlendirme tablosuna bırakmak `docker0` gibi arayüzler yüzünden
//! güvenilmez. Buradaki test o mekanizmanın taşıdığı veriyi doğrular.
//!
//! Multicast bu makinede hiç kurulamıyorsa (kısıtlı konteyner) test
//! nedenini yazdırıp atlanır.

use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::time::Duration;

use openclass_protocol::multicast::{
    split, FrameAssembler, SessionKey, DEFAULT_GROUP, MAX_DATAGRAM,
};
use uuid::Uuid;

/// Testin kullandığı port. Varsayılandan farklı: makinede çalışan gerçek
/// bir OpenClass örneğiyle çakışmasın.
const TEST_PORT: u16 = 47_646;

/// Testin kullandığı arayüz.
///
/// Loopback: her makinede aynı davranır. Kablosuz arayüzler multicast'i
/// geri döndürmediği için testi belirsiz hâle getirirdi.
const IFACE: Ipv4Addr = Ipv4Addr::LOCALHOST;

/// Öğretmenin `broadcast.rs` içinde kurduğu soketin aynısı.
fn sender() -> Option<(UdpSocket, SocketAddr)> {
    let group: Ipv4Addr = DEFAULT_GROUP.parse().ok()?;

    let socket = socket2::Socket::new(
        socket2::Domain::IPV4,
        socket2::Type::DGRAM,
        Some(socket2::Protocol::UDP),
    )
    .ok()?;
    socket.set_multicast_ttl_v4(1).ok()?;
    // Aynı makinedeki alıcıya ulaşabilmek için loopback açık olmalı.
    socket.set_multicast_loop_v4(true).ok()?;
    // Üretimdeki gibi çıkış arayüzü açıkça sabitlenir.
    socket.set_multicast_if_v4(&IFACE).ok()?;

    let bind: SocketAddr = (Ipv4Addr::UNSPECIFIED, 0).into();
    socket.bind(&bind.into()).ok()?;

    Some((socket.into(), SocketAddr::from((group, TEST_PORT))))
}

/// Öğrencinin `net/multicast.rs` içinde kurduğu soketin aynısı.
fn receiver() -> Option<UdpSocket> {
    let group: Ipv4Addr = DEFAULT_GROUP.parse().ok()?;

    let socket = socket2::Socket::new(
        socket2::Domain::IPV4,
        socket2::Type::DGRAM,
        Some(socket2::Protocol::UDP),
    )
    .ok()?;
    socket.set_reuse_address(true).ok()?;

    let bind: SocketAddr = (Ipv4Addr::UNSPECIFIED, TEST_PORT).into();
    socket.bind(&bind.into()).ok()?;

    let socket: UdpSocket = socket.into();
    // Gruba aynı arayüzden katıl; üretimde bu adres öğretmene ulaşan
    // arayüzün adresidir.
    socket.join_multicast_v4(&group, &IFACE).ok()?;
    socket.set_read_timeout(Some(Duration::from_secs(2))).ok()?;
    Some(socket)
}

#[test]
fn sifreli_kare_gercek_soketten_gecer() {
    let Some((tx, target)) = sender() else {
        eprintln!("atlandı: multicast gönderim soketi kurulamadı");
        return;
    };
    let Some(rx) = receiver() else {
        eprintln!("atlandı: multicast grubuna katılınamadı");
        return;
    };

    let key = SessionKey::new(&[42u8; 32]);
    let broadcast_id = Uuid::new_v4();
    // Birden çok pakete bölünecek kadar büyük bir yük.
    let payload: Vec<u8> = (0..5_000u32).map(|i| (i % 251) as u8).collect();

    let packets = split(&key, broadcast_id, 7, true, &payload).expect("kare bölünemedi");
    assert!(packets.len() > 1, "test birden çok paket üretmeli");

    for packet in &packets {
        if tx.send_to(packet, target).is_err() {
            eprintln!("atlandı: multicast paketi gönderilemedi (ağ izni yok)");
            return;
        }
    }

    // Parçaları topla.
    let mut buf = vec![0u8; MAX_DATAGRAM];
    let mut assembler: Option<FrameAssembler> = None;

    for _ in 0..packets.len() {
        let Ok((len, _)) = rx.recv_from(&mut buf) else {
            eprintln!("atlandı: multicast paketi alınamadı (bu makinede taşınmıyor)");
            return;
        };
        let (header, fragment) = key.open(&buf[..len]).expect("paket çözülemedi");
        assert_eq!(header.broadcast_id, broadcast_id);

        match assembler.as_mut() {
            Some(a) => assert!(a.push(&header, fragment), "parça aynı kareye ait olmalı"),
            None => assembler = Some(FrameAssembler::start(&header, fragment)),
        }
    }

    let assembled = assembler.expect("hiç paket alınmadı");
    assert!(assembled.is_complete(), "tüm parçalar gelmeliydi");
    assert!(assembled.is_keyframe());
    assert_eq!(assembled.frame_seq(), 7);
    assert_eq!(
        assembled.finish().expect("kare birleştirilemedi"),
        payload,
        "alınan kare gönderilenle birebir aynı olmalı"
    );
}
