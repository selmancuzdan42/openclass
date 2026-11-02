//! Keşif cevaplayıcısı — öğrenci tarafı.
//!
//! Öğrenci ajanı UDP keşif portunu **dinler** ve yalnızca öğretmen probe'una
//! cevap verir. Kendiliğinden yayın yapmaz: öğretmen konsolu kapalıyken ağda
//! hiç OpenClass trafiği olmaz.
//!
//! Cevap unicast gönderilir, böylece öğretmen cihazın gerçek IP'sini öğrenir.
//!
//! # Burada yetki verilmez
//!
//! Bu katman yalnızca "ben LAB101'im, buradayım" der. Cihazın yönetilebilir
//! olması için [`crate::net::client`] üzerinden kimlik doğrulamasından
//! geçmesi gerekir.

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;

use anyhow::{Context, Result};
use openclass_protocol::auth::fingerprint_to_hex;
use openclass_protocol::{
    discovery::DiscoveryPacket, DEFAULT_DISCOVERY_PORT, MAX_DISCOVERY_PACKET, PROTOCOL_VERSION,
};
use tokio::net::UdpSocket;
use tokio_util::sync::CancellationToken;

use crate::state::{AppState, DiscoveredTeacher};

/// Keşif portunu geçersiz kılan geliştirme değişkeni.
///
/// Yalnızca test içindir: aynı makinede ikinci bir öğrenci ajanı
/// çalıştırabilmek için `OPENCLASS_DISCOVERY_PORT=7645` verilebilir.
/// Öğretmen tarafında bu adres `127.0.0.1:7645` biçiminde ek keşif hedefi
/// olarak girilir.
pub const DISCOVERY_PORT_OVERRIDE_VAR: &str = "OPENCLASS_DISCOVERY_PORT";

fn discovery_port() -> u16 {
    match std::env::var(DISCOVERY_PORT_OVERRIDE_VAR)
        .ok()
        .and_then(|v| v.trim().parse().ok())
    {
        Some(port) => {
            tracing::warn!(
                port,
                "keşif portu ortam değişkeniyle değiştirildi — test modu"
            );
            port
        }
        None => DEFAULT_DISCOVERY_PORT,
    }
}

/// Keşif cevaplayıcısını çalıştırır.
pub async fn run(state: Arc<AppState>, cancel: CancellationToken) -> Result<()> {
    let port = discovery_port();
    let socket = UdpSocket::bind(SocketAddr::from((Ipv4Addr::UNSPECIFIED, port)))
        .await
        .with_context(|| {
            format!(
                "{port} numaralı UDP portu dinlenemedi — \
                 OpenClass Student zaten çalışıyor olabilir veya \
                 Windows Güvenlik Duvarı engelliyor olabilir"
            )
        })?;

    tracing::info!(port, "keşif cevaplayıcısı dinlemede");

    let mut buf = vec![0u8; MAX_DISCOVERY_PACKET];
    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                tracing::info!("keşif cevaplayıcısı durduruldu");
                return Ok(());
            }

            received = socket.recv_from(&mut buf) => {
                let (len, from) = match received {
                    Ok(pair) => pair,
                    Err(err) => {
                        tracing::warn!(%err, "keşif datagramı alınamadı");
                        continue;
                    }
                };
                if let Err(err) = handle(&state, &socket, &buf[..len], from).await {
                    tracing::debug!(%from, error = %format!("{err:#}"), "keşif paketi işlenemedi");
                }
            }
        }
    }
}

async fn handle(
    state: &Arc<AppState>,
    socket: &UdpSocket,
    data: &[u8],
    from: SocketAddr,
) -> Result<()> {
    let Some(parsed) = DiscoveryPacket::decode(data) else {
        return Ok(()); // bizim trafiğimiz değil
    };
    let packet = parsed?;

    let DiscoveryPacket::TeacherProbe {
        protocol,
        teacher_id,
        teacher_name,
        control_port,
        cert_fingerprint,
        nonce,
    } = packet
    else {
        // Başka bir öğrencinin duyurusu; bizi ilgilendirmez.
        return Ok(());
    };

    if protocol != PROTOCOL_VERSION {
        tracing::debug!(%from, karsi_taraf = protocol, biz = PROTOCOL_VERSION, "keşifte sürüm uyuşmazlığı");
        return Ok(());
    }

    // Öğretmeni listeye ekle: arayüz eşleştirme için bunu gösterir.
    let fingerprint_hex = match <[u8; 32]>::try_from(cert_fingerprint.as_slice()) {
        Ok(fp) => fingerprint_to_hex(&fp),
        Err(_) => {
            tracing::debug!(%from, "öğretmen parmak izi 32 bayt değil, paket yok sayıldı");
            return Ok(());
        }
    };

    let teacher = DiscoveredTeacher {
        teacher_id,
        teacher_name,
        host: from.ip().to_string(),
        control_port,
        fingerprint_hex,
        fingerprint: cert_fingerprint,
        last_seen: unix_now(),
    };
    let changed = {
        let mut guard = state.teachers.write();
        let previous = guard.insert(teacher_id, teacher.clone());
        previous
            .map(|p| p.host != teacher.host || p.control_port != teacher.control_port)
            .unwrap_or(true)
    };
    if changed {
        tracing::info!(%teacher_id, host = %teacher.host, "öğretmen konsolu görüldü");
        state.emit("teachers-changed", ());
        // Eşleşilmiş öğretmenin adresi değiştiyse bağlantıyı hemen tazele.
        if state
            .config
            .read()
            .paired_teacher
            .as_ref()
            .map(|p| p.teacher_id)
            == Some(teacher_id)
        {
            state.wake.notify_waiters();
        }
    }

    // Unicast cevap.
    let reply = DiscoveryPacket::StudentAnnounce {
        protocol: PROTOCOL_VERSION,
        device_id: state.identity.device_id,
        hostname: state.hostname.clone(),
        app_version: openclass_common::APP_VERSION.to_string(),
        nonce,
        paired_to: state
            .config
            .read()
            .paired_teacher
            .as_ref()
            .map(|p| p.teacher_id),
    };
    socket
        .send_to(&reply.encode()?, from)
        .await
        .context("keşif cevabı gönderilemedi")?;
    Ok(())
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
