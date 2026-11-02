//! Kontrol kanalı sunucusu: TLS + WebSocket.
//!
//! Öğretmen uygulaması **kendi içinde** bu sunucuyu barındırır; ayrı bir
//! sunucu programı yoktur. Uygulama kapanınca [`CancellationToken`] iptal
//! edilir, dinleyici durur ve açık bağlantılar düzgün kapatılır.
//!
//! # Bağlantı yaşam döngüsü
//!
//! ```text
//! TCP  →  TLS  →  WebSocket  →  Hello  →  ┬─ (kayıtlı)   → AuthChallenge → AuthResponse
//!                                         └─ (yeni)      → PairRequest (eşleştirme kodu)
//!                                                   ↓
//!                                            yetkilendirildi
//!                                                   ↓
//!                                    okuma döngüsü + yazma görevi
//! ```
//!
//! Yetkilendirme tamamlanmadan **hiçbir** komut işlenmez ve hiçbir ekran
//! karesi kabul edilmez.

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use futures_util::{SinkExt, StreamExt};
use openclass_protocol::auth::{verify_challenge, verify_pairing_proof, Challenge};
use openclass_protocol::control::{ClientMessage, DeviceStatus, ServerMessage};
use openclass_protocol::{MAX_CONTROL_FRAME, PROTOCOL_VERSION, WS_PATH};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_rustls::rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use tokio_rustls::rustls::ServerConfig;
use tokio_rustls::TlsAcceptor;
use tokio_tungstenite::tungstenite::Message;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::db::AuditKind;
use crate::state::{AppState, ConnectedDevice};

/// El sıkışmanın her adımı için azami bekleme.
/// Yavaş veya kasıtlı olarak asılı kalan bağlantılar kaynak tutmaz.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(15);

/// Bu süre boyunca hiç mesaj gelmezse bağlantı ölü sayılır.
const IDLE_TIMEOUT: Duration = Duration::from_secs(45);

/// Canlılık yoklaması aralığı.
const PING_INTERVAL: Duration = Duration::from_secs(15);

/// Bir cihaza gönderilmeyi bekleyen azami mesaj sayısı.
/// Dolarsa istemci yavaş demektir; mesaj düşürülür, sunucu bloke olmaz.
const SEND_QUEUE: usize = 64;

/// Sunucuyu başlatır ve bağlantıları kabul eder.
pub async fn run(state: Arc<AppState>, cancel: CancellationToken) -> Result<()> {
    let tls_config = build_tls_config(&state)?;
    let acceptor = TlsAcceptor::from(Arc::new(tls_config));

    let port = state.config.read().control_port;
    let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::UNSPECIFIED, port)))
        .await
        .with_context(|| {
            format!(
                "{port} portu dinlenemedi. En sık sebep: başka bir OpenClass Teacher \
                 örneği hâlâ çalışıyor. Çalışan örneği kapatıp yeniden deneyin. \
                 (Diğer olasılıklar: portu başka bir uygulama kullanıyor veya \
                 güvenlik duvarı engelliyor.) Bu konsol çalışır görünse de hiçbir \
                 öğrenci bağlanamaz."
            )
        })?;

    tracing::info!(
        port,
        parmak_izi = %state.tls.fingerprint_hex(),
        "kontrol sunucusu dinlemede"
    );

    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                tracing::info!("kontrol sunucusu durduruldu");
                return Ok(());
            }

            accepted = listener.accept() => {
                let (stream, peer) = match accepted {
                    Ok(pair) => pair,
                    Err(err) => {
                        tracing::warn!(%err, "TCP bağlantısı kabul edilemedi");
                        continue;
                    }
                };

                let state = Arc::clone(&state);
                let acceptor = acceptor.clone();
                let cancel = cancel.clone();
                tokio::spawn(async move {
                    if let Err(err) = handle_connection(state, acceptor, stream, peer, cancel).await {
                        // Beklenen kopmalar (öğrenci kapattı) da buraya düşer;
                        // bu yüzden seviye warn değil debug.
                        tracing::debug!(%peer, %err, "bağlantı sonlandı");
                    }
                });
            }
        }
    }
}

/// Kendinden imzalı sertifikadan rustls yapılandırması üretir.
fn build_tls_config(state: &AppState) -> Result<ServerConfig> {
    let cert = CertificateDer::from(state.tls.certificate_der.clone());
    let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(state.tls.private_key_der.clone()));

    let provider = Arc::new(tokio_rustls::rustls::crypto::ring::default_provider());
    let config = ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .context("TLS sürümleri yapılandırılamadı")?
        .with_no_client_auth()
        .with_single_cert(vec![cert], key)
        .context("TLS sertifikası yüklenemedi")?;

    Ok(config)
}

/// Tek bir öğrenci bağlantısını baştan sona yönetir.
///
/// `result_large_err`: WebSocket el sıkışma geri çağrısının hata türünü
/// `tungstenite` belirler (`http::Response`), bizim denetimimizde değil.
#[allow(clippy::result_large_err)]
async fn handle_connection(
    state: Arc<AppState>,
    acceptor: TlsAcceptor,
    stream: TcpStream,
    peer: SocketAddr,
    cancel: CancellationToken,
) -> Result<()> {
    stream.set_nodelay(true).ok();

    let tls = tokio::time::timeout(HANDSHAKE_TIMEOUT, acceptor.accept(stream))
        .await
        .context("TLS el sıkışması zaman aşımına uğradı")?
        .context("TLS el sıkışması başarısız")?;

    // WebSocket yolunu doğrula: yanlış yola gelen istemciler reddedilir.
    let ws = tokio::time::timeout(
        HANDSHAKE_TIMEOUT,
        tokio_tungstenite::accept_hdr_async(
            tls,
            |req: &tokio_tungstenite::tungstenite::handshake::server::Request, res| {
                if req.uri().path() == WS_PATH {
                    Ok(res)
                } else {
                    tracing::debug!(yol = %req.uri().path(), "beklenmeyen WebSocket yolu");
                    Err(tokio_tungstenite::tungstenite::http::Response::builder()
                        .status(404)
                        .body(Some("bilinmeyen uç nokta".to_string()))
                        .expect("sabit yanıt her zaman kurulabilir"))
                }
            },
        ),
    )
    .await
    .context("WebSocket el sıkışması zaman aşımına uğradı")?
    .context("WebSocket el sıkışması başarısız")?;

    let (mut sink, mut source) = ws.split();

    // El sıkışma: kimlik doğrulama veya eşleştirme.
    let authenticated = match tokio::time::timeout(
        HANDSHAKE_TIMEOUT,
        authenticate(&state, &mut sink, &mut source, peer),
    )
    .await
    {
        Ok(Ok(info)) => info,
        Ok(Err(err)) => {
            tracing::info!(%peer, %err, "yetkilendirme reddedildi");
            let _ = sink.close().await;
            return Ok(());
        }
        Err(_) => {
            tracing::info!(%peer, "yetkilendirme zaman aşımına uğradı");
            let _ = sink.close().await;
            return Ok(());
        }
    };

    let device_id = authenticated.device_id;
    let hostname = authenticated.hostname.clone();
    let room = authenticated.room.clone();

    // Yazma görevi: kanala düşen mesajları WebSocket'e aktarır.
    let (tx, mut rx) = mpsc::channel::<ServerMessage>(SEND_QUEUE);
    let writer = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            let bytes = match openclass_protocol::encode(&msg, MAX_CONTROL_FRAME) {
                Ok(b) => b,
                Err(err) => {
                    tracing::error!(%err, "giden mesaj kodlanamadı");
                    continue;
                }
            };
            if sink.send(Message::Binary(bytes.into())).await.is_err() {
                break;
            }
        }
        let _ = sink.close().await;
    });

    // Aynı cihazın eski oturumu varsa kapat (yeniden bağlanma senaryosu).
    if let Some(previous) = state.registry.attach(ConnectedDevice {
        device_id,
        hostname: hostname.clone(),
        address: peer,
        connected_at: crate::db::now(),
        sender: tx.clone(),
    }) {
        tracing::info!(%device_id, "aynı cihazın önceki oturumu kapatılıyor");
        drop(previous);
    }

    state.db.touch_device(device_id, &hostname, room.as_deref()).ok();
    state
        .db
        .audit(AuditKind::DeviceConnected, Some(device_id), Some(&hostname));
    state.notify_devices_changed();
    tracing::info!(%peer, %device_id, %hostname, "öğrenci bağlandı");

    // Ders zaten başlamışsa geç katılan cihaza durumu bildir.
    //
    // Kilitler `await` noktasından ÖNCE bırakılır: `parking_lot` guard'ı
    // `Send` değildir, await üzerinden taşınırsa görev spawn edilemez.
    // Ekran akışını kendiliğinden başlat. Öğretmenin cihazı seçip
    // "Ekranları Göster" demesini beklemek gereksiz bir adımdı: konsolu açan
    // öğretmen zaten ekranları görmek istiyor.
    //
    // Öğrencide bu gizli değil — tepsi simgesi değişir ve "EKRANINIZ
    // PAYLAŞILIYOR" yazar. Görünür gösterge, kaldırılan ders oturumu
    // kapısının yerine geçen tek koruma olduğu için **hiçbir koşulda**
    // atlanmaz.
    {
        let quality = openclass_protocol::StreamQuality::PREVIEW;
        if tx
            .send(ServerMessage::StartScreenStream {
                stream_id: 1,
                quality,
            })
            .await
            .is_ok()
        {
            state.registry.update_flags(device_id, |f| {
                f.streaming = true;
                f.quality = Some(quality);
            });
            tracing::info!(%device_id, %hostname, "ekran akışı kendiliğinden başlatıldı");
        }
    }

    // Yayın sürüyorsa geç katılan öğrenciye de bildir; bir sonraki anahtar
    // kare onu toparlar.
    let pending_broadcast = {
        let guard = state.broadcast.read();
        guard
            .as_ref()
            .map(|b| (b.broadcast_id, b.fullscreen, b.multicast.clone()))
    };
    if let Some((broadcast_id, fullscreen, multicast)) = pending_broadcast {
        let teacher_name = state.config.read().teacher_name.clone();
        let _ = tx
            .send(ServerMessage::TeacherScreenStart {
                broadcast_id,
                teacher_name,
                fullscreen,
                multicast,
            })
            .await;
        state.request_broadcast_keyframe();
    }

    let result = read_loop(&state, &mut source, &tx, device_id, &hostname, cancel).await;

    // Temizlik: bağlantı nasıl biterse bitsin sistem güvenli duruma döner.
    // Kontrol edilen cihaz düşerse kontrol de biter.
    {
        let mut controlled = state.controlled_device.write();
        if *controlled == Some(device_id) {
            *controlled = None;
        }
    }
    state.broadcast_fallback.write().remove(&device_id);
    // Gezilen cihaz düşerse gezgin kapanır ve yarım indirme temizlenir.
    {
        let closed = {
            let mut guard = state.browse.write();
            match guard.as_ref() {
                Some(s) if s.device_id == device_id => {
                    if let Some(session) = guard.take() {
                        if let Some(download) = session.download {
                            download.abort();
                        }
                    }
                    true
                }
                _ => false,
            }
        };
        if closed {
            state.emit("browse-changed", Option::<crate::browse::BrowseView>::None);
        }
    }
    state.registry.detach(device_id);
    state.db.audit(
        AuditKind::DeviceDisconnected,
        Some(device_id),
        Some(&hostname),
    );
    state.notify_devices_changed();
    drop(tx);
    writer.abort();
    tracing::info!(%device_id, %hostname, "öğrenci bağlantısı kapandı");

    result
}

/// Yetkilendirilmiş bağlantının bilgisi.
/// Oda adının kabul edilen en fazla karakter sayısı.
///
/// Öğrenci tarafında da aynı sınır uygulanır; buradaki kırpma, değiştirilmiş
/// bir istemciden uzun değer gelmesine karşı ikinci savunmadır.
const ROOM_MAX_CHARS: usize = 64;

struct Authenticated {
    device_id: Uuid,
    hostname: String,
    /// Öğrencinin bildirdiği oda adı; eski sürüm ajanlarda `None`.
    room: Option<String>,
}

type WsSink = futures_util::stream::SplitSink<
    tokio_tungstenite::WebSocketStream<tokio_rustls::server::TlsStream<TcpStream>>,
    Message,
>;
type WsSource = futures_util::stream::SplitStream<
    tokio_tungstenite::WebSocketStream<tokio_rustls::server::TlsStream<TcpStream>>,
>;

/// Yetkilendirme el sıkışması.
async fn authenticate(
    state: &Arc<AppState>,
    sink: &mut WsSink,
    source: &mut WsSource,
    peer: SocketAddr,
) -> Result<Authenticated> {
    let ClientMessage::Hello {
        protocol,
        device_id,
        hostname,
        app_version,
        public_key,
        room,
    } = recv(source).await?
    else {
        bail!("ilk mesaj Hello olmalıydı");
    };

    if protocol != PROTOCOL_VERSION {
        send(sink, &ServerMessage::AuthResult {
            ok: false,
            reason: Some(format!(
                "Protokol sürümü uyuşmuyor. Öğrenci uygulaması v{protocol}, öğretmen konsolu v{PROTOCOL_VERSION}. \
                 Her iki uygulamayı da aynı sürüme güncelleyin."
            )),
                reset_pairing: false,
            })
        .await?;
        bail!("protokol sürümü uyuşmuyor: {protocol}");
    }

    let hostname = hostname.trim().to_ascii_uppercase();
    if hostname.is_empty() {
        bail!("boş bilgisayar adı");
    }

    // Oda adı öğrencinin beyanıdır; yetki kararında **kullanılmaz**, yalnızca
    // listede gruplamaya yarar. Bu yüzden reddetmek yerine temizlenir: boş ya
    // da yalnızca boşluktan oluşan değer `None` olur, uzun değer kırpılır.
    let room = room
        .map(|r| r.trim().chars().take(ROOM_MAX_CHARS).collect::<String>())
        .filter(|r| !r.is_empty());

    let claimed_key: [u8; 32] = public_key
        .as_slice()
        .try_into()
        .map_err(|_| anyhow::anyhow!("açık anahtar 32 bayt değil"))?;

    // Kilit guard'ı await üzerinden taşınmasın diye ad baştan kopyalanır.
    let teacher_name = state.config.read().teacher_name.clone();
    let record = state.db.device(device_id)?;
    tracing::debug!(%peer, %device_id, %hostname, %app_version, kayitli = record.is_some(), "Hello alındı");

    // Öğretmen o anda eşleştirme kodu üretmiş mi?
    //
    // Yetkisi kaldırılmış bir cihazın geri alınabilmesi buna bağlı: kod
    // üretmek öğretmenin bilinçli eylemidir ve "bu cihazı tekrar kabul
    // ediyorum" demektir. Kod yokken reddedilmiş cihaz kabul edilmez.
    //
    // Bu olmadan "Yetkiyi Kaldır" geri alınamaz bir yasak olurdu; yanlışlıkla
    // basan öğretmenin cihazı kurtarmasının hiçbir yolu kalmazdı.
    let pairing_open = state.pairing.read().as_ref().is_some_and(|p| p.is_valid());

    match record {
        // Yetkisi kaldırılmış cihaz: eşleştirme kodu açık değilse kabul
        // edilmez.
        Some(rec) if rec.status == DeviceStatus::Rejected && !pairing_open => {
            send(
                sink,
                &ServerMessage::HelloAck {
                    protocol: PROTOCOL_VERSION,
                    teacher_id: state.teacher_id,
                    teacher_name: teacher_name.clone(),
                    status: DeviceStatus::Rejected,
                },
            )
            .await?;
            send(
                sink,
                &ServerMessage::AuthResult {
                    ok: false,
                    reason: Some(
                        "Bu cihazın yetkisi öğretmen tarafından kaldırılmış. \
                         Yeniden bağlanmak için öğretmenden eşleştirme kodu isteyin."
                            .into(),
                    ),
                    // Öğrenci kayıtlı eşleştirmesini bıraksın ve yeni kod
                    // girebilecek duruma dönsün; aksi hâlde geçersiz
                    // kimliğiyle sonsuza kadar denerdi.
                    reset_pairing: true,
                },
            )
            .await?;
            state.db.audit(
                AuditKind::AuthFailed,
                Some(device_id),
                Some("yetkisi kaldırılmış cihaz"),
            );
            bail!("yetkisi kaldırılmış cihaz bağlanmaya çalıştı");
        }

        // Tanınan ve açık anahtarı eşleşen cihaz: challenge/imza ile doğrulanır.
        Some(rec) if rec.status == DeviceStatus::Authorized && rec.public_key == claimed_key => {
            send(
                sink,
                &ServerMessage::HelloAck {
                    protocol: PROTOCOL_VERSION,
                    teacher_id: state.teacher_id,
                    teacher_name: teacher_name.clone(),
                    status: DeviceStatus::Authorized,
                },
            )
            .await?;

            let challenge: Challenge = openclass_protocol::random_bytes::<32>();
            send(
                sink,
                &ServerMessage::AuthChallenge {
                    challenge: challenge.to_vec(),
                },
            )
            .await?;

            let ClientMessage::AuthResponse { signature } = recv(source).await? else {
                bail!("AuthResponse bekleniyordu");
            };
            let signature: [u8; 64] = signature
                .as_slice()
                .try_into()
                .map_err(|_| anyhow::anyhow!("imza 64 bayt değil"))?;

            if let Err(err) = verify_challenge(
                &rec.public_key,
                &state.tls.fingerprint,
                &challenge,
                &signature,
            ) {
                send(
                    sink,
                    &ServerMessage::AuthResult {
                        ok: false,
                        reason: Some("Kimlik doğrulama başarısız.".into()),
                        reset_pairing: false,
                    },
                )
                .await?;
                state
                    .db
                    .audit(AuditKind::AuthFailed, Some(device_id), Some(&hostname));
                return Err(err).context("challenge imzası doğrulanamadı");
            }

            send(
                sink,
                &ServerMessage::AuthResult {
                    ok: true,
                    reason: None,
                    reset_pairing: false,
                },
            )
            .await?;
            Ok(Authenticated {
                device_id,
                hostname,
                room,
            })
        }

        // Yeni cihaz, bekleyen cihaz veya anahtarı değişmiş cihaz: eşleştirme gerekir.
        other => {
            if let Some(rec) = &other {
                if rec.status == DeviceStatus::Authorized {
                    // Yetkili bir cihaz kimliğiyle farklı bir anahtar sunuldu.
                    // Meşru hâli: öğrenci uygulaması yeniden kuruldu.
                    // Kötü hâli: başka bir cihaz bu kimliği taklit ediyor.
                    // Her iki durumda da yeniden eşleştirme şart.
                    tracing::warn!(
                        %device_id, %hostname, %peer,
                        "kayıtlı cihaz farklı bir açık anahtarla geldi — yeniden eşleştirme isteniyor"
                    );
                }
            }

            send(
                sink,
                &ServerMessage::HelloAck {
                    protocol: PROTOCOL_VERSION,
                    teacher_id: state.teacher_id,
                    teacher_name: teacher_name.clone(),
                    status: DeviceStatus::PendingPairing,
                },
            )
            .await?;

            let ClientMessage::PairRequest { public_key, proof } = recv(source).await? else {
                bail!("PairRequest bekleniyordu");
            };
            let pair_key: [u8; 32] = public_key
                .as_slice()
                .try_into()
                .map_err(|_| anyhow::anyhow!("açık anahtar 32 bayt değil"))?;
            let proof: [u8; 32] = proof
                .as_slice()
                .try_into()
                .map_err(|_| anyhow::anyhow!("eşleştirme kanıtı 32 bayt değil"))?;

            // Kodu kilit altında doğrula ve hatalı denemeyi hemen say.
            let verdict = {
                let mut guard = state.pairing.write();
                match guard.as_mut() {
                    None => Err("Öğretmen konsolunda etkin bir eşleştirme kodu yok."),
                    Some(active) if !active.is_valid() => {
                        Err("Eşleştirme kodunun süresi doldu veya deneme hakkı bitti.")
                    }
                    Some(active) => {
                        match verify_pairing_proof(
                            &active.code,
                            &state.tls.fingerprint,
                            &device_id,
                            &pair_key,
                            &proof,
                        ) {
                            Ok(()) => Ok(()),
                            Err(_) => {
                                active.failed_attempts += 1;
                                Err("Eşleştirme kodu hatalı.")
                            }
                        }
                    }
                }
            };

            match verdict {
                Ok(()) => {
                    state.db.upsert_paired_device(
                        device_id,
                        &hostname,
                        &pair_key,
                        room.as_deref(),
                    )?;
                    state
                        .db
                        .audit(AuditKind::DevicePaired, Some(device_id), Some(&hostname));
                    // Kod tek kullanımlıktır. (Test modunda sabit kod
                    // hemen yeniden kurulur; bkz. `crate::testing`.)
                    *state.pairing.write() = None;
                    crate::testing::rearm_pairing(state);

                    send(
                        sink,
                        &ServerMessage::PairResult {
                            accepted: true,
                            reason: None,
                        },
                    )
                    .await?;
                    send(
                        sink,
                        &ServerMessage::AuthResult {
                            ok: true,
                            reason: None,
                            reset_pairing: false,
                        },
                    )
                    .await?;

                    state.notify_devices_changed();
                    state.emit("pairing-changed", ());
                    tracing::info!(%device_id, %hostname, "cihaz eşleştirildi");
                    Ok(Authenticated {
                        device_id,
                        hostname,
                        room,
                    })
                }
                Err(reason) => {
                    send(
                        sink,
                        &ServerMessage::PairResult {
                            accepted: false,
                            reason: Some(reason.to_string()),
                        },
                    )
                    .await?;
                    state
                        .db
                        .audit(AuditKind::DeviceRejected, Some(device_id), Some(&hostname));
                    state.emit("pairing-changed", ());
                    bail!("eşleştirme reddedildi: {reason}");
                }
            }
        }
    }
}

/// Yetkilendirilmiş bağlantının ana okuma döngüsü.
async fn read_loop(
    state: &Arc<AppState>,
    source: &mut WsSource,
    tx: &mpsc::Sender<ServerMessage>,
    device_id: Uuid,
    hostname: &str,
    cancel: CancellationToken,
) -> Result<()> {
    let mut ping_timer = tokio::time::interval(PING_INTERVAL);
    ping_timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut ping_token: u64 = 0;

    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                // Konsol kapanıyor: öğrenciye yayını durdurmasını söyle.
                // Bağlantının kopması da aynı sonucu verir ama açık bildirim
                // göstergeyi anında düşürür, öğrenci bir an bile gereksiz
                // yere "paylaşılıyor" görmez.
                let _ = tx.send(ServerMessage::StopScreenStream { stream_id: 0 }).await;
                return Ok(());
            }

            _ = ping_timer.tick() => {
                ping_token = ping_token.wrapping_add(1);
                if tx.send(ServerMessage::Ping { token: ping_token }).await.is_err() {
                    bail!("yazma kanalı kapandı");
                }
            }

            incoming = tokio::time::timeout(IDLE_TIMEOUT, source.next()) => {
                let frame = match incoming {
                    Err(_) => bail!("bağlantı {}s boyunca sessiz kaldı", IDLE_TIMEOUT.as_secs()),
                    Ok(None) => return Ok(()),
                    Ok(Some(Err(err))) => return Err(err).context("WebSocket okuma hatası"),
                    Ok(Some(Ok(frame))) => frame,
                };

                match frame {
                    Message::Binary(bytes) => {
                        let msg: ClientMessage =
                            openclass_protocol::decode(&bytes, MAX_CONTROL_FRAME)
                                .context("gelen mesaj çözülemedi")?;
                        handle_client_message(state, msg, device_id, hostname).await;
                    }
                    Message::Close(_) => return Ok(()),
                    // Metin ve WebSocket seviyesi ping/pong beklenmez; yok sayılır.
                    _ => {}
                }
            }
        }
    }
}

/// Yetkilendirilmiş cihazdan gelen tek bir mesajı işler.
async fn handle_client_message(
    state: &Arc<AppState>,
    msg: ClientMessage,
    device_id: Uuid,
    hostname: &str,
) {
    match msg {
        ClientMessage::ScreenUpdate {
            seq,
            width,
            height,
            keyframe,
            rects,
            stream_id,
            ..
        } => {
            let bytes: usize = rects.iter().map(|r| r.data.len()).sum();

            // İlk veri gelişi `info` seviyesinde: "yayın başladı ama ekran
            // gelmiyor" durumunu ayırt edebilmek için ayrıntılı loglama
            // gerektirmemeli.
            if state.registry.first_screen_update(device_id) {
                tracing::info!(
                    %device_id, %hostname, width, height, keyframe,
                    bolge = rects.len(), bayt = bytes,
                    "ilk ekran verisi alındı"
                );
            }

            if seq % 30 == 0 {
                tracing::debug!(
                    %device_id, %hostname, seq, width, height, keyframe,
                    bolge = rects.len(), bayt = bytes,
                    "ekran güncellemesi alındı"
                );
            }

            let needs_keyframe = state.registry.push_screen_update(
                device_id,
                openclass_screen::StoredUpdate {
                    seq,
                    keyframe,
                    rects,
                    bytes,
                },
                width,
                height,
            );

            state.registry.update_flags(device_id, |f| {
                f.streaming = true;
                f.screen_error = None;
            });

            // Birikim tutarsızsa öğrenciden tam kare iste.
            if needs_keyframe {
                if let Some(tx) = state.registry.sender(device_id) {
                    let _ = tx.send(ServerMessage::RequestKeyframe { stream_id }).await;
                    state.registry.clear_keyframe_request(device_id);
                }
            }

            // Arayüze yalnızca "yeni veri var" bilgisi gider; bölgelerin
            // kendisi `ocframe://` üzerinden ikili olarak çekilir.
            state.emit(
                "frame",
                FrameEvent {
                    device_id,
                    seq,
                    width,
                    height,
                },
            );
        }

        ClientMessage::ScreenError { reason, .. } => {
            tracing::warn!(%device_id, %hostname, %reason, "öğrenci ekranı yakalanamadı");
            state.registry.update_flags(device_id, |f| {
                f.streaming = false;
                f.screen_error = Some(reason);
            });
            state.notify_devices_changed();
        }

        ClientMessage::HelpRequest { note } => {
            tracing::info!(%device_id, %hostname, "yardım talebi alındı");
            state
                .registry
                .update_flags(device_id, |f| f.help_requested = true);
            state
                .db
                .audit(AuditKind::HelpRequested, Some(device_id), Some(hostname));
            state.emit(
                "help-request",
                HelpEvent {
                    device_id,
                    hostname: hostname.to_string(),
                    note,
                },
            );
            state.notify_devices_changed();
            if let Some(tx) = state.registry.sender(device_id) {
                let _ = tx.send(ServerMessage::HelpAcknowledged).await;
            }
        }

        ClientMessage::RequestTeacherKeyframe => {
            tracing::debug!(%device_id, "öğrenci yayında tam kare istedi");
            state.request_broadcast_keyframe();
        }

        ClientMessage::FileAccepted { transfer_id } => {
            tracing::info!(%device_id, %transfer_id, "dosya teklifi kabul edildi");
            crate::transfer::set_target(
                state,
                transfer_id,
                device_id,
                crate::transfer::TargetState::Sending { sent: 0 },
            );
        }

        ClientMessage::FileRejected {
            transfer_id,
            reason,
        } => {
            tracing::info!(%device_id, %transfer_id, %reason, "dosya teklifi reddedildi");
            crate::transfer::set_target(
                state,
                transfer_id,
                device_id,
                crate::transfer::TargetState::Rejected { reason },
            );
        }

        ClientMessage::FileDone {
            transfer_id,
            ok,
            detail,
        } => {
            if ok {
                tracing::info!(%device_id, %transfer_id, "dosya alıcıda doğrulandı");
            } else {
                tracing::warn!(%device_id, %transfer_id, %detail, "dosya alıcıda başarısız");
            }
            crate::transfer::set_target(
                state,
                transfer_id,
                device_id,
                if ok {
                    crate::transfer::TargetState::Done { path: detail }
                } else {
                    crate::transfer::TargetState::Failed { reason: detail }
                },
            );
        }

        ClientMessage::BrowseListing {
            request_id,
            path,
            roots,
            entries,
        } => {
            let mut guard = state.browse.write();
            let Some(session) = guard.as_mut() else {
                return;
            };
            if session.device_id != device_id {
                return;
            }
            tracing::info!(%device_id, %request_id, %path, adet = entries.len(), "klasör listesi alındı");
            session.path = path;
            session.roots = roots;
            session.entries = entries;
            session.loading = false;
            session.error = None;
            let view = crate::browse::BrowseView::from(&*session);
            drop(guard);
            state.emit("browse-changed", Some(view));
        }

        ClientMessage::BrowseFailed { request_id, reason } => {
            let mut guard = state.browse.write();
            let Some(session) = guard.as_mut() else {
                return;
            };
            if session.device_id != device_id {
                return;
            }
            tracing::warn!(%device_id, %request_id, %reason, "gezgin isteği reddedildi");
            session.loading = false;
            session.error = Some(reason);
            // Yarım kalmış indirme varsa temizlenir.
            if let Some(download) = session.download.take() {
                download.abort();
            }
            let view = crate::browse::BrowseView::from(&*session);
            drop(guard);
            state.emit("browse-changed", Some(view));
        }

        ClientMessage::BrowseFileStart {
            request_id,
            name,
            size,
            sha256,
        } => {
            let hostname = {
                let guard = state.browse.read();
                match guard.as_ref() {
                    Some(s) if s.device_id == device_id => s.hostname.clone(),
                    _ => return,
                }
            };

            // Kök kilidi `await` üzerinden taşınmasın diye kopyalanır.
            let kok = state.download_root.read().clone();
            match crate::browse::Download::open(
                request_id,
                &hostname,
                &name,
                size,
                sha256,
                kok.as_deref(),
            ) {
                Ok(download) => {
                    let mut guard = state.browse.write();
                    let Some(session) = guard.as_mut() else {
                        return;
                    };
                    tracing::info!(%device_id, %request_id, %name, size, "indirme başladı");
                    session.download_view = Some(crate::browse::DownloadView {
                        name: download.name.clone(),
                        size,
                        received: 0,
                        finished: false,
                        ok: false,
                        detail: String::new(),
                    });
                    session.download = Some(download);
                    let view = crate::browse::BrowseView::from(&*session);
                    drop(guard);
                    state.emit("browse-changed", Some(view));
                }
                Err(err) => {
                    tracing::error!(%err, "indirme hazırlanamadı");
                    let mut guard = state.browse.write();
                    if let Some(session) = guard.as_mut() {
                        session.error = Some(err.to_string());
                        let view = crate::browse::BrowseView::from(&*session);
                        drop(guard);
                        state.emit("browse-changed", Some(view));
                    }
                }
            }
        }

        ClientMessage::BrowseChunk {
            request_id,
            seq,
            data,
        } => {
            let mut guard = state.browse.write();
            let Some(session) = guard.as_mut() else {
                return;
            };
            if session.device_id != device_id {
                return;
            }
            let Some(download) = session.download.as_mut() else {
                return;
            };
            if download.request_id != request_id {
                return;
            }

            if let Err(err) = download.write_chunk(seq, &data) {
                tracing::error!(%err, "indirme parçası yazılamadı");
                let detail = err.to_string();
                if let Some(aborted) = session.download.take() {
                    aborted.abort();
                }
                session.download_view = Some(crate::browse::DownloadView {
                    name: String::new(),
                    size: 0,
                    received: 0,
                    finished: true,
                    ok: false,
                    detail,
                });
            } else {
                let received = download.received;
                if let Some(view) = session.download_view.as_mut() {
                    view.received = received;
                }
            }
            let view = crate::browse::BrowseView::from(&*session);
            drop(guard);
            state.emit("browse-changed", Some(view));
        }

        ClientMessage::BrowseFileEnd { request_id } => {
            let mut guard = state.browse.write();
            let Some(session) = guard.as_mut() else {
                return;
            };
            if session.device_id != device_id {
                return;
            }
            let Some(download) = session.download.take() else {
                return;
            };
            if download.request_id != request_id {
                download.abort();
                return;
            }

            let name = download.name.clone();
            let size = download.size;
            let (ok, detail) = match download.finish() {
                Ok(path) => {
                    tracing::info!(%device_id, yol = %path.display(), "dosya indirildi");
                    (true, path.display().to_string())
                }
                Err(err) => {
                    tracing::error!(%device_id, %err, "indirme doğrulanamadı");
                    (false, err.to_string())
                }
            };

            session.download_view = Some(crate::browse::DownloadView {
                name: name.clone(),
                size,
                received: size,
                finished: true,
                ok,
                detail: detail.clone(),
            });
            let view = crate::browse::BrowseView::from(&*session);
            drop(guard);

            // Her indirme denetim kaydına yazılır.
            state
                .db
                .audit(AuditKind::FileDownloaded, Some(device_id), Some(&name));
            state.emit("browse-changed", Some(view));
        }

        ClientMessage::MulticastUnavailable { broadcast_id } => {
            // Bu öğrenciye multicast ulaşmıyor; kareleri TLS üzerinden tek
            // tek göndermeye geçilir. Bant genişliği maliyeti artar ama
            // öğrenci ekranı görmeye devam eder.
            let current = state.broadcast.read().as_ref().map(|b| b.broadcast_id);
            if current != Some(broadcast_id) {
                return;
            }
            if state.broadcast_fallback.write().insert(device_id) {
                tracing::warn!(
                    %device_id,
                    %hostname,
                    "multicast ulaşmıyor, TLS yedeğine geçildi"
                );
                state.emit_broadcast();
            }
            // Yedek yola geçen öğrenci baştan tam kare almalı.
            state.request_broadcast_keyframe();
        }

        ClientMessage::HelpCancel => {
            state
                .registry
                .update_flags(device_id, |f| f.help_requested = false);
            state.notify_devices_changed();
        }

        ClientMessage::MessageAck { message_id } => {
            tracing::debug!(%device_id, %message_id, "mesaj görüldü");
        }

        ClientMessage::Pong { .. } => {}

        // Yetkilendirme mesajları el sıkışmaya aittir; burada gelmeleri
        // protokol dışıdır ve yok sayılır.
        ClientMessage::Hello { .. }
        | ClientMessage::PairRequest { .. }
        | ClientMessage::AuthResponse { .. } => {
            tracing::warn!(%device_id, "el sıkışma mesajı oturum ortasında geldi, yok sayıldı");
        }
    }
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct FrameEvent {
    device_id: Uuid,
    seq: u64,
    width: u32,
    height: u32,
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct HelpEvent {
    device_id: Uuid,
    hostname: String,
    note: Option<String>,
}

/// Tek bir mesaj gönderir.
async fn send(sink: &mut WsSink, msg: &ServerMessage) -> Result<()> {
    let bytes = openclass_protocol::encode(msg, MAX_CONTROL_FRAME)?;
    sink.send(Message::Binary(bytes.into()))
        .await
        .context("mesaj gönderilemedi")?;
    Ok(())
}

/// Tek bir ikili mesaj alır; başka çerçeve türleri atlanır.
async fn recv(source: &mut WsSource) -> Result<ClientMessage> {
    loop {
        match source.next().await {
            None => bail!("bağlantı beklenmedik şekilde kapandı"),
            Some(Err(err)) => return Err(err).context("WebSocket okuma hatası"),
            Some(Ok(Message::Binary(bytes))) => {
                return openclass_protocol::decode(&bytes, MAX_CONTROL_FRAME)
                    .context("gelen mesaj çözülemedi")
            }
            Some(Ok(Message::Close(_))) => bail!("karşı taraf bağlantıyı kapattı"),
            Some(Ok(_)) => continue,
        }
    }
}
