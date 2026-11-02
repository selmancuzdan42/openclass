//! Öğrenci arayüzünün çağırdığı Tauri komutları.

use std::sync::Arc;

use openclass_protocol::DEFAULT_CONTROL_PORT;
use tauri::State;

use crate::state::{
    parse_address, ActiveSession, AppState, ConnectionStatus, DiscoveredTeacher, PairIntent,
    ReceivedNotice, SharingState,
};

pub type CmdResult<T> = Result<T, String>;

/// Arayüzün tek seferde ihtiyaç duyduğu tüm durum.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusSnapshot {
    pub hostname: String,
    pub device_id: String,
    pub app_version: String,
    pub status: ConnectionStatus,
    pub session: Option<ActiveSession>,
    pub sharing: SharingState,
    pub help_requested: bool,
    pub paired_teacher_name: Option<String>,
    pub paired_teacher_address: Option<String>,
    pub manual_address: Option<String>,
    pub notices: Vec<ReceivedNotice>,
    /// Bu platformda ekran yakalama destekleniyor mu?
    pub capture_supported: bool,
}

#[tauri::command]
pub fn get_status(state: State<'_, Arc<AppState>>) -> CmdResult<StatusSnapshot> {
    let cfg = state.config.read();
    Ok(StatusSnapshot {
        hostname: state.hostname.clone(),
        device_id: state.identity.device_id.to_string(),
        app_version: openclass_common::APP_VERSION.to_string(),
        status: state.status.read().clone(),
        session: state.session.read().clone(),
        sharing: state.sharing.read().clone(),
        help_requested: *state.help_requested.read(),
        paired_teacher_name: cfg.paired_teacher.as_ref().map(|t| t.teacher_name.clone()),
        paired_teacher_address: cfg
            .paired_teacher
            .as_ref()
            .map(|t| format!("{}:{}", t.host, t.port)),
        manual_address: cfg.manual_teacher_address.clone(),
        notices: state.notices.read().clone(),
        capture_supported: openclass_screen::is_supported(),
    })
}

/// Ağda görülen öğretmen konsolları.
#[tauri::command]
pub fn list_teachers(state: State<'_, Arc<AppState>>) -> CmdResult<Vec<DiscoveredTeacher>> {
    let mut list: Vec<_> = state.teachers.read().values().cloned().collect();
    list.sort_by(|a, b| a.teacher_name.cmp(&b.teacher_name));
    Ok(list)
}

/// Öğretmen konsoluyla eşleşir.
///
/// `address` boş bırakılırsa `teacher_id` ile keşfedilen öğretmen kullanılır.
#[tauri::command]
pub fn pair_with_teacher(
    state: State<'_, Arc<AppState>>,
    teacher_id: Option<String>,
    address: Option<String>,
    code: String,
) -> CmdResult<()> {
    let code = code.trim().to_string();
    if code.chars().filter(|c| c.is_ascii_digit()).count() < 6 {
        return Err("Eşleştirme kodu 6 haneli olmalı.".into());
    }

    let intent = if let Some(raw) = address.as_ref().filter(|a| !a.trim().is_empty()) {
        let (host, port) = parse_address(raw, DEFAULT_CONTROL_PORT).ok_or_else(|| {
            "Adres anlaşılamadı. Örnek: 192.168.1.10 veya 192.168.1.10:7643".to_string()
        })?;
        PairIntent {
            host,
            port,
            code,
            expected_fingerprint: None,
        }
    } else {
        let id = teacher_id
            .as_deref()
            .and_then(|s| uuid::Uuid::parse_str(s).ok())
            .ok_or_else(|| "Öğretmen seçilmedi ve adres girilmedi.".to_string())?;
        let teachers = state.teachers.read();
        let found = teachers
            .get(&id)
            .ok_or_else(|| "Seçilen öğretmen konsolu artık ağda görünmüyor.".to_string())?;
        PairIntent {
            host: found.host.clone(),
            port: found.control_port,
            code,
            expected_fingerprint: Some(found.fingerprint.clone()),
        }
    };

    tracing::info!(host = %intent.host, port = intent.port, "eşleştirme isteniyor");
    *state.pair_intent.write() = Some(intent);
    state.wake.notify_waiters();
    Ok(())
}

/// Eşleştirmeyi kaldırır ve bağlantıyı keser.
#[tauri::command]
pub fn unpair(state: State<'_, Arc<AppState>>) -> CmdResult<()> {
    // Elle kaldırmada adres de silinir: kullanıcı baştan kurmak istiyor.
    state.config.write().manual_teacher_address = None;
    state
        .clear_pairing()
        .map_err(|e| format!("Ayarlar kaydedilemedi: {e}"))?;
    tracing::info!("eşleştirme kaldırıldı");
    Ok(())
}

/// Keşfin çalışmadığı ağlarda öğretmen adresini elle sabitler.
#[tauri::command]
pub fn set_manual_address(
    state: State<'_, Arc<AppState>>,
    address: Option<String>,
) -> CmdResult<()> {
    let cleaned = address
        .map(|a| a.trim().to_string())
        .filter(|a| !a.is_empty());
    if let Some(raw) = cleaned.as_ref() {
        if parse_address(raw, DEFAULT_CONTROL_PORT).is_none() {
            return Err("Adres anlaşılamadı. Örnek: 192.168.1.10 veya 192.168.1.10:7643".into());
        }
    }
    state.config.write().manual_teacher_address = cleaned;
    state
        .save_config()
        .map_err(|e| format!("Ayarlar kaydedilemedi: {e}"))?;
    state.wake.notify_waiters();
    Ok(())
}

/// Bağlantıyı beklemeden yeniden dener.
#[tauri::command]
pub fn reconnect_now(state: State<'_, Arc<AppState>>) -> CmdResult<()> {
    state.wake.notify_waiters();
    Ok(())
}

/// "Yardım İste" butonu.
#[tauri::command]
pub async fn request_help(state: State<'_, Arc<AppState>>, note: Option<String>) -> CmdResult<()> {
    if !matches!(*state.status.read(), ConnectionStatus::Connected { .. }) {
        return Err("Öğretmene bağlı değilsiniz.".into());
    }
    let note = note.map(|n| n.trim().to_string()).filter(|n| !n.is_empty());
    *state.help_requested.write() = true;
    state
        .send_to_teacher(openclass_protocol::control::ClientMessage::HelpRequest { note })
        .await
        .map_err(|e| format!("Yardım talebi gönderilemedi: {e}"))?;
    tracing::info!("yardım talebi gönderildi");
    Ok(())
}

/// Yardım talebini geri çeker.
#[tauri::command]
pub async fn cancel_help(state: State<'_, Arc<AppState>>) -> CmdResult<()> {
    *state.help_requested.write() = false;
    state
        .send_to_teacher(openclass_protocol::control::ClientMessage::HelpCancel)
        .await
        .map_err(|e| format!("İstek geri çekilemedi: {e}"))?;
    Ok(())
}

/// Gelen mesajları temizler.
#[tauri::command]
pub fn clear_notices(state: State<'_, Arc<AppState>>) -> CmdResult<()> {
    state.notices.write().clear();
    Ok(())
}

/// Yapılandırma penceresini gizler. Ajan tepside çalışmayı sürdürür.
#[tauri::command]
pub fn hide_config_window(window: tauri::Window) -> CmdResult<()> {
    window
        .hide()
        .map_err(|e| format!("Pencere gizlenemedi: {e}"))
}

/// Ajanı tamamen durdurur.
///
/// Bu düğme bilinçli olarak **yapılandırma penceresindedir**, tepsi
/// menüsünde değildir: ders ortasında öğrencinin tek tıkla izlemeyi
/// kapatabilmesi istenmez. Bu bir kurcalama koruması değildir — süreç
/// görev yöneticisinden yine sonlandırılabilir.
#[tauri::command]
pub fn quit_agent(app: tauri::AppHandle, state: State<'_, Arc<AppState>>) -> CmdResult<()> {
    tracing::info!("ajan yapılandırma penceresinden durduruluyor");
    state.cancel.cancel();
    app.exit(0);
    Ok(())
}

/// Öğretmen ekranı yayınının durumu.
#[tauri::command]
pub fn get_broadcast(
    state: State<'_, Arc<AppState>>,
) -> CmdResult<Option<crate::state::ActiveBroadcast>> {
    Ok(state.broadcast.read().clone())
}

/// Uzaktan kontrolün durumu.
#[tauri::command]
pub fn get_remote_control(
    state: State<'_, Arc<AppState>>,
) -> CmdResult<Option<crate::state::RemoteControl>> {
    Ok(state.remote_control.read().clone())
}

/// Otomatik başlatmanın durumu.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutostartState {
    pub supported: bool,
    pub enabled: bool,
}

#[tauri::command]
pub fn get_autostart() -> CmdResult<AutostartState> {
    Ok(AutostartState {
        supported: crate::autostart::is_supported(),
        enabled: crate::autostart::is_enabled(),
    })
}

/// Windows açılışında otomatik başlatmayı açar veya kapatır.
///
/// Girdi standart `Run` anahtarına uygulamanın kendi adıyla yazılır;
/// gizlenmez ve kaldırması zorlaştırılmaz.
#[tauri::command]
pub fn set_autostart(enabled: bool) -> CmdResult<()> {
    crate::autostart::set_enabled(enabled)
        .map_err(|e| format!("Otomatik başlatma ayarlanamadı: {e}"))
}

/// Ekran kilidinin durumu.
#[tauri::command]
pub fn get_screen_lock(
    state: State<'_, Arc<AppState>>,
) -> CmdResult<Option<crate::state::ScreenLock>> {
    Ok(state.screen_lock.read().clone())
}

/// Uzak dosya gezgininin durumu.
#[tauri::command]
pub fn get_browsing(state: State<'_, Arc<AppState>>) -> CmdResult<Option<crate::state::Browsing>> {
    Ok(state.browsing.read().clone())
}
