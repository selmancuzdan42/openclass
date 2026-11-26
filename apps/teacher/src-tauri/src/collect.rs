//! Dosya toplamanın öğretmen tarafındaki durumu.
//!
//! "Topla" her hedefe bir istek gönderir; öğrenciler desene uyan dosyaları
//! gezgin indirmesiyle aynı akıştan yollar ve sonunda bir özet bildirir.
//! Burada tutulan durum, öğretmenin kimin teslim ettiğini, kimde dosya
//! bulunmadığını ve kimin hiç yanıt vermediğini tek tabloda görmesini sağlar.
//!
//! Yalnızca **son** toplama tutulur; yenisi başlayınca eskisinin yerini alır.

use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use uuid::Uuid;

use crate::state::AppState;

/// Bu süre içinde hiç yanıt vermeyen cihaz "yanıt yok" sayılır.
///
/// 0.5.2 ve öncesi ajanlar özet bildirmez; dosya da göndermedilerse durumları
/// sonsuza kadar "bekleniyor" kalmasın.
const REPLY_TIMEOUT: Duration = Duration::from_secs(30);

/// Tek bir bilgisayarın toplamadaki durumu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetState {
    /// İstek gönderildi, henüz hiçbir şey gelmedi.
    Waiting,
    /// Dosya geliyor.
    Receiving,
    /// Öğrenci işin bittiğini bildirdi.
    Done,
    /// Süre doldu, yanıt gelmedi.
    NoReply,
    /// İstek gönderilemedi ya da bağlantı toplama sırasında koptu.
    Offline,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Target {
    pub device_id: String,
    pub hostname: String,
    pub state: TargetState,
    /// Öğretmene ulaşan ve doğrulanan dosya sayısı.
    pub files: u32,
    /// Öğrencide desene uyan dosya sayısı; özet gelmediyse `None`.
    pub matched: Option<u32>,
    /// Gönderilemeyen dosyalar ya da isteğin reddedilme nedeni.
    pub errors: Vec<String>,
}

/// Bir toplama işlemi; arayüze olduğu gibi gönderilir.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Collection {
    pub id: String,
    pub pattern: String,
    /// Öğrencide bakılan klasörün adı ("Masaüstü"…).
    pub root: String,
    /// Dosyaların öğretmen bilgisayarında kaydedildiği klasör.
    pub dir: String,
    pub delete_source: bool,
    pub started_at: i64,
    pub targets: Vec<Target>,
}

/// Yeni bir toplamayı kaydeder ve yanıt süresini başlatır.
///
/// `targets`: cihaz, bilgisayar adı ve isteğin gönderilip gönderilemediği.
pub fn start(
    state: &Arc<AppState>,
    pattern: String,
    root: String,
    dir: String,
    delete_source: bool,
    targets: Vec<(Uuid, String, bool)>,
) {
    let id = Uuid::new_v4().to_string();
    let collection = Collection {
        id: id.clone(),
        pattern,
        root,
        dir,
        delete_source,
        started_at: crate::db::now(),
        targets: targets
            .into_iter()
            .map(|(device_id, hostname, sent)| Target {
                device_id: device_id.to_string(),
                hostname,
                state: if sent {
                    TargetState::Waiting
                } else {
                    TargetState::Offline
                },
                files: 0,
                matched: None,
                errors: Vec::new(),
            })
            .collect(),
    };
    *state.collection.write() = Some(collection);
    emit(state);

    let state = Arc::clone(state);
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(REPLY_TIMEOUT).await;
        let changed = {
            let mut guard = state.collection.write();
            match guard.as_mut().filter(|c| c.id == id) {
                Some(c) => {
                    let mut changed = false;
                    for t in c.targets.iter_mut() {
                        if t.state == TargetState::Waiting {
                            t.state = TargetState::NoReply;
                            changed = true;
                        }
                    }
                    changed
                }
                None => false,
            }
        };
        if changed {
            emit(&state);
        }
    });
}

/// Cihazın kaydını değiştirir ve arayüze bildirir.
fn update(state: &AppState, device_id: Uuid, f: impl FnOnce(&mut Target)) {
    let id = device_id.to_string();
    {
        let mut guard = state.collection.write();
        let Some(target) = guard
            .as_mut()
            .and_then(|c| c.targets.iter_mut().find(|t| t.device_id == id))
        else {
            return;
        };
        f(target);
    }
    emit(state);
}

/// Cihazdan bir dosya gelmeye başladı.
pub fn receiving(state: &AppState, device_id: Uuid) {
    update(state, device_id, |t| {
        if t.state != TargetState::Done {
            t.state = TargetState::Receiving;
        }
    });
}

/// Cihazdan gelen bir dosya doğrulanıp kaydedildi.
pub fn file_saved(state: &AppState, device_id: Uuid) {
    update(state, device_id, |t| t.files += 1);
}

/// Cihazdan gelen bir dosya kaydedilemedi.
pub fn file_failed(state: &AppState, device_id: Uuid, reason: String) {
    update(state, device_id, |t| t.errors.push(reason));
}

/// Öğrenci toplamanın bittiğini bildirdi.
pub fn done(state: &AppState, device_id: Uuid, matched: u32, errors: Vec<String>) {
    update(state, device_id, |t| {
        t.state = TargetState::Done;
        t.matched = Some(matched);
        // Karşı taraftan gelen metin sınırlanır; arayüzü şişirmesin.
        t.errors.extend(
            errors
                .into_iter()
                .take(20)
                .map(|e| e.chars().take(300).collect()),
        );
    });
}

/// Cihazın bağlantısı koptu; toplaması sürüyorsa yarıda kaldı.
pub fn dropped(state: &AppState, device_id: Uuid) {
    update(state, device_id, |t| {
        if matches!(t.state, TargetState::Waiting | TargetState::Receiving) {
            t.state = TargetState::Offline;
        }
    });
}

fn emit(state: &AppState) {
    state.emit("collection-changed", state.collection.read().clone());
}
