//! Öğretmen ekranı yayınının multicast alıcısı.
//!
//! # Akış
//!
//! Yayın başladığında [`spawn`] gruba katılan bir görev başlatır. Gelen
//! datagramlar oturum anahtarıyla çözülür, bir karenin parçaları
//! toplanır ve kare tamamlanınca TLS'ten gelen kareyle **aynı yoldan**
//! uygulanır.
//!
//! # Yedeğe geçiş
//!
//! [`SILENCE_TIMEOUT`] boyunca hiç paket gelmezse öğretmene
//! [`ClientMessage::MulticastUnavailable`] gönderilir ve öğretmen bu
//! cihaza kareleri TLS üzerinden tek tek yollamaya başlar. Dinleme
//! durdurulmaz: anahtar yapılandırması düzelirse multicast kendiliğinden
//! geri döner.
//!
//! Bildirim **bir kez** gönderilir; her zaman aşımında tekrar göndermek
//! kontrol kanalını gereksiz meşgul eder.
//!
//! # Kayıp parça
//!
//! Eksik kalan kare atılır ve TLS kanalından anahtar kare istenir.
//! Yeniden iletim denenmez: ekranda eski bir parçayı kurtarmaktansa bir
//! sonraki kareye geçmek doğrudur.

use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use openclass_protocol::multicast::{FrameAssembler, SessionKey, MAX_DATAGRAM};
use openclass_protocol::{ClientMessage, MulticastInfo, ServerMessage};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::state::AppState;

/// Bu süre boyunca paket gelmezse TLS yedeği istenir.
///
/// Birkaç kare kaybı normaldir; 4 saniye sessizlik multicast'in hiç
/// çalışmadığını gösterir (anahtar IGMP'yi süzüyor, sanal ağ arayüzü
/// yanlış seçilmiş…).
const SILENCE_TIMEOUT: Duration = Duration::from_secs(4);

/// Paket beklerken soketin bloke kalacağı süre.
const READ_TIMEOUT: Duration = Duration::from_millis(500);

/// Multicast alıcısını başlatır.
///
/// Soket kurulamazsa hata döner; çağıran TLS yedeğini ister.
pub fn spawn(
    state: Arc<AppState>,
    info: &MulticastInfo,
    broadcast_id: Uuid,
    teacher: std::net::IpAddr,
    tx: mpsc::Sender<ClientMessage>,
    cancel: CancellationToken,
) -> Result<()> {
    let socket = join(info, teacher)?;
    let key = SessionKey::new(&info.key);

    tracing::info!(
        grup = %info.group,
        port = info.port,
        "multicast grubuna katıldı"
    );

    // Okuma bloke eder; ayrı bir iş parçacığında yürür.
    std::thread::Builder::new()
        .name("openclass-multicast".into())
        .spawn(move || receive_loop(state, socket, key, broadcast_id, tx, cancel))
        .context("multicast alıcı iş parçacığı başlatılamadı")?;
    Ok(())
}

/// Multicast grubuna katılır.
///
/// Gruba **öğretmene ulaşan arayüz** üzerinden katılır. `INADDR_ANY` ile
/// katılmak arayüz seçimini yönlendirme tablosuna bırakır; makinede
/// `docker0` veya sanal köprü varsa yanlış arayüz seçilip paketler hiç
/// gelmez. Adres bulunamazsa `INADDR_ANY`'ye düşülür.
fn join(info: &MulticastInfo, teacher: std::net::IpAddr) -> Result<UdpSocket> {
    let group: Ipv4Addr = info
        .group
        .parse()
        .with_context(|| format!("geçersiz multicast adresi: {}", info.group))?;

    // Aynı makinede birden çok öğrenci örneği çalışabilir (yerel test):
    // adres yeniden kullanımı açılmazsa ikincisi bağlanamaz.
    let socket = socket2::Socket::new(
        socket2::Domain::IPV4,
        socket2::Type::DGRAM,
        Some(socket2::Protocol::UDP),
    )
    .context("UDP soketi oluşturulamadı")?;
    socket
        .set_reuse_address(true)
        .context("soket yeniden kullanıma açılamadı")?;

    let bind: SocketAddr = (Ipv4Addr::UNSPECIFIED, info.port).into();
    socket
        .bind(&bind.into())
        .with_context(|| format!("{bind} adresine bağlanılamadı"))?;

    let socket: UdpSocket = socket.into();
    let iface = openclass_common::net::local_ipv4_toward(teacher).unwrap_or(Ipv4Addr::UNSPECIFIED);
    socket
        .join_multicast_v4(&group, &iface)
        .with_context(|| format!("{group} grubuna {iface} arayüzünden katılınamadı"))?;
    tracing::debug!(%iface, "multicast arayüzü seçildi");
    socket
        .set_read_timeout(Some(READ_TIMEOUT))
        .context("okuma zaman aşımı ayarlanamadı")?;
    Ok(socket)
}

fn receive_loop(
    state: Arc<AppState>,
    socket: UdpSocket,
    key: SessionKey,
    broadcast_id: Uuid,
    tx: mpsc::Sender<ClientMessage>,
    cancel: CancellationToken,
) {
    let mut buf = vec![0u8; MAX_DATAGRAM];
    let mut assembler: Option<FrameAssembler> = None;
    let mut last_packet = std::time::Instant::now();
    let mut reported = false;
    let mut applied: u64 = 0;

    while !cancel.is_cancelled() {
        let received = match socket.recv_from(&mut buf) {
            Ok((len, _)) => Some(len),
            Err(err)
                if matches!(
                    err.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                None
            }
            Err(err) => {
                tracing::error!(%err, "multicast okuma hatası");
                break;
            }
        };

        // Sessizlik: öğretmene bir kez haber ver, dinlemeyi sürdür.
        if received.is_none() {
            if !reported && last_packet.elapsed() >= SILENCE_TIMEOUT {
                reported = true;
                tracing::warn!("multicast paketi gelmiyor, TLS yedeği isteniyor");
                let _ = tx.blocking_send(ClientMessage::MulticastUnavailable { broadcast_id });
            }
            continue;
        }

        let len = received.unwrap_or(0);
        let Some((header, fragment)) = key.open(&buf[..len]) else {
            // Yabancı paket, yanlış anahtar veya kurcalanmış başlık.
            continue;
        };
        if header.broadcast_id != broadcast_id {
            continue;
        }

        last_packet = std::time::Instant::now();

        // Yeni kare başladıysa öncekini bırak.
        let same_frame = assembler
            .as_ref()
            .is_some_and(|a| a.frame_seq() == header.frame_seq);

        if !same_frame {
            if let Some(previous) = assembler.take() {
                if !previous.is_complete() {
                    tracing::debug!(
                        kare = previous.frame_seq(),
                        eksik = previous.missing(),
                        "eksik multicast karesi atıldı"
                    );
                    // Eksik kare üzerine çizmek görüntüyü bozar; tam kare
                    // istemek tek doğru kurtarma yolu.
                    let _ = tx.blocking_send(ClientMessage::RequestTeacherKeyframe);
                }
            }
            assembler = Some(FrameAssembler::start(&header, fragment));
        } else if let Some(a) = assembler.as_mut() {
            a.push(&header, fragment);
        }

        let complete = assembler.as_ref().is_some_and(FrameAssembler::is_complete);
        if !complete {
            continue;
        }

        let Some(payload) = assembler.take().and_then(FrameAssembler::finish) else {
            continue;
        };

        match openclass_protocol::decode::<ServerMessage>(
            &payload,
            openclass_protocol::MAX_CONTROL_FRAME,
        ) {
            Ok(ServerMessage::TeacherScreenUpdate {
                seq,
                width,
                height,
                keyframe,
                rects,
                ..
            }) => {
                let bytes: usize = rects.iter().map(|r| r.data.len()).sum();
                let needs_keyframe = {
                    let mut buffer = state.teacher_screen.write();
                    buffer.push(
                        openclass_screen::StoredUpdate {
                            seq,
                            keyframe,
                            rects,
                            bytes,
                        },
                        width,
                        height,
                    );
                    buffer.needs_keyframe()
                };

                applied += 1;
                if applied == 1 {
                    tracing::info!(
                        genislik = width,
                        yukseklik = height,
                        "ilk multicast karesi uygulandı"
                    );
                }
                // TLS yolunun kullandığı olayın aynısı: yayın penceresi
                // karenin nereden geldiğini bilmek zorunda değil.
                state.emit("broadcast-frame", seq);

                if needs_keyframe {
                    let _ = tx.blocking_send(ClientMessage::RequestTeacherKeyframe);
                }
            }
            Ok(other) => {
                tracing::debug!(?other, "multicast'ten beklenmeyen mesaj geldi, yok sayıldı");
            }
            Err(err) => {
                tracing::warn!(%err, "multicast karesi çözülemedi");
            }
        }
    }

    tracing::info!(kare = applied, "multicast alıcısı kapandı");
}
