//! Yerel ağ keşfi — öğretmen tarafı.
//!
//! # Neden öğretmen yayın yapıyor, öğrenci değil?
//!
//! İki yön de mümkündü. Öğretmenin yayın yapması seçildi çünkü:
//!
//! * Laboratuvarda 20 öğrenci sürekli yayın yaparsa ağda gereksiz sabit
//!   trafik olur; öğretmen yalnızca arama yaptığında trafik doğar.
//! * Öğrenci ajanı pasif dinlemede kaldığı için, öğretmen konsolu kapalıyken
//!   hiç paket üretmez.
//! * Cevaplar **unicast** döndüğü için öğretmen, cevabın geldiği gerçek IP'yi
//!   doğrudan öğrenir; DHCP ile IP değişse de sorun olmaz.
//!
//! # Yayının engellendiği ağlar
//!
//! Bazı okul anahtarları broadcast'i keser. Bu durumda öğretmen, bilinen
//! adreslere **doğrudan unicast probe** gönderebilir ([`probe_targets`]);
//! ayrıca öğrenci ajanı yapılandırılmış öğretmen adresine kendisi bağlanabilir.
//! Keşif, bağlantının önkoşulu değildir.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use openclass_protocol::{
    discovery::DiscoveryPacket, DEFAULT_DISCOVERY_PORT, DEFAULT_TEACHER_DISCOVERY_PORT,
    MAX_DISCOVERY_PACKET, PROTOCOL_VERSION,
};
use tokio::net::UdpSocket;
use tokio_util::sync::CancellationToken;

use crate::db::now;
use crate::state::{AppState, DiscoveredDevice};

/// İki probe turu arasındaki süre.
const PROBE_INTERVAL: Duration = Duration::from_secs(3);

/// Bu süre boyunca cevap vermeyen cihaz keşif listesinden düşer.
const DISCOVERY_TTL_SECS: i64 = 30;

/// Keşif servisini çalıştırır.
///
/// Tek bir UDP soketi hem probe gönderir hem unicast cevapları dinler.
/// Soket **sabit** bir porta ([`DEFAULT_TEACHER_DISCOVERY_PORT`]) bağlanır;
/// öğrencilerin dinlediği 7644'e değil, çünkü ikisi aynı makinede
/// çalışabilmeli.
///
/// Portun sabit olması güvenlik duvarı kuralı yazılabilmesi içindir:
/// öğrencilerin cevabı bu porta döner ve gelen UDP trafiğine izin verilmesi
/// gerekir. Port meşgulse gelişigüzel porta düşülür ve bu loglanır.
pub async fn run(state: Arc<AppState>, cancel: CancellationToken) -> Result<()> {
    let socket = match UdpSocket::bind(SocketAddr::from((
        Ipv4Addr::UNSPECIFIED,
        DEFAULT_TEACHER_DISCOVERY_PORT,
    )))
    .await
    {
        Ok(socket) => socket,
        Err(err) => {
            tracing::warn!(
                port = DEFAULT_TEACHER_DISCOVERY_PORT,
                %err,
                "sabit keşif portu alınamadı, gelişigüzel porta düşülüyor — \
                 güvenlik duvarı açıksa keşif çalışmayabilir"
            );
            UdpSocket::bind(SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0)))
                .await
                .context("keşif soketi açılamadı")?
        }
    };
    socket
        .set_broadcast(true)
        .context("UDP yayın izni alınamadı — güvenlik duvarı engelliyor olabilir")?;

    let local = socket.local_addr()?;
    tracing::info!(port = local.port(), "keşif servisi başladı");

    let socket = Arc::new(socket);
    let mut ticker = tokio::time::interval(PROBE_INTERVAL);
    let mut buf = vec![0u8; MAX_DISCOVERY_PACKET];

    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                tracing::info!("keşif servisi durduruldu");
                return Ok(());
            }

            _ = ticker.tick() => {
                state.registry.prune_discovered(DISCOVERY_TTL_SECS);
                if let Err(err) = send_probe(&socket, &state).await {
                    tracing::warn!(%err, "keşif probe'u gönderilemedi");
                }
            }

            result = socket.recv_from(&mut buf) => {
                match result {
                    Ok((len, from)) => handle_datagram(&state, &buf[..len], from),
                    Err(err) => {
                        // Tek bir hatalı datagram servisi düşürmez.
                        tracing::warn!(%err, "keşif datagramı alınamadı");
                    }
                }
            }
        }
    }
}

/// Probe paketini yayın adresine ve yapılandırılmış hedeflere gönderir.
async fn send_probe(socket: &UdpSocket, state: &AppState) -> Result<()> {
    let packet = DiscoveryPacket::TeacherProbe {
        protocol: PROTOCOL_VERSION,
        teacher_id: state.teacher_id,
        teacher_name: state.config.read().teacher_name.clone(),
        control_port: state.config.read().control_port,
        cert_fingerprint: state.tls.fingerprint.to_vec(),
        nonce: openclass_protocol::random_bytes::<16>(),
    };
    let bytes = packet.encode()?;

    let mut sent_any = false;
    let mut last_error = None;

    for target in probe_targets(state) {
        match socket.send_to(&bytes, target).await {
            Ok(_) => sent_any = true,
            Err(err) => {
                tracing::debug!(%target, %err, "probe hedefe ulaşmadı");
                last_error = Some(err);
            }
        }
    }

    if !sent_any {
        if let Some(err) = last_error {
            return Err(err).context("hiçbir keşif hedefine paket gönderilemedi");
        }
    }
    Ok(())
}

/// Probe gönderilecek adresler.
///
/// Sınırlı yayın adresi (`255.255.255.255`) her zaman denenir. Ayrıca
/// yapılandırmada elle eklenmiş adresler varsa onlara unicast gönderilir —
/// bu, yayının engellendiği ağlar için kaçış yoludur.
fn probe_targets(state: &AppState) -> Vec<SocketAddr> {
    let port = DEFAULT_DISCOVERY_PORT;
    let mut targets = vec![SocketAddr::from((Ipv4Addr::BROADCAST, port))];

    for entry in &state.config.read().extra_discovery_hosts {
        // Girdi `ip`, `ad`, `ip:port` veya `192.168.1.10-40` biçiminde
        // olabilir. Port belirtmek, aynı makinede birden fazla öğrenci ajanı
        // çalıştırırken gerekir.
        let (host, target_port) = split_host_port(entry, port);

        // Aralık mı? `192.168.1.10-40` → .10'dan .40'a kadar.
        if let Some(aralik) = expand_ipv4_range(&host) {
            targets.extend(aralik.into_iter().map(|ip| SocketAddr::new(ip.into(), target_port)));
            continue;
        }

        match host.parse::<IpAddr>() {
            Ok(ip) => targets.push(SocketAddr::new(ip, target_port)),
            Err(_) => {
                // Alan adı verilmişse çözümlemeyi işletim sistemine bırakırız.
                match std::net::ToSocketAddrs::to_socket_addrs(&(host.as_str(), target_port)) {
                    Ok(addrs) => targets.extend(addrs),
                    Err(err) => tracing::warn!(%host, %err, "keşif hedefi çözümlenemedi"),
                }
            }
        }
    }
    targets
}

/// Taranacak en fazla adres.
///
/// Yayının engellendiği ağlarda tek tek unicast probe gönderilir. Bir /24
/// alt ağ 254 adrestir; bundan büyük bir aralık yanlışlıkla girilmiş
/// demektir ve ağı gereksiz paketle doldurur.
const MAX_RANGE_HOSTS: u32 = 254;

/// `192.168.1.10-40` biçimindeki aralığı adreslere açar.
///
/// Yalnızca son sekizlide aralık desteklenir: okul ağlarında ihtiyaç bu, ve
/// `10.0.0.1-10.0.3.255` gibi geniş aralıklar yüz binlerce probe demek olur.
/// Aralık değilse `None` döner ve girdi olağan yolla çözülür.
fn expand_ipv4_range(host: &str) -> Option<Vec<Ipv4Addr>> {
    let (bas, son) = host.split_once('-')?;

    let ilk: Ipv4Addr = bas.trim().parse().ok()?;
    let son = son.trim();

    // Son taraf tam adres de olabilir (`192.168.1.40`), yalnız sekizli de
    // (`40`). İkincisi yaygın yazım.
    let son_sekizli: u8 = match son.parse::<Ipv4Addr>() {
        Ok(tam) => {
            let a = ilk.octets();
            let b = tam.octets();
            // İlk üç sekizli aynı olmalı; değilse aralık çok geniştir.
            if a[0] != b[0] || a[1] != b[1] || a[2] != b[2] {
                tracing::warn!(%host, "yalnızca son sekizlide aralık desteklenir");
                return None;
            }
            b[3]
        }
        Err(_) => son.parse().ok()?,
    };

    let o = ilk.octets();
    if son_sekizli < o[3] {
        tracing::warn!(%host, "aralığın sonu başından küçük");
        return None;
    }

    let adet = u32::from(son_sekizli - o[3]) + 1;
    if adet > MAX_RANGE_HOSTS {
        tracing::warn!(%host, adet, "aralık çok geniş, yok sayıldı");
        return None;
    }

    Some(
        (o[3]..=son_sekizli)
            .map(|d| Ipv4Addr::new(o[0], o[1], o[2], d))
            .collect(),
    )
}

/// `ip`, `ad` veya `ip:port` biçimindeki girdiyi ayırır.
///
/// IPv6 adresleri köşeli parantezle yazılır: `[fe80::1]:7644`.
fn split_host_port(entry: &str, default_port: u16) -> (String, u16) {
    let text = entry.trim();

    if let Some(rest) = text.strip_prefix('[') {
        if let Some((host, tail)) = rest.split_once(']') {
            let port = tail
                .strip_prefix(':')
                .and_then(|p| p.parse().ok())
                .unwrap_or(default_port);
            return (host.to_string(), port);
        }
    }

    match text.rsplit_once(':') {
        // Sağ taraf geçerli bir port ise ayır; değilse çıplak IPv6 kabul et.
        Some((host, port)) if !host.is_empty() => match port.parse::<u16>() {
            Ok(p) => (host.to_string(), p),
            Err(_) => (text.to_string(), default_port),
        },
        _ => (text.to_string(), default_port),
    }
}

/// Gelen datagramı işler.
fn handle_datagram(state: &AppState, data: &[u8], from: SocketAddr) {
    let Some(parsed) = DiscoveryPacket::decode(data) else {
        // Bizim imzamızı taşımayan trafik; sessizce yok sayılır.
        return;
    };

    let packet = match parsed {
        Ok(p) => p,
        Err(err) => {
            tracing::debug!(%from, %err, "bozuk keşif paketi");
            return;
        }
    };

    if packet.protocol() != PROTOCOL_VERSION {
        tracing::debug!(
            %from,
            karsi_taraf = packet.protocol(),
            biz = PROTOCOL_VERSION,
            "keşifte protokol sürümü uyuşmuyor"
        );
        return;
    }

    match packet {
        DiscoveryPacket::StudentAnnounce {
            device_id,
            hostname,
            app_version,
            paired_to,
            room,
            ..
        } => {
            let hostname = hostname.trim().to_ascii_uppercase();
            if hostname.is_empty() {
                tracing::debug!(%from, "boş bilgisayar adı bildiren cihaz yok sayıldı");
                return;
            }
            tracing::debug!(%from, %hostname, %device_id, "öğrenci keşfedildi");
            // Oda adı öğrencinin beyanıdır; yetki kararında kullanılmaz,
            // yalnızca listelemeye yarar. Boş değer bilinmiyor sayılır.
            let room = room
                .map(|r| r.trim().chars().take(64).collect::<String>())
                .filter(|r| !r.is_empty());

            state.registry.record_discovery(DiscoveredDevice {
                device_id,
                hostname,
                address: from.to_string(),
                app_version,
                last_seen: now(),
                paired_to_us: paired_to == Some(state.teacher_id),
                room,
            });
        }

        // Başka bir öğretmen konsolunun probe'unu görmek normaldir
        // (aynı ağda iki öğretmen olabilir); cevap vermeyiz.
        DiscoveryPacket::TeacherProbe { teacher_id, .. } => {
            if teacher_id != state.teacher_id {
                tracing::debug!(%from, %teacher_id, "ağda başka bir öğretmen konsolu var");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{expand_ipv4_range, split_host_port};
    use std::net::Ipv4Addr;

    #[test]
    fn portsuz_hedef_varsayilan_portu_alir() {
        assert_eq!(
            split_host_port("192.168.1.255", 7644),
            ("192.168.1.255".into(), 7644)
        );
    }

    #[test]
    fn portlu_hedef_ayristirilir() {
        assert_eq!(
            split_host_port("127.0.0.1:7645", 7644),
            ("127.0.0.1".into(), 7645)
        );
    }

    #[test]
    fn alan_adi_desteklenir() {
        assert_eq!(
            split_host_port("ogretmen-pc", 7644),
            ("ogretmen-pc".into(), 7644)
        );
    }

    #[test]
    fn ipv6_koseli_parantezle_yazilir() {
        assert_eq!(
            split_host_port("[fe80::1]:7646", 7644),
            ("fe80::1".into(), 7646)
        );
        assert_eq!(split_host_port("[fe80::1]", 7644), ("fe80::1".into(), 7644));
    }

    #[test]
    fn ip_araligi_acilir() {
        let a = expand_ipv4_range("192.168.1.10-13").unwrap();
        assert_eq!(
            a,
            vec![
                Ipv4Addr::new(192, 168, 1, 10),
                Ipv4Addr::new(192, 168, 1, 11),
                Ipv4Addr::new(192, 168, 1, 12),
                Ipv4Addr::new(192, 168, 1, 13),
            ]
        );

        // Son taraf tam adres olarak da yazılabilir.
        let b = expand_ipv4_range("192.168.1.10-192.168.1.11").unwrap();
        assert_eq!(b.len(), 2);

        // Tek adreslik aralık geçerlidir.
        assert_eq!(expand_ipv4_range("10.0.0.5-5").unwrap().len(), 1);
    }

    #[test]
    fn bozuk_veya_asiri_aralik_reddedilir() {
        // Aralık değil: olağan yolla çözülsün diye None döner.
        assert!(expand_ipv4_range("192.168.1.10").is_none());
        assert!(expand_ipv4_range("ogretmen.okul.local").is_none());

        // Sonu başından küçük.
        assert!(expand_ipv4_range("192.168.1.40-10").is_none());

        // Farklı alt ağ: yalnızca son sekizlide aralık desteklenir.
        assert!(expand_ipv4_range("192.168.1.1-192.168.3.1").is_none());

        // 254'ten geniş olamaz.
        assert!(expand_ipv4_range("10.0.0.0-255").is_none());
        assert!(expand_ipv4_range("10.0.0.1-254").is_some());

        // Bozuk yazımlar.
        assert!(expand_ipv4_range("192.168.1.x-10").is_none());
        assert!(expand_ipv4_range("-").is_none());
    }
}
