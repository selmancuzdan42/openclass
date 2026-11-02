//! Öğretmen ekranının öğrencilere yayını.
//!
//! # Neden tek kodlama, çok gönderim?
//!
//! Kare bir kez yakalanır, bir kez ölçeklenir, bir kez kodlanır; sonuç tüm
//! bağlı öğrencilere gönderilir. Öğrenci başına kodlamak 20 cihazda CPU'yu
//! 20 katına çıkarırdı.
//!
//! Ağ tarafında şu an her öğrenciye ayrı gönderim yapılır (TLS kanalından).
//! Bu her ağda çalışır ama kablodaki trafik öğrenci sayısıyla büyür. UDP
//! multicast'e geçiş `docs/YOL-HARITASI.md` içinde bir sonraki adımdır;
//! kodlama tarafı o geçişe hazırdır çünkü üretilen paket alıcıdan bağımsızdır.
//!
//! # Öğrenciden gizlenmez
//!
//! Yayın başladığında öğrenci tarafında tam ekran bir pencere açılır ve
//! kapatılamaz. Bu bilinçlidir: öğretmen ekranı zorla gösterilen bir şeydir,
//! ama gizli değildir.

use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use openclass_protocol::control::{FrameCodec, ScreenRect, ServerMessage, StreamQuality};
use openclass_screen::{CaptureSession, TileEncoder};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::state::AppState;

/// Anahtar kare aralığı.
///
/// Öğrenci akışındakiyle aynı gerekçe: sonradan katılanın toparlanması ve
/// kayıp sonrası kendini düzeltme.
const KEYFRAME_INTERVAL: Duration = Duration::from_secs(3);

/// Çalışan yayının tutamacı.
pub struct BroadcastHandle {
    pub broadcast_id: Uuid,
    pub started_at: i64,
    /// Penceresiz (tam ekran) mod mu? Geç katılan öğrenciye de bildirilir.
    pub fullscreen: bool,
    /// Multicast açıksa taşıma bilgisi; geç katılan öğrenciye de verilir.
    pub multicast: Option<openclass_protocol::MulticastInfo>,
    cancel: CancellationToken,
}

impl BroadcastHandle {
    pub fn stop(&self) {
        self.cancel.cancel();
    }
}

/// Multicast soketini kurar.
///
/// Başarısız olursa `None` döner ve yayın yalnızca TLS unicast'ten yürür:
/// multicast'in çalışmaması yayını engellemek için sebep değil.
fn open_multicast() -> Option<(std::net::UdpSocket, std::net::SocketAddr)> {
    use openclass_protocol::multicast;

    let group: std::net::Ipv4Addr = multicast::DEFAULT_GROUP.parse().ok()?;

    let socket = socket2::Socket::new(
        socket2::Domain::IPV4,
        socket2::Type::DGRAM,
        Some(socket2::Protocol::UDP),
    )
    .ok()?;

    // TTL 1: yayın yerel ağdan çıkmaz. Okulun geri kalanına sızmasın.
    socket.set_multicast_ttl_v4(1).ok()?;
    // Loopback açık kalır. Öğretmenin kendi alıcısı olmadığı için bunun
    // bir maliyeti yok; kapatmak ise aynı makinede çalışan öğrenci
    // örneklerinin kare almasını engeller ve yerel testi imkânsız kılardı.
    socket.set_multicast_loop_v4(true).ok()?;

    // Çıkış arayüzünü **açıkça** sabitle. Yönlendirme tablosuna bırakmak
    // güvenilmez: laboratuvar makinesinde kablolu arayüzün yanında
    // `docker0`, sanal makine köprüleri ve kablosuz arayüz de bulunur.
    // Yanlış arayüz seçilirse paketler hiç kimseye ulaşmaz ve sorunu
    // teşhis etmek zordur.
    match openclass_common::net::primary_ipv4() {
        Some(local) => {
            socket.set_multicast_if_v4(&local).ok()?;
            tracing::info!(%local, "multicast çıkış arayüzü sabitlendi");
        }
        None => {
            tracing::warn!(
                "yerel LAN adresi bulunamadı, multicast arayüzü işletim \
                 sistemine bırakılıyor"
            );
        }
    }

    let bind: std::net::SocketAddr = (std::net::Ipv4Addr::UNSPECIFIED, 0).into();
    socket.bind(&bind.into()).ok()?;
    let socket: std::net::UdpSocket = socket.into();

    let target = std::net::SocketAddr::from((group, multicast::DEFAULT_PORT));
    Some((socket, target))
}

/// Yayını başlatır.
///
/// `fullscreen` penceresiz modu seçer: öğrencide pencere ekranı kaplar ve
/// üstte durur. `false` ise pencere normaldir ve öğrenci Alt+Tab ile başka
/// uygulamalara geçebilir.
pub fn start(state: Arc<AppState>, fullscreen: bool) -> Result<BroadcastHandle> {
    let teacher_name = state.config.read().teacher_name.clone();
    let session = CaptureSession::start_primary_monitor(&teacher_name)
        .context("öğretmen ekranı yakalanamadı")?;

    let broadcast_id = Uuid::new_v4();
    let cancel = CancellationToken::new();
    let backend = session.backend_name();

    // Her yayının kendi oturum anahtarı olur ve yayın bitince düşer.
    let multicast = open_multicast().map(|(socket, target)| {
        let mut key = [0u8; 32];
        // İşletim sisteminin rastgele kaynağı; yayın anahtarı tahmin
        // edilebilir olmamalı.
        if getrandom::fill(&mut key).is_err() {
            tracing::error!("multicast oturum anahtarı üretilemedi");
        }
        tracing::info!(%target, "multicast yayını açık");
        (
            socket,
            target,
            openclass_protocol::MulticastInfo {
                group: target.ip().to_string(),
                port: target.port(),
                key,
            },
        )
    });
    if multicast.is_none() {
        tracing::warn!("multicast soketi açılamadı, yayın yalnızca TLS üzerinden gidecek");
    }

    tracing::info!(%broadcast_id, backend, "öğretmen ekranı yayını başlıyor");

    // Bilgi kopyası handle'da kalır; soket yayın döngüsüne taşınır.
    let info = multicast.as_ref().map(|(_, _, info)| info.clone());

    tauri::async_runtime::spawn(run(
        Arc::clone(&state),
        session,
        broadcast_id,
        fullscreen,
        multicast,
        cancel.clone(),
    ));

    Ok(BroadcastHandle {
        broadcast_id,
        started_at: crate::db::now(),
        fullscreen,
        multicast: info,
        cancel,
    })
}

/// Yayın döngüsü.
async fn run(
    state: Arc<AppState>,
    session: CaptureSession,
    broadcast_id: Uuid,
    fullscreen: bool,
    multicast: Option<(
        std::net::UdpSocket,
        std::net::SocketAddr,
        openclass_protocol::MulticastInfo,
    )>,
    cancel: CancellationToken,
) {
    let quality = StreamQuality::BROADCAST;
    let teacher_name = state.config.read().teacher_name.clone();

    // Bağlı herkese yayının başladığını bildir.
    for (_, tx) in state.registry.all_senders() {
        let _ = tx
            .send(ServerMessage::TeacherScreenStart {
                broadcast_id,
                teacher_name: teacher_name.clone(),
                fullscreen,
                multicast: multicast.as_ref().map(|(_, _, info)| info.clone()),
            })
            .await;
    }

    let mut encoder = TileEncoder::new();
    let mut ticker =
        tokio::time::interval(Duration::from_millis(1000 / u64::from(quality.fps.max(1))));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    let mut seq: u64 = 0;
    let mut last_keyframe = Instant::now();
    let mut window_start = Instant::now();
    let mut window_bytes: usize = 0;
    let budget = (quality.max_kbps as usize * 1000) / 8;

    loop {
        tokio::select! {
            _ = cancel.cancelled() => break,
            _ = ticker.tick() => {}
        }

        if !session.is_running() {
            tracing::error!("öğretmen ekranı yakalaması durdu, yayın sonlandırılıyor");
            state.set_network_error("Öğretmen ekranı yakalaması durdu, yayın sonlandırıldı.");
            break;
        }

        let Some(raw) = session.take_latest() else {
            continue;
        };

        let now = Instant::now();
        if now.duration_since(window_start) >= Duration::from_secs(1) {
            window_start = now;
            window_bytes = 0;
        }
        if window_bytes >= budget {
            continue;
        }

        let force_keyframe = now.duration_since(last_keyframe) >= KEYFRAME_INTERVAL
            || state.take_broadcast_keyframe_request();

        let encoded = tokio::task::spawn_blocking(move || {
            let result = encoder.encode(
                &raw,
                quality.max_width,
                quality.max_height,
                quality.jpeg_quality,
                force_keyframe,
            );
            (encoder, result)
        })
        .await;

        let (returned, result) = match encoded {
            Ok(pair) => pair,
            Err(err) => {
                tracing::error!(%err, "yayın kodlama görevi başarısız");
                break;
            }
        };
        encoder = returned;

        let update = match result {
            Ok(Some(update)) => update,
            Ok(None) => continue,
            Err(err) => {
                tracing::error!(error = %format!("{err:#}"), "yayın karesi kodlanamadı");
                continue;
            }
        };

        if update.keyframe {
            last_keyframe = now;
        }
        window_bytes += update.bytes;
        seq += 1;

        if seq == 1 {
            tracing::info!(
                genislik = update.width,
                yukseklik = update.height,
                bolge = update.rects.len(),
                "yayının ilk karesi gönderiliyor"
            );
        }

        let rects: Vec<ScreenRect> = update
            .rects
            .into_iter()
            .map(|r| ScreenRect {
                x: r.x,
                y: r.y,
                width: r.width,
                height: r.height,
                data: r.jpeg,
            })
            .collect();

        let message = ServerMessage::TeacherScreenUpdate {
            seq,
            width: update.width,
            height: update.height,
            keyframe: update.keyframe,
            codec: FrameCodec::Jpeg,
            rects: rects.clone(),
        };

        // 1. Multicast: kare telden **bir kez** geçer, anahtar kopyalar.
        //    30 öğrenciye TLS unicast ile göndermek aynı kareyi 30 kez
        //    telden geçirir ve 100 Mbit'lik laboratuvar anahtarını doldurur.
        let mut multicast_ok = false;
        if let Some((socket, target, info)) = multicast.as_ref() {
            match openclass_protocol::encode(&message, openclass_protocol::MAX_CONTROL_FRAME) {
                Ok(payload) => {
                    let key = openclass_protocol::multicast::SessionKey::new(&info.key);
                    match openclass_protocol::multicast::split(
                        &key,
                        broadcast_id,
                        seq as u32,
                        update.keyframe,
                        &payload,
                    ) {
                        Some(packets) => {
                            multicast_ok = true;
                            for packet in &packets {
                                if let Err(err) = socket.send_to(packet, target) {
                                    // Tek paketin düşmesi kareyi bitirir;
                                    // alıcı eksik parçayı fark edip anahtar
                                    // kare ister.
                                    tracing::trace!(%err, seq, "multicast paketi gönderilemedi");
                                    multicast_ok = false;
                                    break;
                                }
                            }
                        }
                        None => tracing::warn!(seq, "kare multicast paketlerine bölünemedi"),
                    }
                }
                Err(err) => tracing::warn!(%err, "kare multicast için kodlanamadı"),
            }
        }

        // 2. TLS yedeği. Multicast kapalıysa herkese; açıksa yalnızca
        //    "paketler bana ulaşmıyor" diyen öğrencilere. `try_send`:
        //    kuyruğu dolu olan öğrenci bu kareyi kaçırır ve bir sonraki
        //    anahtar karede toparlanır.
        let fallback = state.broadcast_fallback.read().clone();
        for (device_id, tx) in state.registry.all_senders() {
            if multicast_ok && !fallback.contains(&device_id) {
                continue;
            }
            if tx.try_send(message.clone()).is_err() {
                tracing::trace!(%device_id, seq, "yayın karesi düşürüldü (yavaş istemci)");
            }
        }
    }

    session.stop();

    for (_, tx) in state.registry.all_senders() {
        let _ = tx.send(ServerMessage::TeacherScreenStop).await;
    }

    // Oturum anahtarı ve yedek listesi yayınla birlikte düşer.
    state.broadcast_fallback.write().clear();
    *state.broadcast.write() = None;
    state.emit("broadcast-changed", Option::<()>::None);
    tracing::info!(%broadcast_id, kare = seq, "öğretmen ekranı yayını bitti");
}
