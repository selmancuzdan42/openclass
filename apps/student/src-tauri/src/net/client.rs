//! Öğretmen konsoluna bağlantı: eşleştirme, kimlik doğrulama, ekran yayını.
//!
//! Tek bir görev olarak çalışır ve kendi kendini onarır:
//!
//! * Bağlantı koparsa üstel geri çekilmeyle (1s → 30s) yeniden denenir.
//! * Öğretmenin IP'si değişirse keşifte görülen yeni adres kullanılır.
//! * Öğretmen konsolu yeniden açıldığında bağlantı kendiliğinden kurulur.
//!
//! Ekran yayını **yalnızca** öğretmen açıkça başlattığında ve aktif bir ders
//! oturumu varken çalışır; ders bitince veya bağlantı koparsa durur.

use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use futures_util::{SinkExt, StreamExt};
use openclass_protocol::auth::pairing_proof;
use openclass_protocol::control::{
    ClientMessage, DeviceStatus, FrameCodec, ScreenRect, ServerMessage, StreamQuality,
};
use openclass_protocol::{MAX_CONTROL_FRAME, PROTOCOL_VERSION, WS_PATH};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_rustls::rustls::pki_types::ServerName;
use tokio_rustls::rustls::ClientConfig;
use tokio_rustls::TlsConnector;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;
use tokio_util::sync::CancellationToken;

use crate::config::PairedTeacher;
use crate::net::pinned_tls::PinnedFingerprintVerifier;
use crate::state::{ActiveSession, AppState, ConnectionStatus, ReceivedNotice, SharingState};
use openclass_screen::CaptureSession;
use openclass_screen::TileEncoder;

/// İlk yeniden deneme gecikmesi.
const BACKOFF_MIN: Duration = Duration::from_secs(1);
/// Azami yeniden deneme gecikmesi.
const BACKOFF_MAX: Duration = Duration::from_secs(30);
/// El sıkışma adımları için üst sınır.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(15);
/// Bu süre boyunca öğretmenden hiç mesaj gelmezse bağlantı ölü sayılır.
const IDLE_TIMEOUT: Duration = Duration::from_secs(45);
/// Giden mesaj kuyruğu. Dolarsa kareler düşürülür; bellek şişmez.
const SEND_QUEUE: usize = 8;

/// Bağlantı döngüsü. Uygulama kapanana kadar çalışır.
pub async fn run(state: Arc<AppState>, cancel: CancellationToken) {
    let mut backoff = BACKOFF_MIN;

    loop {
        if cancel.is_cancelled() {
            return;
        }

        let intent = state.pair_intent.write().take();

        let outcome = if let Some(intent) = intent {
            state.set_status(ConnectionStatus::Pairing);
            let expected = intent
                .expected_fingerprint
                .as_deref()
                .and_then(to_fingerprint);
            connect_once(
                &state,
                &intent.host,
                intent.port,
                expected,
                Some(intent.code.clone()),
                cancel.clone(),
            )
            .await
        } else if let Some(target) = state.connect_target() {
            state.set_status(ConnectionStatus::Connecting);
            let Some(expected) = to_fingerprint(&target.fingerprint) else {
                state.set_status(ConnectionStatus::Disconnected {
                    reason: Some(
                        "Kayıtlı öğretmen parmak izi bozuk. Lütfen yeniden eşleştirin.".into(),
                    ),
                    retry_in_secs: 0,
                });
                wait_for_wake(&state, &cancel).await;
                continue;
            };
            connect_once(
                &state,
                &target.host,
                target.port,
                Some(expected),
                None,
                cancel.clone(),
            )
            .await
        } else {
            state.set_status(ConnectionStatus::NotPaired);
            stop_sharing(&state);

            // Test modunda sabit kodla kendiliğinden eşleş; öğretmen henüz
            // keşfedilmediyse kısa aralıklarla yeniden dene.
            if crate::testing::try_autopair(&state) {
                continue;
            }
            if crate::testing::autopair_code().is_some() {
                tokio::select! {
                    _ = cancel.cancelled() => return,
                    _ = state.wake.notified() => {}
                    _ = tokio::time::sleep(Duration::from_secs(2)) => {}
                }
            } else {
                wait_for_wake(&state, &cancel).await;
            }
            continue;
        };

        stop_sharing(&state);
        // Bağlantı koptuğunda kontrol, kilit ve gezgin kendiliğinden kapanır.
        // Multicast alıcısı okuma döngüsünün muhafızıyla durur.
        state.stop_remote_control();
        state.unlock_screen();
        state.stop_browsing();
        *state.session.write() = None;

        if cancel.is_cancelled() {
            return;
        }

        let reason = match outcome {
            Ok(()) => {
                backoff = BACKOFF_MIN;
                None
            }
            Err(err) => {
                let text = format!("{err:#}");
                tracing::warn!(hata = %text, "bağlantı başarısız");
                Some(text)
            }
        };

        state.set_status(ConnectionStatus::Disconnected {
            reason,
            retry_in_secs: backoff.as_secs(),
        });

        tokio::select! {
            _ = cancel.cancelled() => return,
            _ = state.wake.notified() => backoff = BACKOFF_MIN,
            _ = tokio::time::sleep(backoff) => {
                backoff = (backoff * 2).min(BACKOFF_MAX);
            }
        }
    }
}

async fn wait_for_wake(state: &Arc<AppState>, cancel: &CancellationToken) {
    tokio::select! {
        _ = cancel.cancelled() => {}
        _ = state.wake.notified() => {}
    }
}

fn to_fingerprint(bytes: &[u8]) -> Option<[u8; 32]> {
    <[u8; 32]>::try_from(bytes).ok()
}

fn stop_sharing(state: &Arc<AppState>) {
    let current = state.sharing.read().clone();
    if current.active {
        state.set_sharing(SharingState {
            active: false,
            ..SharingState::default()
        });
    }
}

/// Tek bir bağlantı denemesi. Bağlantı kopana kadar döner.
async fn connect_once(
    state: &Arc<AppState>,
    host: &str,
    port: u16,
    expected_fingerprint: Option<[u8; 32]>,
    pairing_code: Option<String>,
    cancel: CancellationToken,
) -> Result<()> {
    let provider = Arc::new(tokio_rustls::rustls::crypto::ring::default_provider());
    let verifier = match expected_fingerprint {
        Some(fp) => PinnedFingerprintVerifier::pinned(fp, Arc::clone(&provider)),
        // Öğrenme modu yalnızca eşleştirme akışında kullanılır; kimliği
        // doğrulayan asıl mekanizma eşleştirme kodudur.
        None => PinnedFingerprintVerifier::learning(Arc::clone(&provider)),
    };

    let tls_config = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .context("TLS sürümleri yapılandırılamadı")?
        .dangerous()
        .with_custom_certificate_verifier(Arc::clone(&verifier) as Arc<_>)
        .with_no_client_auth();

    let server_name = match host.parse::<std::net::IpAddr>() {
        Ok(ip) => ServerName::IpAddress(ip.into()),
        Err(_) => ServerName::try_from(host.to_string())
            .map_err(|_| anyhow!("geçersiz sunucu adı: {host}"))?,
    };

    let tcp = tokio::time::timeout(HANDSHAKE_TIMEOUT, TcpStream::connect((host, port)))
        .await
        .map_err(|_| anyhow!("{host}:{port} adresine bağlanılamadı (zaman aşımı)"))?
        .with_context(|| format!("{host}:{port} adresine bağlanılamadı"))?;
    tcp.set_nodelay(true).ok();
    // Multicast gruba katılırken doğru arayüzü seçmek için öğretmenin
    // gerçek adresi gerekir; ad çözümlemesi sonrası bu adres kesindir.
    let teacher_ip = tcp.peer_addr().map(|a| a.ip()).unwrap_or_else(|_| {
        host.parse()
            .unwrap_or(std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED))
    });

    let tls = TlsConnector::from(Arc::new(tls_config))
        .connect(server_name, tcp)
        .await
        .context("TLS el sıkışması başarısız")?;

    let url = format!("wss://{host}:{port}{WS_PATH}");
    let request = url
        .into_client_request()
        .context("WebSocket isteği kurulamadı")?;
    let (ws, _) = tokio_tungstenite::client_async(request, tls)
        .await
        .context("WebSocket el sıkışması başarısız")?;

    let observed = verifier
        .observed()
        .ok_or_else(|| anyhow!("sunucu sertifikası okunamadı"))?;

    let (mut sink, mut source) = ws.split();

    // El sıkışma.
    let teacher_name = tokio::time::timeout(
        HANDSHAKE_TIMEOUT,
        handshake(
            state,
            &mut sink,
            &mut source,
            &observed,
            pairing_code,
            host,
            port,
        ),
    )
    .await
    .map_err(|_| anyhow!("kimlik doğrulama zaman aşımına uğradı"))??;

    state.set_status(ConnectionStatus::Connected { teacher_name });

    // Yazma görevi.
    let (tx, mut rx) = mpsc::channel::<ClientMessage>(SEND_QUEUE);
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

    // Arayüz komutlarının (yardım talebi gibi) bu bağlantıyı kullanabilmesi için.
    *state.outbox.write() = Some(tx.clone());

    let result = session_loop(state, &mut source, &tx, teacher_ip, cancel).await;

    *state.outbox.write() = None;
    drop(tx);
    writer.abort();
    result
}

type WsSink = futures_util::stream::SplitSink<
    tokio_tungstenite::WebSocketStream<tokio_rustls::client::TlsStream<TcpStream>>,
    Message,
>;
type WsSource = futures_util::stream::SplitStream<
    tokio_tungstenite::WebSocketStream<tokio_rustls::client::TlsStream<TcpStream>>,
>;

/// Hello → (eşleştirme | challenge) → yetkilendirildi.
/// Başarılıysa öğretmenin adını döner.
#[allow(clippy::too_many_arguments)]
async fn handshake(
    state: &Arc<AppState>,
    sink: &mut WsSink,
    source: &mut WsSource,
    cert_fingerprint: &[u8; 32],
    pairing_code: Option<String>,
    host: &str,
    port: u16,
) -> Result<String> {
    send(
        sink,
        &ClientMessage::Hello {
            protocol: PROTOCOL_VERSION,
            device_id: state.identity.device_id,
            hostname: state.hostname.clone(),
            app_version: openclass_common::APP_VERSION.to_string(),
            public_key: state.identity.public_key().to_vec(),
        },
    )
    .await?;

    let ServerMessage::HelloAck {
        protocol,
        teacher_id,
        teacher_name,
        status,
    } = recv(source).await?
    else {
        bail!("HelloAck bekleniyordu");
    };

    if protocol != PROTOCOL_VERSION {
        bail!(
            "Protokol sürümü uyuşmuyor: öğretmen konsolu v{protocol}, bu uygulama v{PROTOCOL_VERSION}. \
             Her iki uygulamayı da aynı sürüme güncelleyin."
        );
    }

    match status {
        DeviceStatus::Rejected => {
            bail!("Bu cihazın yetkisi öğretmen tarafından kaldırılmış.")
        }

        DeviceStatus::Authorized => {
            if pairing_code.is_some() {
                tracing::info!("cihaz zaten yetkili, eşleştirme adımı atlanıyor");
            }
            let ServerMessage::AuthChallenge { challenge } = recv(source).await? else {
                bail!("AuthChallenge bekleniyordu");
            };
            let challenge: [u8; 32] = challenge
                .as_slice()
                .try_into()
                .map_err(|_| anyhow!("challenge 32 bayt değil"))?;

            let signature = state
                .identity
                .keys
                .sign_challenge(cert_fingerprint, &challenge);
            send(
                sink,
                &ClientMessage::AuthResponse {
                    signature: signature.to_vec(),
                },
            )
            .await?;

            expect_auth_ok(state, source).await?;
            Ok(teacher_name)
        }

        DeviceStatus::PendingPairing => {
            let Some(code) = pairing_code else {
                bail!(
                    "Bu cihaz öğretmen konsolunda kayıtlı değil. \
                     Öğretmenden eşleştirme kodu isteyip 'Eşleştir' adımını yapın."
                );
            };

            let public_key = state.identity.public_key();
            let proof = pairing_proof(
                &code,
                cert_fingerprint,
                &state.identity.device_id,
                &public_key,
            );
            send(
                sink,
                &ClientMessage::PairRequest {
                    public_key: public_key.to_vec(),
                    proof: proof.to_vec(),
                },
            )
            .await?;

            let ServerMessage::PairResult { accepted, reason } = recv(source).await? else {
                bail!("PairResult bekleniyordu");
            };
            if !accepted {
                bail!(
                    "{}",
                    reason.unwrap_or_else(|| "Eşleştirme reddedildi.".into())
                );
            }

            expect_auth_ok(state, source).await?;

            // Eşleştirme başarılı: öğretmeni ve parmak izini kalıcı olarak sabitle.
            {
                let mut cfg = state.config.write();
                cfg.paired_teacher = Some(PairedTeacher {
                    teacher_id,
                    teacher_name: teacher_name.clone(),
                    host: host.to_string(),
                    port,
                    cert_fingerprint: cert_fingerprint.to_vec(),
                });
            }
            state
                .save_config()
                .context("eşleştirme bilgisi kaydedilemedi")?;
            state.emit("paired", ());
            tracing::info!(%teacher_id, %teacher_name, "öğretmenle eşleşildi");

            Ok(teacher_name)
        }
    }
}

async fn expect_auth_ok(state: &Arc<AppState>, source: &mut WsSource) -> Result<()> {
    let ServerMessage::AuthResult {
        ok,
        reason,
        reset_pairing,
    } = recv(source).await?
    else {
        bail!("AuthResult bekleniyordu");
    };
    if !ok {
        if reset_pairing {
            forget_pairing(state);
        }
        bail!(
            "{}",
            reason.unwrap_or_else(|| "Kimlik doğrulama reddedildi.".into())
        );
    }
    Ok(())
}

/// Yetkilendirilmiş oturumun ana döngüsü.
async fn session_loop(
    state: &Arc<AppState>,
    source: &mut WsSource,
    tx: &mpsc::Sender<ClientMessage>,
    // Öğretmenin adresi; multicast arayüzünü seçmek için.
    teacher_ip: std::net::IpAddr,
    cancel: CancellationToken,
) -> Result<()> {
    // Okuma döngüsünden çıkış birden çok yoldan olur (iptal, okuma hatası,
    // kapanan bağlantı). Muhafız `Drop` ile durdurduğu için hiçbir yol
    // multicast alıcısını arkada çalışır hâlde bırakmaz.
    let mut ctx = SessionCtx {
        stream: None,
        transfer: None,
        multicast: MulticastGuard(None),
        teacher_ip,
    };
    // Yayın yokken tetiklenmemesi için uzun aralıkla başlar.
    let mut frame_timer = tokio::time::interval(Duration::from_secs(3600));
    frame_timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                if let Some(s) = ctx.stream.take() { s.stop(state); }
                return Ok(());
            }

            _ = frame_timer.tick(), if ctx.stream.is_some() => {
                let Some(s) = ctx.stream.as_mut() else { continue };
                match s.capture_and_send(state, tx).await {
                    Ok(()) => {}
                    Err(err) => {
                        let text = format!("{err:#}");
                        tracing::warn!(hata = %text, "ekran karesi gönderilemedi");
                        let _ = tx.try_send(ClientMessage::ScreenError {
                            stream_id: s.stream_id,
                            reason: text.clone(),
                        });
                        state.set_sharing(SharingState {
                            active: false,
                            error: Some(text),
                            ..SharingState::default()
                        });
                        if let Some(s) = ctx.stream.take() { s.stop(state); }
                    }
                }
            }

            incoming = tokio::time::timeout(IDLE_TIMEOUT, source.next()) => {
                let frame = match incoming {
                    Err(_) => bail!("Öğretmen konsolundan {}s boyunca yanıt gelmedi.", IDLE_TIMEOUT.as_secs()),
                    Ok(None) => return Ok(()),
                    Ok(Some(Err(err))) => return Err(err).context("WebSocket okuma hatası"),
                    Ok(Some(Ok(f))) => f,
                };

                let Message::Binary(bytes) = frame else {
                    if matches!(frame, Message::Close(_)) { return Ok(()); }
                    continue;
                };
                let msg: ServerMessage = openclass_protocol::decode(&bytes, MAX_CONTROL_FRAME)
                    .context("gelen mesaj çözülemedi")?;

                handle_server_message(state, msg, tx, &mut ctx, &mut frame_timer).await?;
            }
        }
    }
}

async fn handle_server_message(
    state: &Arc<AppState>,
    msg: ServerMessage,
    tx: &mpsc::Sender<ClientMessage>,
    ctx: &mut SessionCtx,
    frame_timer: &mut tokio::time::Interval,
) -> Result<()> {
    match msg {
        ServerMessage::SessionStart {
            session_id,
            teacher_name,
            class_name,
        } => {
            tracing::info!(%session_id, %class_name, "ders oturumu başladı");
            let session = ActiveSession {
                session_id,
                teacher_name,
                class_name,
                started_at: unix_now(),
            };
            *state.session.write() = Some(session.clone());
            state.emit("session-changed", Some(session));
        }

        ServerMessage::SessionEnd { session_id } => {
            tracing::info!(%session_id, "ders oturumu bitti, yayın durduruluyor");
            // Ders bitince kontrol, kilit ve gezgin de biter; bu bir güvenlik
            // kuralıdır.
            state.stop_remote_control();
            state.unlock_screen();
            state.stop_browsing();
            if let Some(s) = ctx.stream.take() {
                s.stop(state);
            }
            *frame_timer = idle_timer();
            *state.session.write() = None;
            state.emit("session-changed", Option::<ActiveSession>::None);
        }

        ServerMessage::StartScreenStream { stream_id, quality } => {
            // Ders oturumu yoksa yayın başlatılmaz. Öğretmen tarafı da
            // engelliyor; bu ikinci savunma hattıdır.
            if state.session.read().is_none() {
                tracing::warn!("oturum yokken ekran yayını istendi, reddedildi");
                let _ = tx
                    .send(ClientMessage::ScreenError {
                        stream_id,
                        reason: "Aktif ders oturumu olmadan ekran paylaşılmaz.".into(),
                    })
                    .await;
                return Ok(());
            }

            match ScreenStream::start(stream_id, quality, &state.hostname) {
                Ok(s) => {
                    tracing::info!(stream_id, backend = s.backend, "ekran yayını başladı");
                    state.set_sharing(SharingState {
                        active: true,
                        backend: Some(s.backend.to_string()),
                        fps: Some(quality.fps),
                        width: None,
                        height: None,
                        error: None,
                    });
                    *frame_timer = fps_timer(quality.fps);
                    ctx.stream = Some(s);
                }
                Err(err) => {
                    let text = format!("{err:#}");
                    tracing::error!(hata = %text, "ekran yakalama başlatılamadı");
                    let _ = tx
                        .send(ClientMessage::ScreenError {
                            stream_id,
                            reason: text.clone(),
                        })
                        .await;
                    state.set_sharing(SharingState {
                        active: false,
                        error: Some(text),
                        ..SharingState::default()
                    });
                }
            }
        }

        ServerMessage::UpdateStreamQuality { quality, .. } => {
            if let Some(s) = ctx.stream.as_mut() {
                tracing::debug!(fps = quality.fps, "yayın kalitesi değişti");
                s.quality = quality;
                // Kalite/çözünürlük değişince önceki karo özetleri
                // karşılaştırılabilir değildir; sonraki paket anahtar kare olur.
                s.encoder.invalidate();
                *frame_timer = fps_timer(quality.fps);
                let mut sharing = state.sharing.read().clone();
                sharing.fps = Some(quality.fps);
                state.set_sharing(sharing);
            }
        }

        ServerMessage::FileOffer {
            transfer_id,
            name,
            size,
            sha256,
        } => {
            // Yürüyen bir aktarım varsa yenisini kabul etmeyiz: öğretmen
            // dosyaları sırayla gönderir.
            if ctx.transfer.is_some() {
                let _ = tx
                    .send(ClientMessage::FileRejected {
                        transfer_id,
                        reason: "Başka bir dosya aktarımı sürüyor.".into(),
                    })
                    .await;
                return Ok(());
            }

            match crate::transfer::Incoming::open(transfer_id, &name, size, sha256) {
                Ok(incoming) => {
                    tracing::info!(%transfer_id, dosya = %incoming.name, size, "dosya alınıyor");
                    // Öğrenciye her zaman haber verilir; sessiz bırakma yok.
                    state.show_notice(format!(
                        "Öğretmeniniz size bir dosya gönderiyor: {} ({:.1} MB)",
                        incoming.name,
                        size as f64 / 1_048_576.0
                    ));
                    ctx.transfer = Some(incoming);
                    let _ = tx.send(ClientMessage::FileAccepted { transfer_id }).await;
                }
                Err(err) => {
                    tracing::warn!(%transfer_id, %err, "dosya teklifi reddedildi");
                    let _ = tx
                        .send(ClientMessage::FileRejected {
                            transfer_id,
                            reason: err.to_string(),
                        })
                        .await;
                }
            }
        }

        ServerMessage::FileChunk {
            transfer_id,
            seq,
            data,
        } => {
            let Some(incoming) = ctx.transfer.as_mut() else {
                return Ok(());
            };
            if incoming.transfer_id != transfer_id {
                return Ok(());
            }
            if let Err(err) = incoming.write_chunk(seq, &data) {
                tracing::error!(%transfer_id, %err, "dosya parçası yazılamadı");
                let detail = err.to_string();
                if let Some(aborted) = ctx.transfer.take() {
                    aborted.abort();
                }
                let _ = tx
                    .send(ClientMessage::FileDone {
                        transfer_id,
                        ok: false,
                        detail,
                    })
                    .await;
            }
        }

        ServerMessage::FileEnd { transfer_id } => {
            let Some(incoming) = ctx.transfer.take() else {
                return Ok(());
            };
            if incoming.transfer_id != transfer_id {
                incoming.abort();
                return Ok(());
            }

            let name = incoming.name.clone();
            match incoming.finish() {
                Ok(path) => {
                    tracing::info!(%transfer_id, yol = %path.display(), "dosya kaydedildi");
                    state.show_notice(format!(
                        "Dosya kaydedildi: {name}\n\nKonum: {}",
                        path.display()
                    ));
                    let _ = tx
                        .send(ClientMessage::FileDone {
                            transfer_id,
                            ok: true,
                            detail: path.display().to_string(),
                        })
                        .await;
                }
                Err(err) => {
                    tracing::error!(%transfer_id, %err, "dosya doğrulanamadı");
                    state.show_notice(format!("Dosya alınamadı: {name}\n\n{err}"));
                    let _ = tx
                        .send(ClientMessage::FileDone {
                            transfer_id,
                            ok: false,
                            detail: err.to_string(),
                        })
                        .await;
                }
            }
        }

        ServerMessage::FileCancel {
            transfer_id,
            reason,
        } => {
            if let Some(incoming) = ctx.transfer.take() {
                if incoming.transfer_id == transfer_id {
                    tracing::info!(%transfer_id, %reason, "dosya aktarımı iptal edildi");
                    incoming.abort();
                } else {
                    ctx.transfer = Some(incoming);
                }
            }
        }

        ServerMessage::BrowseStart { teacher_name } => {
            if state.session.read().is_none() {
                tracing::warn!("oturum yokken dosya gezgini istendi, reddedildi");
                return Ok(());
            }
            tracing::info!(%teacher_name, "uzak dosya gezgini açıldı");
            state.start_browsing(teacher_name);
        }

        ServerMessage::BrowseEnd => {
            tracing::info!("uzak dosya gezgini kapandı");
            state.stop_browsing();
        }

        ServerMessage::BrowseList { request_id, path } => {
            // Gezgin açık değilse hiçbir şey listelenmez.
            if state.browsing.read().is_none() {
                let _ = tx
                    .send(ClientMessage::BrowseFailed {
                        request_id,
                        reason: "Dosya gezgini açık değil.".into(),
                    })
                    .await;
                return Ok(());
            }

            // Boş yol: izin verilen köklerin listesi.
            if path.trim().is_empty() {
                let _ = tx
                    .send(ClientMessage::BrowseListing {
                        request_id,
                        path: String::new(),
                        roots: crate::browse::root_views(),
                        entries: Vec::new(),
                    })
                    .await;
                return Ok(());
            }

            let result = crate::browse::resolve(&path).and_then(|real| {
                let entries = crate::browse::list(&real)?;
                Ok((real, entries))
            });

            match result {
                Ok((real, entries)) => {
                    tracing::info!(yol = %real.display(), adet = entries.len(), "klasör listelendi");
                    let _ = tx
                        .send(ClientMessage::BrowseListing {
                            request_id,
                            path: real.to_string_lossy().into_owned(),
                            roots: Vec::new(),
                            entries,
                        })
                        .await;
                }
                Err(err) => {
                    tracing::warn!(%path, %err, "klasör listelenemedi");
                    let _ = tx
                        .send(ClientMessage::BrowseFailed {
                            request_id,
                            reason: err.to_string(),
                        })
                        .await;
                }
            }
        }

        ServerMessage::BrowseFetch { request_id, path } => {
            if state.browsing.read().is_none() {
                let _ = tx
                    .send(ClientMessage::BrowseFailed {
                        request_id,
                        reason: "Dosya gezgini açık değil.".into(),
                    })
                    .await;
                return Ok(());
            }

            let prepared = crate::browse::resolve(&path)
                .and_then(|real| crate::browse::check_fetch(&real).map(|size| (real, size)));

            let (real, size) = match prepared {
                Ok(pair) => pair,
                Err(err) => {
                    tracing::warn!(%path, %err, "dosya indirilemedi");
                    let _ = tx
                        .send(ClientMessage::BrowseFailed {
                            request_id,
                            reason: err.to_string(),
                        })
                        .await;
                    return Ok(());
                }
            };

            let name = real
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "dosya".to_string());

            // Her indirme öğrenciye ayrıca bildirilir: dosyasının kopyalandığını
            // bilmeden kalmamalı.
            tracing::info!(yol = %real.display(), size, "dosya öğretmene gönderiliyor");
            state.show_notice(format!(
                "Öğretmeniniz bilgisayarınızdan bir dosya aldı:\n{name}"
            ));

            // Okuma bloke eder; gönderim ayrı bir iş parçacığında yürür ki
            // kontrol kanalı tıkanmasın.
            let sender = tx.clone();
            tokio::task::spawn_blocking(move || {
                if let Err(err) = stream_file(&sender, request_id, &real, size) {
                    tracing::error!(%err, "dosya gönderilemedi");
                    let _ = sender.blocking_send(ClientMessage::BrowseFailed {
                        request_id,
                        reason: err.to_string(),
                    });
                }
            });
        }

        ServerMessage::LockStudent { message } => {
            if state.session.read().is_none() {
                tracing::warn!("oturum yokken kilit istendi, reddedildi");
                return Ok(());
            }
            tracing::info!("ekran kilitlendi");
            state.lock_screen(message);
        }

        ServerMessage::UnlockStudent => {
            tracing::info!("ekran kilidi kaldırıldı");
            state.unlock_screen();
        }

        ServerMessage::TeacherScreenStart {
            broadcast_id,
            teacher_name,
            fullscreen,
            multicast,
        } => {
            tracing::info!(%broadcast_id, %teacher_name, "öğretmen ekranı yayını başladı");
            *state.broadcast.write() = Some(crate::state::ActiveBroadcast {
                broadcast_id,
                teacher_name,
                started_at: unix_now(),
            });
            state.show_broadcast_window(fullscreen);
            state.emit("broadcast-changed", state.broadcast.read().clone());

            // Multicast açıksa gruba katıl. Kurulamazsa öğretmenden TLS
            // yedeği istenir: multicast'in çalışmaması dersi durdurmaz.
            if let Some(info) = multicast {
                let token = CancellationToken::new();
                ctx.multicast.0 = Some(token.clone());
                if let Err(err) = crate::net::multicast::spawn(
                    Arc::clone(state),
                    &info,
                    broadcast_id,
                    ctx.teacher_ip,
                    tx.clone(),
                    token,
                ) {
                    tracing::warn!(
                        error = %format!("{err:#}"),
                        "multicast alıcısı başlatılamadı, TLS yedeği isteniyor"
                    );
                    ctx.multicast.0 = None;
                    let _ = tx
                        .send(ClientMessage::MulticastUnavailable { broadcast_id })
                        .await;
                }
            }
        }

        ServerMessage::TeacherScreenUpdate {
            seq,
            width,
            height,
            keyframe,
            rects,
            ..
        } => {
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

            if needs_keyframe {
                // Eksik parça kaldı; öğretmenden tam kare iste.
                state.teacher_screen.write().clear_keyframe_request();
                let _ = tx.try_send(ClientMessage::RequestTeacherKeyframe);
            }

            state.emit("broadcast-frame", seq);
        }

        ServerMessage::RemoteControlStart { teacher_name } => {
            if !crate::input::is_supported() {
                tracing::warn!("uzaktan kontrol bu platformda desteklenmiyor");
                return Ok(());
            }
            if state.session.read().is_none() {
                tracing::warn!("oturum yokken uzaktan kontrol istendi, reddedildi");
                return Ok(());
            }
            tracing::info!(%teacher_name, "uzaktan kontrol devralındı");
            state.start_remote_control(teacher_name);
        }

        ServerMessage::RemoteControlStop => {
            tracing::info!("uzaktan kontrol bırakıldı");
            state.stop_remote_control();
        }

        ServerMessage::RemoteInput { events } => {
            // Kontrol açık değilse girdi uygulanmaz. Bu ikinci savunma
            // hattıdır; öğretmen tarafı da kontrol almadan göndermez.
            if state.remote_control.read().is_none() {
                // Uyarı seviyesinde: kontrolün çalışmamasının en olası
                // sebeplerinden biri bu ve sessiz kalırsa fark edilmez.
                tracing::warn!(
                    olay = events.len(),
                    "kontrol açık değilken girdi geldi, yok sayıldı"
                );
                return Ok(());
            }
            crate::input::apply(&events);
        }

        ServerMessage::TeacherScreenStop => {
            // Multicast alıcısı yayınla birlikte kapanır; oturum anahtarı
            // da onunla düşer.
            if let Some(token) = ctx.multicast.0.take() {
                token.cancel();
            }
            tracing::info!("öğretmen ekranı yayını bitti");
            *state.broadcast.write() = None;
            *state.teacher_screen.write() = openclass_screen::ScreenBuffer::new();
            state.close_broadcast_window();
            state.emit(
                "broadcast-changed",
                Option::<crate::state::ActiveBroadcast>::None,
            );
        }

        ServerMessage::RequestKeyframe { .. } => {
            if let Some(s) = ctx.stream.as_mut() {
                tracing::debug!("öğretmen anahtar kare istedi");
                s.encoder.invalidate();
            }
        }

        ServerMessage::StopScreenStream { .. } => {
            if let Some(s) = ctx.stream.take() {
                tracing::info!("ekran yayını durduruldu");
                s.stop(state);
            }
            *frame_timer = idle_timer();
        }

        ServerMessage::Notice {
            message_id,
            title,
            body,
        } => {
            tracing::info!(%message_id, "öğretmenden mesaj geldi");
            state.push_notice(ReceivedNotice {
                message_id,
                title,
                body,
                received_at: unix_now(),
            });
            let _ = tx.send(ClientMessage::MessageAck { message_id }).await;
        }

        ServerMessage::HelpAcknowledged => {
            *state.help_requested.write() = false;
            state.emit("help-acknowledged", ());
        }

        ServerMessage::Ping { token } => {
            let _ = tx.send(ClientMessage::Pong { token }).await;
        }

        ServerMessage::AuthResult {
            ok: false,
            reason,
            reset_pairing,
        } => {
            if reset_pairing {
                forget_pairing(state);
            }
            bail!("{}", reason.unwrap_or_else(|| "Yetki kaldırıldı.".into()));
        }

        // El sıkışmaya ait mesajlar oturum ortasında beklenmez.
        ServerMessage::HelloAck { .. }
        | ServerMessage::PairResult { .. }
        | ServerMessage::AuthChallenge { .. }
        | ServerMessage::AuthResult { ok: true, .. } => {
            tracing::warn!("el sıkışma mesajı oturum ortasında geldi, yok sayıldı");
        }
    }
    Ok(())
}

fn fps_timer(fps: u8) -> tokio::time::Interval {
    let period = Duration::from_millis(1000 / u64::from(fps.max(1)));
    let mut timer = tokio::time::interval(period);
    timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    timer
}

fn idle_timer() -> tokio::time::Interval {
    let mut timer = tokio::time::interval(Duration::from_secs(3600));
    timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    timer
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Anahtar kare aralığı.
///
/// Sonradan bağlanan istemcinin toparlanması, paket kaybında kendini
/// düzeltme ve özet çakışmasıyla atlanmış bir değişikliğin kapanması için
/// belirli aralıklarla tüm kare gönderilir.
const KEYFRAME_INTERVAL: Duration = Duration::from_secs(3);

/// Yayın başladıktan sonra bu süre boyunca hiç kare yakalanmazsa uyarılır.
const STALL_WARNING: Duration = Duration::from_secs(5);

/// Çalışan bir ekran yayını.
struct ScreenStream {
    stream_id: u32,
    quality: StreamQuality,
    session: CaptureSession,
    backend: &'static str,
    seq: u64,
    /// Kareler arası karo durumunu taşır.
    encoder: TileEncoder,
    last_keyframe: Instant,
    /// Tanılama: yakalanan ham kare ve gönderilen güncelleme sayısı.
    ///
    /// Yayın "başladı" deyip hiçbir şey göndermediğinde sorunun yakalamada
    /// mı yoksa aktarımda mı olduğunu ayırt etmek için gerekli.
    captured: u64,
    started_at: Instant,
    stall_warned: bool,
    /// Verim sınırı penceresi.
    window_start: Instant,
    window_bytes: usize,
}

impl ScreenStream {
    fn start(stream_id: u32, quality: StreamQuality, hostname: &str) -> Result<Self> {
        let session = CaptureSession::start_primary_monitor(hostname)?;
        let backend = session.backend_name();
        let now = Instant::now();
        Ok(Self {
            stream_id,
            quality,
            session,
            backend,
            seq: 0,
            encoder: TileEncoder::new(),
            last_keyframe: now,
            captured: 0,
            started_at: now,
            stall_warned: false,
            window_start: now,
            window_bytes: 0,
        })
    }

    /// Saniyelik verim bütçesi doldu mu?
    ///
    /// Dolduysa kare atlanır. Hareketli içerikte ağı boğmak yerine kare
    /// hızını düşürmek, görüntünün akmaya devam etmesini sağlar.
    fn budget_exceeded(&mut self, now: Instant) -> bool {
        if now.duration_since(self.window_start) >= Duration::from_secs(1) {
            self.window_start = now;
            self.window_bytes = 0;
        }
        self.window_bytes >= (self.quality.max_kbps as usize * 1000) / 8
    }

    /// En son kareyi alır, değişen bölgeleri kodlar ve gönderir.
    async fn capture_and_send(
        &mut self,
        state: &Arc<AppState>,
        tx: &mpsc::Sender<ClientMessage>,
    ) -> Result<()> {
        if !self.session.is_running() {
            bail!("Ekran yakalama beklenmedik şekilde durdu.");
        }

        let Some(raw) = self.session.take_latest() else {
            // Yeni kare yok. Durağan ekranda bu normaldir: Windows Graphics
            // Capture yalnızca içerik değiştiğinde kare üretir.
            //
            // Ancak yayın başladıktan sonra hiç kare gelmediyse sorun vardır
            // ve bu, "başladı" deyip susan bir akıştan ayırt edilemez.
            // Bir kez uyarılır.
            if self.captured == 0
                && !self.stall_warned
                && self.started_at.elapsed() >= STALL_WARNING
            {
                self.stall_warned = true;
                tracing::warn!(
                    stream_id = self.stream_id,
                    backend = self.backend,
                    saniye = STALL_WARNING.as_secs(),
                    "yayın başladı ama hiç kare yakalanmadı — ekran tamamen \
                     durağan olabilir, oturum kilitli olabilir veya yakalama \
                     arka ucu kare üretmiyor olabilir"
                );
            }
            return Ok(());
        };

        if self.captured == 0 {
            tracing::info!(
                stream_id = self.stream_id,
                backend = self.backend,
                genislik = raw.width,
                yukseklik = raw.height,
                "ilk ham kare yakalandı"
            );
        }
        self.captured += 1;

        let now = Instant::now();
        if self.budget_exceeded(now) {
            tracing::trace!(stream_id = self.stream_id, "verim sınırı, kare atlandı");
            return Ok(());
        }

        let force_keyframe = now.duration_since(self.last_keyframe) >= KEYFRAME_INTERVAL;
        let quality = self.quality;

        // Kodlama CPU yoğundur; async çalıştırıcıyı bloklamamak için ayrı bir
        // iş parçacığına verilir. Kodlayıcı kareler arası durum taşıdığı için
        // sahipliği geçici olarak devredilir ve geri alınır.
        let mut encoder = std::mem::take(&mut self.encoder);
        let (encoder, encoded) = tokio::task::spawn_blocking(move || {
            let result = encoder.encode(
                &raw,
                quality.max_width,
                quality.max_height,
                quality.jpeg_quality,
                force_keyframe,
            );
            (encoder, result)
        })
        .await
        .context("kodlama görevi başarısız")?;
        self.encoder = encoder;

        let Some(update) = encoded? else {
            // Hiçbir bölge değişmemiş; paket üretilmez.
            return Ok(());
        };

        if self.seq == 0 {
            tracing::info!(
                stream_id = self.stream_id,
                genislik = update.width,
                yukseklik = update.height,
                bolge = update.rects.len(),
                bayt = update.bytes,
                "ilk ekran güncellemesi gönderiliyor"
            );
        }

        if update.keyframe {
            self.last_keyframe = now;
        }
        self.window_bytes += update.bytes;
        self.seq += 1;

        let message = ClientMessage::ScreenUpdate {
            stream_id: self.stream_id,
            seq: self.seq,
            width: update.width,
            height: update.height,
            keyframe: update.keyframe,
            codec: FrameCodec::Jpeg,
            rects: update
                .rects
                .into_iter()
                .map(|r| ScreenRect {
                    x: r.x,
                    y: r.y,
                    width: r.width,
                    height: r.height,
                    data: r.jpeg,
                })
                .collect(),
        };

        // Kuyruk doluysa kare düşürülür: yavaş bağlantıda bellek birikmez.
        // Düşen kare karo durumunu bozar, bu yüzden kodlayıcı geçersizlenir
        // ve bir sonraki paket anahtar kare olur.
        match tx.try_send(message) {
            Ok(()) => {}
            Err(mpsc::error::TrySendError::Full(_)) => {
                tracing::debug!(seq = self.seq, "giden kuyruk dolu, kare düşürüldü");
                self.encoder.invalidate();
                return Ok(());
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                bail!("bağlantı kapandı");
            }
        }

        // Arayüzdeki "paylaşılıyor" göstergesini güncel tut.
        let mut sharing = state.sharing.read().clone();
        if sharing.width != Some(update.width) || sharing.height != Some(update.height) {
            sharing.width = Some(update.width);
            sharing.height = Some(update.height);
            state.set_sharing(sharing);
        }
        Ok(())
    }

    fn stop(self, state: &Arc<AppState>) {
        self.session.stop();
        state.set_sharing(SharingState {
            active: false,
            ..SharingState::default()
        });
    }
}

async fn send(sink: &mut WsSink, msg: &ClientMessage) -> Result<()> {
    let bytes = openclass_protocol::encode(msg, MAX_CONTROL_FRAME)?;
    sink.send(Message::Binary(bytes.into()))
        .await
        .context("mesaj gönderilemedi")?;
    Ok(())
}

async fn recv(source: &mut WsSource) -> Result<ServerMessage> {
    loop {
        match source.next().await {
            None => bail!("Bağlantı beklenmedik şekilde kapandı."),
            Some(Err(err)) => return Err(err).context("WebSocket okuma hatası"),
            Some(Ok(Message::Binary(bytes))) => {
                return openclass_protocol::decode(&bytes, MAX_CONTROL_FRAME)
                    .context("gelen mesaj çözülemedi")
            }
            Some(Ok(Message::Close(_))) => bail!("Öğretmen konsolu bağlantıyı kapattı."),
            Some(Ok(_)) => continue,
        }
    }
}

/// Bir dosyayı parçalara bölüp öğretmene gönderir.
///
/// Özet önce hesaplanır (dosya iki kez okunur): alıcı bütünlüğü bununla
/// doğrular ve 512 MB'lık bir dosyayı belleğe almak laboratuvar makinesinde
/// kabul edilemez.
fn stream_file(
    tx: &mpsc::Sender<ClientMessage>,
    request_id: uuid::Uuid,
    path: &std::path::Path,
    size: u64,
) -> Result<()> {
    use sha2::{Digest, Sha256};
    use std::io::Read;

    /// Parça boyutu; gönderici tarafıyla aynı.
    const CHUNK: usize = 64 * 1024;

    let mut hasher = Sha256::new();
    let mut file = std::fs::File::open(path).context("dosya açılamadı")?;
    let mut buf = vec![0u8; CHUNK];
    loop {
        let read = file.read(&mut buf).context("dosya okunamadı")?;
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
    }
    let sha256: [u8; 32] = hasher.finalize().into();

    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "dosya".to_string());

    tx.blocking_send(ClientMessage::BrowseFileStart {
        request_id,
        name,
        size,
        sha256,
    })
    .map_err(|_| anyhow::anyhow!("bağlantı kapandı"))?;

    let mut file = std::fs::File::open(path).context("dosya açılamadı")?;
    let mut seq: u32 = 0;
    loop {
        let read = file.read(&mut buf).context("dosya okunamadı")?;
        if read == 0 {
            break;
        }
        tx.blocking_send(ClientMessage::BrowseChunk {
            request_id,
            seq,
            data: buf[..read].to_vec(),
        })
        .map_err(|_| anyhow::anyhow!("bağlantı kapandı"))?;
        seq = seq.wrapping_add(1);
    }

    tx.blocking_send(ClientMessage::BrowseFileEnd { request_id })
        .map_err(|_| anyhow::anyhow!("bağlantı kapandı"))?;
    Ok(())
}

/// Oturum boyunca yaşayan değiştirilebilir durum.
///
/// Ekran akışı, dosya aktarımı ve multicast alıcısı tek bir bağlamda
/// toplanır; bunları mesaj işleyiciye ayrı ayrı parametre olarak taşımak
/// imzayı okunmaz hâle getiriyordu.
struct SessionCtx {
    /// Çalışan ekran paylaşımı.
    stream: Option<ScreenStream>,
    /// Aynı anda tek dosya alınır; aktarım bağlantı boyunca yaşar.
    transfer: Option<crate::transfer::Incoming>,
    /// Multicast alıcısı; muhafız `Drop` ile durdurur.
    multicast: MulticastGuard,
    /// Öğretmenin adresi; multicast arayüzünü seçmek için.
    teacher_ip: std::net::IpAddr,
}

/// Multicast alıcısını kapsam dışına çıkınca durduran muhafız.
///
/// Alıcı ayrı bir iş parçacığında çalışır; durdurulmazsa yayın bittikten
/// sonra da soketi tutmaya devam ederdi.
struct MulticastGuard(Option<CancellationToken>);

impl Drop for MulticastGuard {
    fn drop(&mut self) {
        if let Some(token) = self.0.take() {
            token.cancel();
        }
    }
}

/// Kayıtlı eşleştirmeyi siler ve "eşleştirilmedi" durumuna döner.
///
/// Öğretmen cihazın yetkisini kaldırdığında çağrılır. Eşleştirme
/// silinmezse öğrenci geçersiz kimliğiyle sonsuza kadar yeniden bağlanmayı
/// dener ve yapılandırma penceresi yeni kod istemez — kullanıcı elle
/// "Eşleştirmeyi Kaldır" demeden kurtulamaz.
fn forget_pairing(state: &Arc<AppState>) {
    tracing::warn!("öğretmen yetkiyi kaldırdı, kayıtlı eşleştirme siliniyor");
    if let Err(err) = state.clear_pairing() {
        tracing::error!(error = %format!("{err:#}"), "eşleştirme silinemedi");
    }
}
