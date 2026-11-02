//! Geliştirme ve test kolaylıkları.
//!
//! Bu modüldeki her şey **yalnızca hata ayıklama derlemesinde** çalışır.
//! `cfg!(debug_assertions)` kontrolü sürüm derlemesinde tüm dalları kapatır;
//! yayımlanan `.exe` bu davranışların hiçbirini içermez.
//!
//! Amaç, yerel testte tıklama zincirini ortadan kaldırmaktır: eşleştirme
//! kodunu elle girmek, dersi başlatmak ve her cihazın ekranını tek tek açmak
//! yerine tek komutla uçtan uca akış elde edilir.

use std::sync::Arc;
use std::time::Duration;

use openclass_protocol::control::{ServerMessage, StreamQuality};

use crate::state::{ActivePairing, AppState};

/// Sabit eşleştirme kodunu belirleyen değişken.
pub const PAIRING_CODE_VAR: &str = "OPENCLASS_TEST_PAIRING_CODE";

/// Dersi ve ekran yayınlarını kendiliğinden başlatan değişken.
pub const AUTOSTART_VAR: &str = "OPENCLASS_TEST_AUTOSTART";

/// Öğretmen ekranı yayınını da kendiliğinden başlatan değişken.
pub const BROADCAST_VAR: &str = "OPENCLASS_TEST_BROADCAST";

/// Yapılandırılmış sabit eşleştirme kodu.
///
/// Sürüm derlemesinde her zaman `None` döner.
pub fn fixed_pairing_code() -> Option<String> {
    if !cfg!(debug_assertions) {
        return None;
    }
    let code = std::env::var(PAIRING_CODE_VAR).ok()?;
    let code = code.trim().to_string();
    if code.is_empty() {
        return None;
    }
    tracing::warn!(
        "TEST MODU: sabit eşleştirme kodu kullanılıyor ({PAIRING_CODE_VAR}). \
         Bu yalnızca hata ayıklama derlemesinde çalışır."
    );
    Some(code)
}

/// Sabit kod ayarlıysa eşleştirmeyi sürekli açık tutar.
///
/// Normalde kod tek kullanımlıktır ve 5 dakikada bir biter; testte her
/// öğrenci için yeniden kod üretmek zorunda kalmamak için yeniden kurulur.
pub fn rearm_pairing(state: &AppState) {
    let Some(code) = state.test_pairing_code.clone() else {
        return;
    };
    *state.pairing.write() = Some(ActivePairing::new(code));
}

/// Ders oturumunu ve ekran yayınlarını kendiliğinden başlatan görev.
///
/// Bağlanan her yetkili cihaz için önizleme yayını açar. Sürüm derlemesinde
/// hiç başlatılmaz.
pub fn spawn_autostart(state: Arc<AppState>) {
    if !cfg!(debug_assertions) {
        return;
    }
    if std::env::var(AUTOSTART_VAR)
        .map(|v| v != "1")
        .unwrap_or(true)
    {
        return;
    }

    tracing::warn!(
        "TEST MODU: ders ve ekran yayınları kendiliğinden başlatılacak ({AUTOSTART_VAR})."
    );

    let cancel = state.cancel.clone();
    tauri::async_runtime::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(3));
        loop {
            tokio::select! {
                _ = cancel.cancelled() => return,
                _ = ticker.tick() => {}
            }

            // Oturum yoksa aç.
            if !state.session_active() {
                let class_name = state.config.read().class_name.clone();
                let session = crate::state::ClassSession {
                    session_id: uuid::Uuid::new_v4(),
                    class_name: class_name.clone(),
                    started_at: crate::db::now(),
                };
                if let Err(err) = state.db.start_session(session.session_id, &class_name) {
                    tracing::error!(%err, "test oturumu kaydedilemedi");
                    continue;
                }
                *state.session.write() = Some(session.clone());
                let teacher_name = state.config.read().teacher_name.clone();

                for (_, tx) in state.registry.all_senders() {
                    let _ = tx
                        .send(ServerMessage::SessionStart {
                            session_id: session.session_id,
                            teacher_name: teacher_name.clone(),
                            class_name: class_name.clone(),
                        })
                        .await;
                }
                state.emit("session-changed", Some(session));
                tracing::info!("TEST MODU: ders oturumu açıldı");
            }

            // İstenmişse öğretmen ekranı yayınını da aç.
            if !state.broadcasting()
                && state.registry.online_count() > 0
                && std::env::var(BROADCAST_VAR)
                    .map(|v| v == "1")
                    .unwrap_or(false)
            {
                match crate::broadcast::start(Arc::clone(&state), true) {
                    Ok(handle) => {
                        tracing::info!("TEST MODU: öğretmen ekranı yayını açıldı");
                        *state.broadcast.write() = Some(handle);
                    }
                    Err(err) => {
                        tracing::error!(
                            error = %format!("{err:#}"),
                            "TEST MODU: öğretmen ekranı yayını açılamadı"
                        );
                    }
                }
            }

            // Yayını olmayan bağlı cihazlarda önizlemeyi başlat.
            for (device_id, tx) in state.registry.all_senders() {
                let flags = state.registry.flags(device_id);
                // Yayını zaten açık olanı ve ekranı yakalanamayanı atla:
                // hata bildiren cihazda yayını sürekli yeniden açmak,
                // konsolda hata ile "bekleniyor" arasında gidip gelen bir
                // görüntüye yol açar.
                if flags.streaming || flags.screen_error.is_some() {
                    continue;
                }
                let sent = tx
                    .send(ServerMessage::StartScreenStream {
                        stream_id: 1,
                        quality: StreamQuality::PREVIEW,
                    })
                    .await;
                if sent.is_ok() {
                    state.registry.update_flags(device_id, |f| {
                        f.streaming = true;
                        f.quality = Some(StreamQuality::PREVIEW);
                    });
                    tracing::info!(%device_id, "TEST MODU: ekran yayını başlatıldı");
                }
            }
            state.notify_devices_changed();
        }
    });
}
