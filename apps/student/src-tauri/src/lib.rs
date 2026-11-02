//! OpenClass Student — öğrenci ajanı.
//!
//! Uygulama açıldığında iki görev başlar:
//!
//! * **Keşif cevaplayıcısı** — öğretmen probe'larına cevap verir.
//! * **Bağlantı döngüsü** — eşleşilmiş öğretmene bağlanır, kopunca yeniden dener.
//!
//! Uygulama sistem tepsisinde çalışabilir ancak gizli bir izleme mekanizması
//! içermez: ekran paylaşımının aktif olduğu her zaman arayüzde görünür.

mod autostart;
mod browse;
mod commands;
mod config;
mod identity;
mod input;
mod lock;
mod net;
mod state;
mod testing;
mod transfer;
mod tray;

use std::borrow::Cow;
use std::sync::Arc;

use anyhow::Result;
use openclass_common::paths::{self, App};
use parking_lot::RwLock;
use tauri::Manager;
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

use crate::config::StudentConfig;
use crate::identity::StudentIdentity;
use crate::state::{AppState, ConnectionStatus, SharingState};

pub fn run() {
    let _log_guard = match openclass_common::logging::init(App::Student) {
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
            eprintln!("OpenClass Student başlatılamadı: {err:#}");
            std::process::exit(1);
        }
    };

    let cancel = state.cancel.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(Arc::clone(&state))
        .register_uri_scheme_protocol(BROADCAST_SCHEME, {
            let state = Arc::clone(&state);
            move |_ctx, request| serve_broadcast(&state, request)
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::list_teachers,
            commands::pair_with_teacher,
            commands::unpair,
            commands::set_manual_address,
            commands::reconnect_now,
            commands::request_help,
            commands::cancel_help,
            commands::clear_notices,
            commands::hide_config_window,
            commands::quit_agent,
            commands::get_broadcast,
            commands::get_remote_control,
            commands::get_autostart,
            commands::get_screen_lock,
            commands::get_browsing,
            commands::set_autostart,
            broadcast_base_url,
        ])
        .setup({
            let state = Arc::clone(&state);
            move |app| {
                let handle = app.handle().clone();
                *state.app.write() = Some(handle.clone());

                match tray::build(&handle, Arc::clone(&state)) {
                    Ok(handles) => {
                        *state.tray.write() = Some(handles);
                        tray::refresh(&state);
                    }
                    Err(err) => {
                        // Tepsi kurulamazsa uygulama yine çalışır; ancak
                        // öğrenci durumu göremez, bu yüzden pencere açık kalır.
                        tracing::error!(error = %format!("{err:#}"), "sistem tepsisi kurulamadı");
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                        }
                    }
                }

                // Henüz eşleştirilmemiş cihazda yapılandırma penceresi açılır;
                // eşleştirilmişse ajan sessizce tepside çalışır.
                //
                // Test modunda ajan kendiliğinden eşleşeceği için pencere
                // açılmaz; aksi hâlde yük testinde onlarca pencere açılırdı.
                if state.config.read().paired_teacher.is_none()
                    && testing::autopair_code().is_none()
                {
                    tray::show_config_window(&handle);
                }

                spawn_network_services(Arc::clone(&state));
                Ok(())
            }
        })
        .on_window_event(|window, event| {
            // Pencerenin kapatılması ajanı durdurmaz; ajan tepside çalışmayı
            // sürdürür. Pencere gizlenmez, **yok edilir**: WebView süreci
            // kapanır ve belleği geri verir. Düşük bellekli laboratuvar
            // makinelerinde bu fark belirleyici.
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                tracing::debug!(
                    etiket = window.label(),
                    "pencere kapatıldı, WebView serbest bırakılıyor"
                );
            }
        })
        .build(tauri::generate_context!())
        .expect("Tauri uygulaması kurulamadı")
        .run(move |_app, event| {
            match event {
                tauri::RunEvent::ExitRequested { .. } => {
                    tracing::info!("çıkış isteniyor, ağ servisleri durduruluyor");
                    cancel.cancel();
                }
                // Tüm pencereler kapansa bile ajan tepside çalışmaya devam eder.
                tauri::RunEvent::WindowEvent { .. } => {}
                _ => {}
            }
        });
}

fn bootstrap() -> Result<Arc<AppState>> {
    let config_path = paths::config_file(App::Student)?;
    let secrets_dir = paths::secrets_dir(App::Student)?;

    let config = StudentConfig::load_or_create(&config_path)?;
    let identity = StudentIdentity::load_or_create(&secrets_dir)?;
    let hostname = openclass_common::machine::normalized_hostname();

    let initial_status = if config.paired_teacher.is_some() {
        ConnectionStatus::Connecting
    } else {
        ConnectionStatus::NotPaired
    };

    tracing::info!(
        %hostname,
        cihaz = %identity.device_id,
        eslesmis = config.paired_teacher.is_some(),
        "öğrenci ajanı hazırlandı"
    );

    Ok(Arc::new(AppState {
        identity,
        hostname,
        config: RwLock::new(config),
        config_path,
        status: RwLock::new(initial_status),
        teachers: RwLock::new(Default::default()),
        notices: RwLock::new(Vec::new()),
        session: RwLock::new(None),
        broadcast: RwLock::new(None),
        teacher_screen: RwLock::new(openclass_screen::ScreenBuffer::new()),
        remote_control: RwLock::new(None),
        screen_lock: RwLock::new(None),
        browsing: RwLock::new(None),
        sharing: RwLock::new(SharingState::default()),
        help_requested: RwLock::new(false),
        pair_intent: RwLock::new(None),
        outbox: RwLock::new(None),
        tray: RwLock::new(None),
        wake: Notify::new(),
        app: RwLock::new(None),
        cancel: CancellationToken::new(),
    }))
}

fn spawn_network_services(state: Arc<AppState>) {
    let cancel = state.cancel.clone();

    tauri::async_runtime::spawn({
        let state = Arc::clone(&state);
        let cancel = cancel.clone();
        async move {
            if let Err(err) = net::announce::run(Arc::clone(&state), cancel).await {
                let text = format!("{err:#}");
                tracing::error!(hata = %text, "keşif cevaplayıcısı durdu");
                // Keşif çalışmasa da elle adres girilerek bağlanılabilir;
                // bu yüzden uygulama durmaz, yalnızca uyarı gösterilir.
                state.emit("network-warning", text);
            }
        }
    });

    tauri::async_runtime::spawn({
        let state = Arc::clone(&state);
        async move {
            net::client::run(state, cancel).await;
        }
    });
}

/// Öğretmen ekranı yayınının arayüze taşındığı özel URI şeması.
const BROADCAST_SCHEME: &str = "ocbcast";

/// Arayüzün yayın bölgelerini çekeceği taban adres.
#[tauri::command]
fn broadcast_base_url() -> String {
    if cfg!(windows) {
        format!("http://{BROADCAST_SCHEME}.localhost")
    } else {
        format!("{BROADCAST_SCHEME}://localhost")
    }
}

/// `ocbcast://` isteklerini karşılar.
///
/// Öğretmen konsolundaki `ocframe://` ile aynı mantık: bölgeler burada
/// çözülmez, ham olarak arayüze verilir ve tuvalde birleştirilir.
fn serve_broadcast(
    state: &Arc<AppState>,
    request: tauri::http::Request<Vec<u8>>,
) -> tauri::http::Response<Cow<'static, [u8]>> {
    let empty = |status: u16| {
        tauri::http::Response::builder()
            .status(status)
            .header("Content-Type", "text/plain; charset=utf-8")
            // Özel şema farklı bir köken sayılır; `fetch` için gerekli.
            .header("Access-Control-Allow-Origin", "*")
            .body(Cow::Borrowed(b"".as_slice()))
            .expect("sabit yanıt her zaman kurulabilir")
    };

    let since = request
        .uri()
        .query()
        .and_then(|q| {
            q.split('&')
                .filter_map(|pair| pair.split_once('='))
                .find(|(k, _)| *k == "since")
                .map(|(_, v)| v.to_string())
        })
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);

    let Some(blob) = state.teacher_screen.read().encode_since(since) else {
        return empty(204);
    };

    tauri::http::Response::builder()
        .status(200)
        .header("Content-Type", "application/octet-stream")
        .header("Cache-Control", "no-store")
        .header("Access-Control-Allow-Origin", "*")
        .body(Cow::Owned(blob))
        .unwrap_or_else(|_| empty(500))
}
