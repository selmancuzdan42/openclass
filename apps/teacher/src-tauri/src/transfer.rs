//! Öğrencilere dosya gönderme.
//!
//! # Tasarım
//!
//! Dosya diskten **bir kez** okunur ve her parça bağlı tüm alıcılara
//! gönderilir: beş öğrenciye gönderirken dosyayı beş kez okumak gereksiz.
//!
//! # Akış
//!
//! 1. Özet (SHA-256) hesaplanır — alıcı bütünlüğü bununla doğrular.
//! 2. Her hedefe [`ServerMessage::FileOffer`] gönderilir.
//! 3. Kabul/ret cevapları beklenir ([`OFFER_TIMEOUT`]). Reddeden veya
//!    cevap vermeyen cihaza tek bayt gönderilmez.
//! 4. Parçalar sırayla kabul edenlere yazılır.
//! 5. [`ServerMessage::FileEnd`] ile bitirilir; alıcı özeti doğrular.
//!
//! # Denetim
//!
//! Her gönderim denetim kaydına yazılır: kime, hangi dosya, sonuç ne oldu.

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use openclass_protocol::ServerMessage;
use serde::Serialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::db::AuditKind;
use crate::state::AppState;

/// Bir parçanın boyutu.
///
/// 64 KiB, WebSocket çerçeve başlığı yükünü önemsiz kılacak kadar büyük,
/// ekran karelerinin arasına sıkışacak kadar küçük.
const CHUNK_SIZE: usize = 64 * 1024;

/// Teklifin kabul veya ret cevabı için beklenecek süre.
const OFFER_TIMEOUT: Duration = Duration::from_secs(15);

/// Öğretmen tarafının kabul ettiği en büyük dosya.
///
/// Öğrenci tarafındaki sınırla aynı; daha büyüğünü göndermeye çalışmak
/// baştan boşa iş olurdu.
pub const MAX_FILE_SIZE: u64 = 512 * 1024 * 1024;

/// Tek bir alıcının durumu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum TargetState {
    /// Teklif gönderildi, cevap bekleniyor.
    Offered,
    /// Kabul edildi, parçalar gönderiliyor.
    Sending { sent: u64 },
    /// Öğrenci reddetti (yer yok, boyut aşıldı…).
    Rejected { reason: String },
    /// Dosya alındı ve özeti doğrulandı.
    Done { path: String },
    /// Aktarım başarısız oldu.
    Failed { reason: String },
}

/// Yürüyen veya bitmiş bir gönderimin tamamı.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferView {
    pub transfer_id: String,
    pub name: String,
    pub size: u64,
    /// Cihaz kimliği → durum.
    pub targets: HashMap<String, TargetState>,
    pub finished: bool,
}

/// Gönderimi başlatır; arka planda yürür.
///
/// Dosyanın okunabilir ve sınırlar içinde olduğu burada doğrulanır, böylece
/// öğretmen hatayı anında görür.
pub fn send(
    state: Arc<AppState>,
    path: PathBuf,
    targets: Vec<Uuid>,
) -> Result<(Uuid, String, u64)> {
    let meta =
        std::fs::metadata(&path).with_context(|| format!("dosya okunamadı: {}", path.display()))?;
    if !meta.is_file() {
        bail!("Yalnızca dosya gönderilebilir, klasör gönderilemez.");
    }
    let size = meta.len();
    if size == 0 {
        bail!("Boş dosya gönderilemez.");
    }
    if size > MAX_FILE_SIZE {
        bail!(
            "Dosya çok büyük ({:.1} MB); en fazla {} MB gönderilebilir.",
            size as f64 / 1_048_576.0,
            MAX_FILE_SIZE / 1_048_576
        );
    }

    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "dosya".to_string());

    let sha256 = hash_file(&path)?;
    let transfer_id = Uuid::new_v4();

    // Durum kaydı önce oluşturulur; arayüz gönderim başlamadan listeyi görür.
    {
        let mut all = state.transfers.write();
        all.insert(
            transfer_id,
            TransferView {
                transfer_id: transfer_id.to_string(),
                name: name.clone(),
                size,
                targets: targets
                    .iter()
                    .map(|id| (id.to_string(), TargetState::Offered))
                    .collect(),
                finished: false,
            },
        );
    }
    state.emit_transfers();

    tauri::async_runtime::spawn(run(
        Arc::clone(&state),
        transfer_id,
        path,
        name.clone(),
        size,
        sha256,
        targets,
    ));

    Ok((transfer_id, name, size))
}

/// Dosyanın SHA-256 özeti.
///
/// Parça parça okunur: 500 MB'lık bir dosyayı belleğe almak laboratuvar
/// makinesinde kabul edilemez.
fn hash_file(path: &Path) -> Result<[u8; 32]> {
    let mut file = std::fs::File::open(path)
        .with_context(|| format!("dosya açılamadı: {}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; CHUNK_SIZE];
    loop {
        let read = file.read(&mut buf).context("dosya okunamadı")?;
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
    }
    Ok(hasher.finalize().into())
}

#[allow(clippy::too_many_arguments)]
async fn run(
    state: Arc<AppState>,
    transfer_id: Uuid,
    path: PathBuf,
    name: String,
    size: u64,
    sha256: [u8; 32],
    targets: Vec<Uuid>,
) {
    tracing::info!(%transfer_id, %name, size, hedef = targets.len(), "dosya gönderimi başlıyor");

    // 1. Teklifleri gönder.
    for id in &targets {
        let Some(tx) = state.registry.sender(*id) else {
            set_target(
                &state,
                transfer_id,
                *id,
                TargetState::Failed {
                    reason: "Cihaz bağlı değil.".into(),
                },
            );
            continue;
        };
        if tx
            .send(ServerMessage::FileOffer {
                transfer_id,
                name: name.clone(),
                size,
                sha256,
            })
            .await
            .is_err()
        {
            set_target(
                &state,
                transfer_id,
                *id,
                TargetState::Failed {
                    reason: "Bağlantı kapandı.".into(),
                },
            );
        }
    }

    // 2. Cevapları bekle. Durum haritası, cevapları işleyen bağlantı
    //    görevleri tarafından güncellenir; burada kısa aralıklarla
    //    yoklanır — kilitlenme riski olmayan en basit yol.
    let deadline = tokio::time::Instant::now() + OFFER_TIMEOUT;
    loop {
        let pending = state
            .transfers
            .read()
            .get(&transfer_id)
            .map(|t| {
                t.targets
                    .values()
                    .filter(|s| matches!(s, TargetState::Offered))
                    .count()
            })
            .unwrap_or(0);
        if pending == 0 || tokio::time::Instant::now() >= deadline {
            if pending > 0 {
                tracing::warn!(%transfer_id, pending, "bazı cihazlar teklife cevap vermedi");
            }
            break;
        }
        tokio::time::sleep(Duration::from_millis(150)).await;
    }

    // Cevap vermeyenler gönderimin dışında kalır.
    let accepted: Vec<Uuid> = {
        let mut all = state.transfers.write();
        let Some(view) = all.get_mut(&transfer_id) else {
            return;
        };
        let mut accepted = Vec::new();
        for (raw, st) in view.targets.iter_mut() {
            match st {
                TargetState::Sending { .. } => {
                    if let Ok(id) = raw.parse() {
                        accepted.push(id);
                    }
                }
                TargetState::Offered => {
                    *st = TargetState::Failed {
                        reason: "Cihaz cevap vermedi.".into(),
                    };
                }
                _ => {}
            }
        }
        accepted
    };
    state.emit_transfers();

    if accepted.is_empty() {
        tracing::warn!(%transfer_id, "hiçbir cihaz dosyayı kabul etmedi");
        finish(&state, transfer_id);
        return;
    }

    // 3. Parçaları gönder.
    let Ok(mut file) = std::fs::File::open(&path) else {
        cancel_all(&state, transfer_id, &accepted, "Dosya açılamadı.").await;
        finish(&state, transfer_id);
        return;
    };

    let mut buf = vec![0u8; CHUNK_SIZE];
    let mut seq: u32 = 0;
    let mut live: Vec<Uuid> = accepted;
    let mut sent_bytes: u64 = 0;

    loop {
        let read = match file.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(err) => {
                tracing::error!(%transfer_id, %err, "dosya okuma hatası");
                cancel_all(&state, transfer_id, &live, "Dosya okunamadı.").await;
                finish(&state, transfer_id);
                return;
            }
        };

        let chunk = buf[..read].to_vec();
        // Kuyruğu tıkanan cihaz gönderimden düşer; diğerleri beklemez.
        let mut still_live = Vec::with_capacity(live.len());
        for id in &live {
            let Some(tx) = state.registry.sender(*id) else {
                set_target(
                    &state,
                    transfer_id,
                    *id,
                    TargetState::Failed {
                        reason: "Bağlantı kapandı.".into(),
                    },
                );
                continue;
            };
            if tx
                .send(ServerMessage::FileChunk {
                    transfer_id,
                    seq,
                    data: chunk.clone(),
                })
                .await
                .is_ok()
            {
                still_live.push(*id);
            } else {
                set_target(
                    &state,
                    transfer_id,
                    *id,
                    TargetState::Failed {
                        reason: "Bağlantı kapandı.".into(),
                    },
                );
            }
        }
        live = still_live;
        if live.is_empty() {
            break;
        }

        seq = seq.wrapping_add(1);
        sent_bytes += read as u64;
        for id in &live {
            set_target(
                &state,
                transfer_id,
                *id,
                TargetState::Sending { sent: sent_bytes },
            );
        }
    }

    // 4. Bitir. Alıcı özeti doğrular ve `FileDone` ile sonucu bildirir.
    for id in &live {
        if let Some(tx) = state.registry.sender(*id) {
            let _ = tx.send(ServerMessage::FileEnd { transfer_id }).await;
        }
        state.db.audit(AuditKind::FileSent, Some(*id), Some(&name));
    }

    tracing::info!(%transfer_id, alici = live.len(), "dosya gönderimi tamamlandı");
    finish(&state, transfer_id);
}

async fn cancel_all(state: &Arc<AppState>, transfer_id: Uuid, targets: &[Uuid], reason: &str) {
    for id in targets {
        if let Some(tx) = state.registry.sender(*id) {
            let _ = tx
                .send(ServerMessage::FileCancel {
                    transfer_id,
                    reason: reason.to_string(),
                })
                .await;
        }
        set_target(
            state,
            transfer_id,
            *id,
            TargetState::Failed {
                reason: reason.to_string(),
            },
        );
    }
}

/// Bir alıcının durumunu günceller ve arayüze bildirir.
pub fn set_target(state: &Arc<AppState>, transfer_id: Uuid, device_id: Uuid, st: TargetState) {
    {
        let mut all = state.transfers.write();
        let Some(view) = all.get_mut(&transfer_id) else {
            return;
        };
        view.targets.insert(device_id.to_string(), st);
    }
    state.emit_transfers();
}

/// Gönderimi bitmiş olarak işaretler.
fn finish(state: &Arc<AppState>, transfer_id: Uuid) {
    {
        let mut all = state.transfers.write();
        if let Some(view) = all.get_mut(&transfer_id) {
            view.finished = true;
        }
    }
    state.emit_transfers();
}
