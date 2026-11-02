//! Yerel ağ arayüzünün seçimi.

use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};

/// Belirtilen hedefe giderken kullanılacak yerel IPv4 adresi.
///
/// # Neden gerekli?
///
/// Multicast gönderiminde çıkış arayüzünü işletim sistemine bırakmak
/// güvenilmez. Bir laboratuvar makinesinde kablolu arayüzün yanında
/// `docker0`, sanal makine köprüleri ve kablosuz arayüz de bulunur;
/// yönlendirme tablosu yanlış olanı seçerse paketler hiç kimseye ulaşmaz
/// ve sorunu teşhis etmek zordur.
///
/// Bu yüzden arayüz, **öğretmene/ağa ulaşan adres** üzerinden açıkça
/// sabitlenir.
///
/// # Nasıl çalışır?
///
/// UDP soketi hedefe `connect` edilir. Bu paket göndermez — yalnızca
/// çekirdeğin yönlendirme tablosundan hangi yerel adresi kullanacağını
/// öğrenmesini sağlar. Sonra o adres okunur.
pub fn local_ipv4_toward(target: IpAddr) -> Option<Ipv4Addr> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).ok()?;
    // Port önemsiz; bağlantı kurulmaz, veri gitmez.
    socket.connect(SocketAddr::new(target, 9)).ok()?;
    match socket.local_addr().ok()?.ip() {
        IpAddr::V4(addr) if !addr.is_unspecified() => Some(addr),
        _ => None,
    }
}

/// Yerel ağa çıkan IPv4 adresi.
///
/// Hedef olarak genel bir adres kullanılır; amaç paket göndermek değil,
/// çekirdeğin "dışarıya çıkarken hangi arayüzü kullanırım" cevabını
/// okumak. Böylece `127.0.0.1` veya bir Docker köprüsü değil, gerçek LAN
/// adresi bulunur.
pub fn primary_ipv4() -> Option<Ipv4Addr> {
    local_ipv4_toward(IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_hedefi_loopback_adresi_verir() {
        let addr = local_ipv4_toward(IpAddr::V4(Ipv4Addr::LOCALHOST));
        // Çekirdek loopback'e giderken 127.0.0.1 kullanır.
        assert_eq!(addr, Some(Ipv4Addr::LOCALHOST));
    }

    #[test]
    fn birincil_adres_loopback_degildir() {
        // Ağ yoksa None dönebilir; varsa loopback olmamalı.
        if let Some(addr) = primary_ipv4() {
            assert!(!addr.is_loopback(), "LAN adresi loopback olmamalı: {addr}");
        }
    }
}
