//! OpenClass Teacher — öğretmen konsolu.
//!
//! Uygulama açıldığında yerel ağ servisleri (keşif + kontrol sunucusu)
//! kendi Rust arka planında başlar; ayrı bir sunucu programı yoktur.
//! Pencere kapatıldığında tüm görevler iptal edilir, öğrencilere ders
//! bitişi bildirilir ve bağlantılar düzgün kapatılır.

mod broadcast;
mod browse;
mod commands;
mod config;
mod db;
mod net;
mod state;
mod testing;
mod transfer;

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Context, Result};
use openclass_common::paths::{self, App};
use openclass_common::tls::TlsIdentity;
use parking_lot::RwLock;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::config::TeacherConfig;
use crate::db::Db;
use crate::state::{AppState, Registry};

/// Ekran karelerinin arayüze taşındığı özel URI şeması.
///
/// Kareler Tauri IPC'sinden JSON olarak geçmez; `<img src="…">` ile doğrudan
/// bu şemadan ham JPEG olarak okunur. Base64 şişmesi ve JSON ayrıştırma
/// maliyeti böylece ortadan kalkar.
const FRAME_SCHEME: &str = "ocframe";

/// Uygulamayı başlatır.
pub fn run() {
    // Loglama her şeyden önce kurulur; sonraki hatalar da diske düşsün.
    let _log_guard = match openclass_common::logging::init(App::Teacher) {
        Ok(guard) => Some(guard),
        Err(err) => {
            eprintln!("UYARI: loglama başlatılamadı: {err:#}");
            None
        }
    };

    let state = match bootstrap() {
        Ok(state) => state,
        Err(err) => {
            tracing::error!(error = %format!("{err:#}"), "uygulama başlatılamadı");
            eprintln!("OpenClass Teacher başlatılamadı: {err:#}");
            std::process::exit(1);
        }
    };

    let cancel = state.cancel.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(Arc::clone(&state))
        .register_uri_scheme_protocol(FRAME_SCHEME, {
            let state = Arc::clone(&state);
            move |_ctx, request| serve_frame(&state, request)
        })
        .invoke_handler(tauri::generate_handler![
            commands::search_devices,
            commands::set_display_name,
            commands::revoke_device,
            commands::start_pairing,
            commands::cancel_pairing,
            commands::get_pairing,
            commands::start_screen_streams,
            commands::stop_screen_streams,
            commands::focus_device,
            commands::unfocus_devices,
            commands::send_notice,
            commands::clear_help_request,
            commands::get_server_info,
            commands::update_settings,
            commands::get_audit_log,
            commands::list_groups,
            commands::save_group,
            commands::delete_group,
            commands::suggest_groups,
            commands::start_broadcast,
            commands::stop_broadcast,
            commands::get_broadcast,
            commands::start_remote_control,
            commands::stop_remote_control,
            commands::send_remote_input,
            commands::lock_devices,
            commands::unlock_devices,
            commands::send_file,
            commands::get_transfers,
            commands::clear_finished_transfers,
            commands::open_browser,
            commands::close_browser,
            commands::browse_list,
            commands::browse_fetch,
            commands::get_browse,
            commands::open_downloads_dir,
            commands::get_download_root,
            commands::set_download_root,
            frame_base_url,
        ])
        .setup({
            let state = Arc::clone(&state);
            move |app| {
                *state.app.write() = Some(app.handle().clone());
                spawn_network_services(Arc::clone(&state));
                Ok(())
            }
        })
        .build(tauri::generate_context!())
        .expect("Tauri uygulaması kurulamadı")
        .run(move |_app, event| {
            if let tauri::RunEvent::ExitRequested { .. } = event {
                // Kapanışta ağ görevleri iptal edilir; read_loop öğrencilere
                // oturumun bittiğini bildirip bağlantıları kapatır.
                tracing::info!("çıkış isteniyor, ağ servisleri durduruluyor");
                cancel.cancel();
            }
        });
}

/// Arayüzün ekran karelerini çekeceği taban adres.
///
/// Windows'ta Tauri özel şemaları `http://<şema>.localhost` altında sunar;
/// diğer platformlarda `<şema>://localhost` kullanılır.
#[tauri::command]
fn frame_base_url() -> String {
    if cfg!(windows) {
        format!("http://{FRAME_SCHEME}.localhost")
    } else {
        format!("{FRAME_SCHEME}://localhost")
    }
}

/// Disk durumunu okuyup uygulama durumunu kurar.
fn bootstrap() -> Result<Arc<AppState>> {
    let data_dir = paths::data_dir(App::Teacher)?;
    let config_path = paths::config_file(App::Teacher)?;
    let secrets_dir = paths::secrets_dir(App::Teacher)?;
    let db_path = paths::database_file(App::Teacher)?;

    let config = TeacherConfig::load_or_create(&config_path)?;
    let teacher_id = config.teacher_id;
    let tls = TlsIdentity::load_or_create(&secrets_dir).context("TLS kimliği hazırlanamadı")?;
    let db = Db::open(&db_path)?;

    tracing::info!(
        %teacher_id,
        veri_dizini = %data_dir.display(),
        parmak_izi = %tls.fingerprint_hex(),
        "öğretmen konsolu hazırlandı"
    );

    Ok(Arc::new(AppState {
        teacher_id,
        tls: Arc::new(tls),
        db: Arc::new(db),
        registry: Arc::new(Registry::new()),
        config: RwLock::new(config),
        config_path,
        pairing: RwLock::new(None),
        test_pairing_code: testing::fixed_pairing_code(),
        app: RwLock::new(None),
        transfers: RwLock::new(HashMap::new()),
        broadcast_fallback: RwLock::new(std::collections::HashSet::new()),
        browse: RwLock::new(None),
        download_root: RwLock::new(None),
        controlled_device: RwLock::new(None),
        broadcast: RwLock::new(None),
        broadcast_keyframe_requested: std::sync::atomic::AtomicBool::new(false),
        network_error: RwLock::new(None),
        cancel: CancellationToken::new(),
    }))
}

/// Keşif ve kontrol sunucusu görevlerini başlatır.
fn spawn_network_services(state: Arc<AppState>) {
    let cancel = state.cancel.clone();

    // Test kolaylıkları: sürüm derlemesinde ikisi de hiç çalışmaz.
    testing::rearm_pairing(&state);
    testing::spawn_autostart(Arc::clone(&state));

    tauri::async_runtime::spawn({
        let state = Arc::clone(&state);
        let cancel = cancel.clone();
        async move {
            if let Err(err) = net::server::run(Arc::clone(&state), cancel).await {
                tracing::error!(error = %format!("{err:#}"), "kontrol sunucusu durdu");
                state.emit(
                    "network-error",
                    format!("Bağlantı sunucusu başlatılamadı: {err}"),
                );
            }
        }
    });

    tauri::async_runtime::spawn({
        let state = Arc::clone(&state);
        async move {
            if let Err(err) = net::discovery::run(Arc::clone(&state), cancel).await {
                tracing::error!(error = %format!("{err:#}"), "keşif servisi durdu");
                state.emit("network-error", format!("Ağ keşfi başlatılamadı: {err}"));
            }
        }
    });
}

/// `ocframe://` isteklerini karşılar.
///
/// Yol biçimi: `/<cihaz-kimliği>?since=<sıra>`. Yanıt, arayüzün elindeki
/// sıradan sonraki ekran bölgelerini taşıyan ikili bir pakettir; arayüz
/// bunları `<canvas>` üzerinde birleştirir.
///
/// Bölgeler burada çözülmez veya birleştirilmez: 20 cihaz için kare başına
/// JPEG çözme + yeniden kodlama öğretmen makinesini gereksiz yorardı.
/// Tarayıcı bu işi donanım hızlandırmalı yapar.
fn serve_frame(
    state: &Arc<AppState>,
    request: tauri::http::Request<Vec<u8>>,
) -> tauri::http::Response<Cow<'static, [u8]>> {
    // Arayüz bu kaynağı `fetch` ile çeker. Özel şema, pencerenin kökeninden
    // farklı bir köken sayılır; CORS başlığı olmadan tarayıcı yanıtı
    // okumayı reddeder ve hata yakalayıcıya düşer. `<img src>` kullanılırken
    // gerekmiyordu, `fetch`e geçince zorunlu oldu.
    let empty = |status: u16| {
        tauri::http::Response::builder()
            .status(status)
            .header("Content-Type", "text/plain; charset=utf-8")
            .header("Access-Control-Allow-Origin", "*")
            .body(Cow::Borrowed(b"".as_slice()))
            .expect("sabit yanıt her zaman kurulabilir")
    };

    let uri = request.uri();
    let Ok(device_id) = Uuid::parse_str(uri.path().trim_start_matches('/')) else {
        return empty(404);
    };

    // `since`: arayüzün elindeki son sıra numarası. Yoksa sıfır kabul edilir
    // ve tam görüntü (anahtar kare) gönderilir.
    let since = uri
        .query()
        .and_then(|q| {
            q.split('&')
                .filter_map(|pair| pair.split_once('='))
                .find(|(k, _)| *k == "since")
                .map(|(_, v)| v.to_string())
        })
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);

    let Some(blob) = state.registry.screen_blob(device_id, since) else {
        // Yeni veri yok; arayüz mevcut tuvalini korur.
        tracing::trace!(%device_id, since, "ocframe: veri yok (204)");
        return empty(204);
    };

    tracing::debug!(%device_id, since, bayt = blob.len(), "ocframe: paket verildi");

    tauri::http::Response::builder()
        .status(200)
        .header("Content-Type", "application/octet-stream")
        .header("Cache-Control", "no-store")
        .header("Access-Control-Allow-Origin", "*")
        .body(Cow::Owned(blob))
        .unwrap_or_else(|_| empty(500))
}
