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
use tokio::sync::mpsc;
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
    /// Yayının kaynağı bir öğrenci ekranıysa o öğrenci; `None` ise öğretmen
    /// kendi ekranını yayınlıyor.
    pub source: Option<RelaySource>,
    cancel: CancellationToken,
}

/// Ekranı sınıfa gösterilen öğrenci.
pub struct RelaySource {
    pub device_id: Uuid,
    pub hostname: String,
    /// Öğrenciden gelen ekran güncellemeleri buraya düşer (bkz.
    /// `net::server`); yansıtma döngüsü onları diğer öğrencilere aktarır.
    pub frames: mpsc::Sender<RelayFrame>,
}

/// Yansıtılan öğrenciden gelen tek bir ekran güncellemesi.
pub struct RelayFrame {
    pub width: u32,
    pub height: u32,
    pub keyframe: bool,
    pub rects: Vec<ScreenRect>,
}

/// Yansıtma kuyruğunun boyu. Dolarsa kare düşer ve kaynaktan tam kare
/// istenir; izleyenlerde bozuk bölge kalmaz.
const RELAY_QUEUE: usize = 32;

type Multicast = (
    std::net::UdpSocket,
    std::net::SocketAddr,
    openclass_protocol::MulticastInfo,
);

/// Multicast soketini ve bu yayına özel oturum anahtarını hazırlar.
fn prepare_multicast() -> Option<Multicast> {
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
    multicast
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

    let multicast = prepare_multicast();

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
        source: None,
        cancel,
    })
}

/// Bir öğrencinin ekranını sınıfın geri kalanına gösterir.
///
/// Öğrenciden zaten gelen ekran güncellemeleri **yeniden kodlanmadan**
/// öğretmen yayınının taşıma yoluna (multicast + TLS yedeği) aktarılır.
/// İzleyen öğrenciler için bu sıradan bir öğretmen yayınıdır; eski sürüm
/// ajanlar da görür. Kaynak öğrenciye yayın penceresi açılmaz, bunun yerine
/// ekranının gösterildiği bildirilir.
pub async fn start_relay(
    state: Arc<AppState>,
    device_id: Uuid,
    fullscreen: bool,
) -> Result<BroadcastHandle> {
    let source_tx = state
        .registry
        .sender(device_id)
        .context("cihaz bağlı değil")?;
    let hostname = state
        .registry
        .hostname(device_id)
        .context("cihaz bağlı değil")?;

    // Kaynağın akışını yayın kalitesine yükselt; bitince eskisine döner.
    let flags = state.registry.flags(device_id);
    let previous_quality = flags.quality.unwrap_or(StreamQuality::PREVIEW);
    let quality = StreamQuality::BROADCAST;
    let upgrade = if flags.streaming {
        ServerMessage::UpdateStreamQuality {
            stream_id: 1,
            quality,
        }
    } else {
        ServerMessage::StartScreenStream {
            stream_id: 1,
            quality,
        }
    };
    source_tx
        .send(upgrade)
        .await
        .ok()
        .context("cihazla bağlantı kapandı")?;
    state.registry.update_flags(device_id, |f| {
        f.streaming = true;
        f.quality = Some(quality);
    });

    let broadcast_id = Uuid::new_v4();
    let cancel = CancellationToken::new();
    let multicast = prepare_multicast();
    let info = multicast.as_ref().map(|(_, _, info)| info.clone());
    let (frames, rx) = mpsc::channel(RELAY_QUEUE);

    tracing::info!(%broadcast_id, %device_id, %hostname, "öğrenci ekranı sınıfa gösteriliyor");

    tauri::async_runtime::spawn(run_relay(
        Arc::clone(&state),
        RelayTask {
            device_id,
            hostname: hostname.clone(),
            source_tx,
            previous_quality,
            rx,
        },
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
        source: Some(RelaySource {
            device_id,
            hostname,
            frames,
        }),
        cancel,
    })
}

/// Yansıtma döngüsünün kaynağa ait parçaları.
struct RelayTask {
    device_id: Uuid,
    hostname: String,
    source_tx: mpsc::Sender<ServerMessage>,
    previous_quality: StreamQuality,
    rx: mpsc::Receiver<RelayFrame>,
}

/// Yansıtma döngüsü: kaynaktan gelen güncellemeleri diğer öğrencilere aktarır.
async fn run_relay(
    state: Arc<AppState>,
    mut task: RelayTask,
    broadcast_id: Uuid,
    fullscreen: bool,
    multicast: Option<Multicast>,
    cancel: CancellationToken,
) {
    let source = task.device_id;

    // Kaynak dışındaki herkese yayının başladığını bildir. Başlıkta kimin
    // ekranı olduğu görünsün diye ad alanına öğrencinin bilgisayar adı konur.
    for (id, tx) in state.registry.all_senders() {
        if id == source {
            continue;
        }
        let _ = tx
            .send(ServerMessage::TeacherScreenStart {
                broadcast_id,
                teacher_name: task.hostname.clone(),
                fullscreen,
                multicast: multicast.as_ref().map(|(_, _, info)| info.clone()),
            })
            .await;
    }
    // Öğrenciden gizlenmez: ekranının sınıfa gösterildiği açıkça bildirilir.
    let _ = task
        .source_tx
        .send(ServerMessage::Notice {
            message_id: Uuid::new_v4(),
            title: "Ekranınız sınıfa gösteriliyor".into(),
            body: "Öğretmeniniz ekranınızı sınıftaki diğer öğrencilere gösteriyor.".into(),
        })
        .await;
    // İzleyenler tam kareyle başlamalı.
    state.request_broadcast_keyframe();

    let mut ticker = tokio::time::interval(Duration::from_millis(250));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut seq: u64 = 0;

    loop {
        tokio::select! {
            _ = cancel.cancelled() => break,

            _ = ticker.tick() => {
                if !state.registry.is_online(source) {
                    tracing::info!(%source, "ekranı gösterilen öğrenci ayrıldı, yansıtma bitiyor");
                    break;
                }
                // İzleyenlerden biri eksik parça bildirdi ya da yeni katıldı:
                // tam kareyi kaynak öğrenci üretir.
                if state.take_broadcast_keyframe_request() {
                    let _ = task
                        .source_tx
                        .send(ServerMessage::RequestKeyframe { stream_id: 1 })
                        .await;
                }
            }

            frame = task.rx.recv() => {
                let Some(frame) = frame else { break };
                // İlk aktarılan güncelleme tam kare olmalı; öncesindeki
                // farklar izleyende boş tuvale çizilirdi.
                if seq == 0 && !frame.keyframe {
                    continue;
                }
                seq += 1;
                if seq == 1 {
                    tracing::info!(
                        genislik = frame.width,
                        yukseklik = frame.height,
                        bolge = frame.rects.len(),
                        "yansıtmanın ilk karesi aktarılıyor"
                    );
                }
                let message = ServerMessage::TeacherScreenUpdate {
                    seq,
                    width: frame.width,
                    height: frame.height,
                    keyframe: frame.keyframe,
                    codec: FrameCodec::Jpeg,
                    rects: frame.rects,
                };
                distribute(
                    &state,
                    multicast.as_ref(),
                    broadcast_id,
                    seq,
                    frame.keyframe,
                    &message,
                    Some(source),
                );
            }
        }
    }

    for (id, tx) in state.registry.all_senders() {
        if id != source {
            let _ = tx.send(ServerMessage::TeacherScreenStop).await;
        }
    }

    // Kaynak hâlâ bağlıysa akışını eski kalitesine döndür ve bittiğini bildir.
    if state.registry.is_online(source) {
        let quality = task.previous_quality;
        let _ = task
            .source_tx
            .send(ServerMessage::UpdateStreamQuality {
                stream_id: 1,
                quality,
            })
            .await;
        state
            .registry
            .update_flags(source, |f| f.quality = Some(quality));
        let _ = task
            .source_tx
            .send(ServerMessage::Notice {
                message_id: Uuid::new_v4(),
                title: "Ekran gösterimi bitti".into(),
                body: "Ekranınız artık sınıfa gösterilmiyor.".into(),
            })
            .await;
    }

    // Yalnızca kendi kaydını düşür: başlatma yarışında bu yansıtma hiç
    // kaydedilmemiş olabilir ve yuvada başka bir yayın duruyor olabilir.
    {
        let mut slot = state.broadcast.write();
        if slot.as_ref().map(|b| b.broadcast_id) == Some(broadcast_id) {
            *slot = None;
            state.broadcast_fallback.write().clear();
        }
    }
    state.emit_broadcast_or_none();
    tracing::info!(%broadcast_id, kare = seq, "öğrenci ekranı yansıtması bitti");
}

/// Bir yayın karesini izleyenlere ulaştırır: önce multicast, sonra TLS yedeği.
///
/// `exclude` yansıtmada kaynak öğrencidir; kendi ekranı ona geri gönderilmez.
fn distribute(
    state: &AppState,
    multicast: Option<&Multicast>,
    broadcast_id: Uuid,
    seq: u64,
    keyframe: bool,
    message: &ServerMessage,
    exclude: Option<Uuid>,
) {
    // 1. Multicast: kare telden **bir kez** geçer, anahtar kopyalar.
    //    30 öğrenciye TLS unicast ile göndermek aynı kareyi 30 kez
    //    telden geçirir ve 100 Mbit'lik laboratuvar anahtarını doldurur.
    let mut multicast_ok = false;
    if let Some((socket, target, info)) = multicast {
        match openclass_protocol::encode(message, openclass_protocol::MAX_CONTROL_FRAME) {
            Ok(payload) => {
                let key = openclass_protocol::multicast::SessionKey::new(&info.key);
                match openclass_protocol::multicast::split(
                    &key,
                    broadcast_id,
                    seq as u32,
                    keyframe,
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
        if Some(device_id) == exclude || (multicast_ok && !fallback.contains(&device_id)) {
            continue;
        }
        if tx.try_send(message.clone()).is_err() {
            tracing::trace!(%device_id, seq, "yayın karesi düşürüldü (yavaş istemci)");
        }
    }
}

/// Yayın döngüsü.
async fn run(
    state: Arc<AppState>,
    session: CaptureSession,
    broadcast_id: Uuid,
    fullscreen: bool,
    multicast: Option<Multicast>,
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
            rects,
        };

        distribute(
            &state,
            multicast.as_ref(),
            broadcast_id,
            seq,
            update.keyframe,
            &message,
            None,
        );
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
