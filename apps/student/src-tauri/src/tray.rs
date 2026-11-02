//! Sistem tepsisi — öğrencinin gördüğü tek yüzey.
//!
//! Öğrenci ajanı normal kullanımda penceresiz çalışır. Öğrencinin erişimi
//! yalnızca tepsi simgesi ve menüsüyle sınırlıdır:
//!
//! * **Simge** bağlantı ve ekran paylaşımı durumunu gösterir.
//! * **Menü** durumu yazar ve yardım talebi göndermeyi sağlar.
//! * **Yapılandırma** penceresi öğretmen/yönetici içindir; eşleştirme ve
//!   bağlantı adresi oradan ayarlanır.
//!
//! # Neden simge gizlenmiyor?
//!
//! NetSupport'ta "Quiet Mode" öğrenci ajanının varlığını tamamen gizler.
//! OpenClass'ta böyle bir seçenek **yoktur ve eklenmeyecektir**: ekranı
//! paylaşılan kişinin bunu görebilmesi ürünün temel ilkesidir. Simge her
//! zaman görünür ve paylaşım sırasında belirgin biçimde değişir.
//!
//! # Tepsi menüsünde neden "Çıkış" yok?
//!
//! Ders ortasında öğrencinin tek tıkla izlemeyi kapatabilmesi öğretmen için
//! işlevsel bir gerileme olurdu. Ajanı durdurmak yapılandırma penceresinden
//! yapılır. Bu bir kurcalama koruması **değildir** — görev yöneticisinden
//! süreç yine sonlandırılabilir; bunu engellemek işletim sistemi ilkesi
//! gerektirir ve bu projenin kapsamı dışındadır.

use std::sync::Arc;

use anyhow::{Context, Result};
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Wry};

use crate::state::{AppState, ConnectionStatus};

const MENU_HELP: &str = "help";
const MENU_CONFIG: &str = "config";

/// Tepsi simgesi ve menüsünün değiştirilebilir parçaları.
pub struct TrayHandles {
    icon: TrayIcon<Wry>,
    status_item: MenuItem<Wry>,
    help_item: MenuItem<Wry>,
}

/// Tepsi simgesini kurar.
pub fn build(app: &AppHandle, state: Arc<AppState>) -> Result<TrayHandles> {
    let status_item = MenuItem::with_id(app, "status", "Başlatılıyor…", false, None::<&str>)
        .context("tepsi durum öğesi oluşturulamadı")?;
    let help_item = MenuItem::with_id(app, MENU_HELP, "Yardım İste", false, None::<&str>)
        .context("tepsi yardım öğesi oluşturulamadı")?;
    let config_item = MenuItem::with_id(app, MENU_CONFIG, "Yapılandırma…", true, None::<&str>)
        .context("tepsi yapılandırma öğesi oluşturulamadı")?;
    let separator = PredefinedMenuItem::separator(app)?;

    let menu = Menu::with_items(
        app,
        &[
            &status_item,
            &separator,
            &help_item,
            &separator,
            &config_item,
        ],
    )
    .context("tepsi menüsü oluşturulamadı")?;

    let icon = TrayIconBuilder::with_id("openclass-student")
        .icon(icon_image(IconKind::Offline)?)
        .tooltip("OpenClass Öğrenci")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event({
            let state = Arc::clone(&state);
            move |app, event| match event.id().as_ref() {
                MENU_HELP => toggle_help(Arc::clone(&state)),
                MENU_CONFIG => show_config_window(app),
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event| {
            // Sol tıkla menü açılmıyor; çift tık yapılandırmayı getirir.
            if let TrayIconEvent::DoubleClick { .. } = event {
                show_config_window(tray.app_handle());
            }
        })
        .build(app)
        .context("tepsi simgesi oluşturulamadı")?;

    Ok(TrayHandles {
        icon,
        status_item,
        help_item,
    })
}

/// Tepsi simgesini, ipucu metnini ve menüyü güncel duruma göre tazeler.
///
/// Durum her değiştiğinde çağrılır; tepsi henüz kurulmadıysa sessizce geçer.
pub fn refresh(state: &AppState) {
    let guard = state.tray.read();
    let Some(tray) = guard.as_ref() else { return };

    let status = state.status.read().clone();
    let sharing = state.sharing.read().clone();
    let help_requested = *state.help_requested.read();
    let controlled = state.remote_control.read().is_some();
    let locked = state.screen_lock.read().is_some();
    let browsing = state.browsing.read().is_some();
    let hostname = state.hostname.clone();

    let connected = matches!(status, ConnectionStatus::Connected { .. });

    let kind = if sharing.active || controlled || locked || browsing {
        IconKind::Sharing
    } else if connected {
        IconKind::Idle
    } else {
        IconKind::Offline
    };
    if let Ok(image) = icon_image(kind) {
        let _ = tray.icon.set_icon(Some(image));
    }

    let status_text = match &status {
        ConnectionStatus::NotPaired => "Eşleştirilmedi — yapılandırma gerekiyor".to_string(),
        ConnectionStatus::Connecting => "Bağlanıyor…".to_string(),
        ConnectionStatus::Pairing => "Eşleştiriliyor…".to_string(),
        ConnectionStatus::Connected { teacher_name } => format!("Bağlı — {teacher_name}"),
        ConnectionStatus::Disconnected { retry_in_secs, .. } => {
            format!("Bağlantı yok — {retry_in_secs} sn sonra denenecek")
        }
    };

    // Öğrencinin makinesinde olup bitenler tepsiden de okunabilmeli.
    let sharing_text = if locked {
        "  ·  EKRAN KİLİTLİ"
    } else if controlled {
        "  ·  ÖĞRETMEN BİLGİSAYARINIZI KULLANIYOR"
    } else if browsing {
        "  ·  ÖĞRETMEN DOSYALARINIZA BAKIYOR"
    } else if sharing.active {
        "  ·  EKRANINIZ PAYLAŞILIYOR"
    } else {
        ""
    };

    let _ = tray
        .status_item
        .set_text(format!("{status_text}{sharing_text}"));
    let _ = tray.icon.set_tooltip(Some(format!(
        "OpenClass Öğrenci — {hostname}\n{status_text}{}",
        if locked {
            "\nKlavye ve fareniz geçici olarak kilitli"
        } else if controlled {
            "\nÖğretmeniniz bilgisayarınızı uzaktan kullanıyor"
        } else if browsing {
            "\nÖğretmeniniz dosyalarınızı görüntülüyor"
        } else if sharing.active {
            "\nEkranınız öğretmeninizle paylaşılıyor"
        } else {
            ""
        }
    )));

    let _ = tray.help_item.set_enabled(connected);
    let _ = tray.help_item.set_text(if help_requested {
        "Yardım talebini geri çek"
    } else {
        "Yardım İste"
    });
}

/// Yardım talebini açıp kapatır. Tepsi menüsünden çağrılır.
fn toggle_help(state: Arc<AppState>) {
    tauri::async_runtime::spawn(async move {
        let requested = *state.help_requested.read();
        let result = if requested {
            *state.help_requested.write() = false;
            state
                .send_to_teacher(openclass_protocol::control::ClientMessage::HelpCancel)
                .await
        } else {
            *state.help_requested.write() = true;
            state
                .send_to_teacher(openclass_protocol::control::ClientMessage::HelpRequest {
                    note: None,
                })
                .await
        };

        if let Err(err) = result {
            // Gönderilemediyse bayrağı geri al; aksi hâlde arayüz yalan söyler.
            *state.help_requested.write() = requested;
            tracing::warn!(error = %format!("{err:#}"), "yardım talebi gönderilemedi");
        } else {
            tracing::info!(geri_cekildi = requested, "yardım talebi değişti");
        }

        refresh(&state);
        state.emit("status-changed", ());
    });
}

/// Yapılandırma penceresini açar; yoksa o anda oluşturur.
///
/// # Neden pencere baştan oluşturulmuyor?
///
/// Her Tauri penceresi kendi WebView sürecini doğurur ve ölçümlerimizde bu
/// tek başına ~200 MB yerleşik bellek demek. Okul laboratuvarlarında 4–8 GB
/// bellekli makineler var; ajan ders boyunca penceresiz çalışmalı. Pencere
/// yalnızca kurulum sırasında (nadiren) açılır ve kapatılınca **yok edilir**,
/// belleği geri verir.
pub fn show_config_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(CONFIG_WINDOW) {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
        return;
    }

    let built = tauri::WebviewWindowBuilder::new(
        app,
        CONFIG_WINDOW,
        tauri::WebviewUrl::App("index.html".into()),
    )
    .title("OpenClass Öğrenci — Yapılandırma")
    .inner_size(460.0, 640.0)
    .min_inner_size(400.0, 520.0)
    .resizable(true)
    .center()
    .build();

    if let Err(err) = built {
        tracing::error!(%err, "yapılandırma penceresi açılamadı");
    }
}

/// Yapılandırma penceresinin etiketi.
pub const CONFIG_WINDOW: &str = "main";

#[derive(Clone, Copy)]
enum IconKind {
    Idle,
    Sharing,
    Offline,
}

/// Tepsi simgeleri ikiliye gömülür; çalışma anında dosya aranmaz.
fn icon_image(kind: IconKind) -> Result<Image<'static>> {
    const IDLE: &[u8] = include_bytes!("../icons/tray/idle.png");
    const SHARING: &[u8] = include_bytes!("../icons/tray/sharing.png");
    const OFFLINE: &[u8] = include_bytes!("../icons/tray/offline.png");

    let bytes = match kind {
        IconKind::Idle => IDLE,
        IconKind::Sharing => SHARING,
        IconKind::Offline => OFFLINE,
    };
    Image::from_bytes(bytes).context("tepsi simgesi çözümlenemedi")
}
