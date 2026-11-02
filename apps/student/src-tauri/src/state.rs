//! Öğrenci ajanının çalışma anı durumu.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use openclass_protocol::control::ClientMessage;
use parking_lot::RwLock;
use tokio::sync::Notify;
use uuid::Uuid;

use crate::config::StudentConfig;
use crate::identity::StudentIdentity;

/// Bağlantının kullanıcıya gösterilen durumu.
///
/// `rename_all` yalnızca **varyant adlarını** çevirir; varyant içindeki
/// alanlar için `rename_all_fields` gerekir. Bu olmadan arayüz
/// `teacherName` okur ama Rust `teacher_name` gönderir ve durum çubuğunda
/// "Bağlı — undefined" görünür.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ConnectionStatus {
    /// Henüz hiçbir öğretmenle eşleşilmemiş.
    NotPaired,
    /// Eşleşilmiş ama bağlantı yok; yeniden denenecek.
    Disconnected {
        reason: Option<String>,
        retry_in_secs: u64,
    },
    Connecting,
    Pairing,
    Connected {
        teacher_name: String,
    },
}

/// Keşifte görülen bir öğretmen konsolu.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredTeacher {
    pub teacher_id: Uuid,
    pub teacher_name: String,
    /// Probe'un geldiği IP.
    pub host: String,
    pub control_port: u16,
    /// Kullanıcıya gösterilecek parmak izi (`AB:CD:...`).
    pub fingerprint_hex: String,
    #[serde(skip)]
    pub fingerprint: Vec<u8>,
    pub last_seen: i64,
}

/// Öğretmenden gelen bir mesaj.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReceivedNotice {
    pub message_id: Uuid,
    pub title: String,
    pub body: String,
    pub received_at: i64,
}

/// Öğretmen ekranı yayını açık.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveBroadcast {
    pub broadcast_id: Uuid,
    pub teacher_name: String,
    pub started_at: i64,
}

/// Öğretmen dosyalara bakıyor.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Browsing {
    pub teacher_name: String,
    pub started_at: i64,
}

/// Öğrenci klavye/faresi kilitli.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenLock {
    pub message: String,
    pub started_at: i64,
}

/// Öğretmen uzaktan kontrolü devraldı.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteControl {
    pub teacher_name: String,
    pub started_at: i64,
}

/// Aktif ders oturumu.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveSession {
    pub session_id: Uuid,
    pub teacher_name: String,
    pub class_name: String,
    pub started_at: i64,
}

/// Ekran paylaşımının güncel durumu.
///
/// Öğrenciye **her zaman** görünür biçimde gösterilir; gizli ekran paylaşımı
/// bu uygulamada bulunmaz.
#[derive(Debug, Clone, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SharingState {
    pub active: bool,
    pub backend: Option<String>,
    pub fps: Option<u8>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub error: Option<String>,
}

/// Arayüzden gelen eşleştirme isteği.
pub struct PairIntent {
    pub host: String,
    pub port: u16,
    pub code: String,
    /// Keşiften öğrenilen parmak izi; elle adres girildiyse `None`
    /// (ilk bağlantıda öğrenilir ve kullanıcıya doğrulatılır).
    pub expected_fingerprint: Option<Vec<u8>>,
}

/// Saklanan azami mesaj sayısı.
const MAX_NOTICES: usize = 50;

/// Öğretmen ekranı penceresinin etiketi.
pub const BROADCAST_WINDOW: &str = "broadcast";

/// Etkinlik göstergesi penceresinin etiketi.
///
/// Uzaktan kontrol ve dosya gezgini **tek** pencereyi paylaşır: her biri
/// için ayrı WebView açmak 4 GB'lık laboratuvar makinesinde boşa giden
/// bellektir.
pub const ACTIVITY_WINDOW: &str = "activity";

/// Kilit penceresinin etiketi.
pub const LOCK_WINDOW: &str = "lock";

pub struct AppState {
    pub identity: StudentIdentity,
    /// Windows bilgisayar adı (büyük harfe normalleştirilmiş).
    pub hostname: String,
    pub config: RwLock<StudentConfig>,
    pub config_path: PathBuf,

    pub status: RwLock<ConnectionStatus>,
    pub teachers: RwLock<HashMap<Uuid, DiscoveredTeacher>>,
    pub notices: RwLock<Vec<ReceivedNotice>>,
    pub session: RwLock<Option<ActiveSession>>,
    /// Öğretmen ekranı yayını; açıkken tam ekran pencere gösterilir.
    pub broadcast: RwLock<Option<ActiveBroadcast>>,
    /// Öğretmen ekranının alınan bölgeleri.
    pub teacher_screen: RwLock<openclass_screen::ScreenBuffer>,
    /// Uzaktan kontrol açıksa bilgisi.
    pub remote_control: RwLock<Option<RemoteControl>>,
    /// Ekran kilidi açıksa bilgisi.
    pub screen_lock: RwLock<Option<ScreenLock>>,
    /// Uzak dosya gezgini açıksa bilgisi.
    pub browsing: RwLock<Option<Browsing>>,
    pub sharing: RwLock<SharingState>,
    pub help_requested: RwLock<bool>,

    /// Arayüzün tetiklediği eşleştirme isteği.
    pub pair_intent: RwLock<Option<PairIntent>>,
    /// Açık bağlantının giden mesaj kanalı. Bağlantı yokken `None`.
    pub outbox: RwLock<Option<tokio::sync::mpsc::Sender<ClientMessage>>>,
    /// Bağlantı döngüsünü hemen yeniden denemeye zorlar.
    pub wake: Notify,

    pub app: RwLock<Option<tauri::AppHandle>>,
    /// Tepsi simgesi; `setup` tamamlanana kadar `None`.
    pub tray: RwLock<Option<crate::tray::TrayHandles>>,
    pub cancel: tokio_util::sync::CancellationToken,
}

impl AppState {
    pub fn save_config(&self) -> anyhow::Result<()> {
        self.config.read().save(&self.config_path)
    }

    pub fn emit<T: serde::Serialize + Clone>(&self, event: &str, payload: T) {
        use tauri::Emitter;
        if let Some(app) = self.app.read().as_ref() {
            if let Err(err) = app.emit(event, payload) {
                tracing::warn!(%err, event, "arayüze olay gönderilemedi");
            }
        }
    }

    /// Bağlantı durumunu değiştirir, tepsiyi ve arayüzü bilgilendirir.
    pub fn set_status(&self, status: ConnectionStatus) {
        {
            let mut guard = self.status.write();
            if *guard == status {
                return;
            }
            *guard = status.clone();
        }
        tracing::info!(?status, "bağlantı durumu değişti");
        crate::tray::refresh(self);
        self.emit("status-changed", status);
    }

    /// Ekran paylaşım durumunu değiştirir, tepsiyi ve arayüzü bilgilendirir.
    ///
    /// Tepsi simgesi paylaşım sırasında belirgin biçimde değişir; öğrencinin
    /// bunu görebilmesi ürünün temel güvenlik ilkesidir.
    pub fn set_sharing(&self, sharing: SharingState) {
        *self.sharing.write() = sharing.clone();
        crate::tray::refresh(self);
        self.emit("sharing-changed", sharing);
    }

    /// Gelen mesajı saklar ve öğrenciye açılır pencereyle gösterir.
    ///
    /// Mesajlar kalıcı bir panelde birikmez: öğrencinin ekranında sürekli
    /// duran bir OpenClass penceresi yoktur. Mesaj geldiğinde küçük bir
    /// pencere açılır, öğrenci okuyup kapatır.
    pub fn push_notice(&self, notice: ReceivedNotice) {
        {
            let mut guard = self.notices.write();
            guard.push(notice.clone());
            let overflow = guard.len().saturating_sub(MAX_NOTICES);
            if overflow > 0 {
                guard.drain(..overflow);
            }
        }
        self.emit("notice", notice);
        self.show_notice_window();
    }

    /// Sistemin kendi ürettiği bir bilgilendirmeyi gösterir.
    ///
    /// Öğretmenin yazdığı mesajlardan ayrı bir başlık taşır; öğrenci neyin
    /// kimden geldiğini karıştırmamalı.
    pub fn show_notice(&self, body: String) {
        self.push_notice(ReceivedNotice {
            message_id: Uuid::new_v4(),
            title: "OpenClass".to_string(),
            body,
            received_at: unix_now(),
        });
    }

    /// Öğretmen ekranı penceresini açar.
    ///
    /// Tam ekran ve üstte kalır: öğretmen ekranı gösterildiğinde öğrencinin
    /// başka bir şeye bakmaması istenir. Pencere **gizli değildir**; başlıkta
    /// ve içeriğinde ne olduğu yazar.
    pub fn show_broadcast_window(&self, fullscreen: bool) {
        use tauri::Manager;

        let app = self.app.read().clone();
        let Some(app) = app else { return };

        if let Some(window) = app.get_webview_window(BROADCAST_WINDOW) {
            let _ = window.show();
            let _ = window.set_focus();
            return;
        }

        let built = tauri::WebviewWindowBuilder::new(
            &app,
            BROADCAST_WINDOW,
            tauri::WebviewUrl::App("index.html?view=broadcast".into()),
        )
        .title("Öğretmen ekranı")
        // Pencereli modda öğrenci Alt+Tab ile başka uygulamalara geçebilir;
        // penceresiz modda ekranı kaplar ve üstte durur.
        .fullscreen(fullscreen)
        .always_on_top(fullscreen)
        .decorations(!fullscreen)
        .inner_size(1280.0, 760.0)
        .build();

        if let Err(err) = built {
            tracing::error!(%err, "öğretmen ekranı penceresi açılamadı");
        }
    }

    /// Öğretmen ekranı penceresini kapatır.
    pub fn close_broadcast_window(&self) {
        use tauri::Manager;

        let app = self.app.read().clone();
        let Some(app) = app else { return };
        if let Some(window) = app.get_webview_window(BROADCAST_WINDOW) {
            let _ = window.close();
        }
    }

    /// Uzaktan kontrolü başlatır ve göstergeyi açar.
    ///
    /// Gösterge kapatılamaz ve her zaman üsttedir: öğrencinin makinesini
    /// başkasının sürdüğünü bilmemesi kabul edilemez.
    pub fn start_remote_control(&self, teacher_name: String) {
        *self.remote_control.write() = Some(RemoteControl {
            teacher_name,
            started_at: unix_now(),
        });
        self.sync_activity_indicator();
        crate::tray::refresh(self);
        self.emit("remote-control-changed", self.remote_control.read().clone());
    }

    /// Uzaktan kontrolü bitirir ve göstergeyi kapatır.
    pub fn stop_remote_control(&self) {
        if self.remote_control.write().take().is_none() {
            return;
        }
        self.sync_activity_indicator();
        crate::tray::refresh(self);
        self.emit("remote-control-changed", Option::<RemoteControl>::None);
    }

    /// Dosya gezginini başlatır ve göstergeyi açar.
    pub fn start_browsing(&self, teacher_name: String) {
        *self.browsing.write() = Some(Browsing {
            teacher_name,
            started_at: unix_now(),
        });
        self.sync_activity_indicator();
        crate::tray::refresh(self);
        self.emit("browsing-changed", self.browsing.read().clone());
    }

    /// Dosya gezginini kapatır.
    pub fn stop_browsing(&self) {
        if self.browsing.write().take().is_none() {
            return;
        }
        self.sync_activity_indicator();
        crate::tray::refresh(self);
        self.emit("browsing-changed", Option::<Browsing>::None);
    }

    /// Göstergeyi etkinlik durumuna göre açar veya kapatır.
    ///
    /// Uzaktan kontrol veya dosya gezgini açıkken durur, ikisi de
    /// kapandığında yok edilir — belleği geri verir.
    fn sync_activity_indicator(&self) {
        let needed = self.remote_control.read().is_some() || self.browsing.read().is_some();
        if needed {
            self.show_activity_indicator();
        } else {
            self.close_activity_indicator();
        }
    }

    fn show_activity_indicator(&self) {
        use tauri::Manager;

        let app = self.app.read().clone();
        let Some(app) = app else { return };

        if let Some(window) = app.get_webview_window(ACTIVITY_WINDOW) {
            let _ = window.show();
            return;
        }

        let built = tauri::WebviewWindowBuilder::new(
            &app,
            ACTIVITY_WINDOW,
            tauri::WebviewUrl::App("index.html?view=activity".into()),
        )
        .title("OpenClass etkinlik göstergesi")
        .inner_size(420.0, 40.0)
        .resizable(false)
        .always_on_top(true)
        .decorations(false)
        .skip_taskbar(true)
        .focused(false)
        .build();

        match built {
            Ok(window) => {
                // Ekranın üst ortasına yerleştir.
                if let Ok(Some(monitor)) = window.primary_monitor() {
                    let size = monitor.size();
                    let x = (size.width as f64 / 2.0) - 210.0;
                    let _ = window.set_position(tauri::PhysicalPosition::new(x as i32, 8));
                }
            }
            Err(err) => tracing::error!(%err, "etkinlik göstergesi açılamadı"),
        }
    }

    fn close_activity_indicator(&self) {
        use tauri::Manager;

        let app = self.app.read().clone();
        let Some(app) = app else { return };
        if let Some(window) = app.get_webview_window(ACTIVITY_WINDOW) {
            let _ = window.close();
        }
    }

    /// Kayıtlı eşleştirmeyi siler ve "eşleştirilmedi" durumuna döner.
    ///
    /// Öğretmen adresi **korunur**: öğretmen yetkiyi kaldırdıysa öğrenci
    /// büyük ihtimalle aynı öğretmene yeniden eşleşecek, adresi tekrar
    /// yazdırmanın anlamı yok.
    pub fn clear_pairing(&self) -> anyhow::Result<()> {
        self.config.write().paired_teacher = None;
        self.save_config()?;
        self.set_status(ConnectionStatus::NotPaired);
        // Bağlantı döngüsü uyanıp yeni duruma göre davransın.
        self.wake.notify_waiters();
        Ok(())
    }

    /// Ekranı kilitler: girdi engellenir ve açıklayan pencere gösterilir.
    ///
    /// Zaman aşımı kurulur; öğretmen açmayı unutursa kilit kendiliğinden
    /// kalkar (bkz. [`crate::lock::MAX_DURATION`]).
    pub fn lock_screen(self: &Arc<Self>, message: String) {
        if !crate::lock::is_supported() {
            tracing::warn!("ekran kilidi bu platformda desteklenmiyor");
            return;
        }
        if self.screen_lock.read().is_some() {
            return;
        }

        *self.screen_lock.write() = Some(ScreenLock {
            message,
            started_at: unix_now(),
        });
        self.show_lock_window();
        crate::lock::engage();
        crate::tray::refresh(self);
        self.emit("lock-changed", self.screen_lock.read().clone());

        // Unutulmuş kilit makineyi süresiz bloke etmemeli.
        let state = Arc::clone(self);
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(crate::lock::MAX_DURATION).await;
            if state.screen_lock.read().is_some() {
                tracing::warn!("ekran kilidi zaman aşımıyla kaldırıldı");
                state.unlock_screen();
            }
        });
    }

    /// Kilidi kaldırır.
    pub fn unlock_screen(&self) {
        if self.screen_lock.write().take().is_none() {
            return;
        }
        crate::lock::release();
        self.close_lock_window();
        crate::tray::refresh(self);
        self.emit("lock-changed", Option::<ScreenLock>::None);
    }

    fn show_lock_window(&self) {
        use tauri::Manager;

        let app = self.app.read().clone();
        let Some(app) = app else { return };

        if let Some(window) = app.get_webview_window(LOCK_WINDOW) {
            let _ = window.show();
            return;
        }

        let built = tauri::WebviewWindowBuilder::new(
            &app,
            LOCK_WINDOW,
            tauri::WebviewUrl::App("index.html?view=lock".into()),
        )
        .title("Ekran kilitli")
        .fullscreen(true)
        .always_on_top(true)
        .decorations(false)
        .build();

        if let Err(err) = built {
            tracing::error!(%err, "kilit penceresi açılamadı");
        }
    }

    fn close_lock_window(&self) {
        use tauri::Manager;

        let app = self.app.read().clone();
        let Some(app) = app else { return };
        if let Some(window) = app.get_webview_window(LOCK_WINDOW) {
            let _ = window.close();
        }
    }

    /// Mesaj penceresini açar; zaten açıksa öne getirir.
    fn show_notice_window(&self) {
        use tauri::Manager;

        let app = self.app.read().clone();
        let Some(app) = app else { return };

        if let Some(window) = app.get_webview_window("notice") {
            let _ = window.show();
            let _ = window.unminimize();
            let _ = window.set_focus();
            return;
        }

        let built = tauri::WebviewWindowBuilder::new(
            &app,
            "notice",
            tauri::WebviewUrl::App("index.html?view=notice".into()),
        )
        .title("Öğretmeninizden mesaj")
        .inner_size(440.0, 320.0)
        .resizable(false)
        .always_on_top(true)
        .center()
        .build();

        if let Err(err) = built {
            tracing::warn!(%err, "mesaj penceresi açılamadı");
        }
    }

    /// Açık bağlantı üzerinden öğretmene mesaj gönderir.
    ///
    /// Kanal kopyası kilit altında alınır ve kilit `await` öncesinde
    /// bırakılır; `parking_lot` guard'ı `Send` değildir.
    pub async fn send_to_teacher(&self, msg: ClientMessage) -> anyhow::Result<()> {
        let sender = self.outbox.read().clone();
        let Some(sender) = sender else {
            anyhow::bail!("öğretmen konsoluna bağlı değilsiniz");
        };
        sender
            .send(msg)
            .await
            .map_err(|_| anyhow::anyhow!("bağlantı kapandı"))?;
        Ok(())
    }

    /// Eşleşilmiş öğretmen varsa bağlanılacak adresi döner.
    ///
    /// Öncelik sırası:
    /// 1. Arayüzden elle girilmiş adres,
    /// 2. Keşifte görülen güncel adres (IP değişmiş olabilir),
    /// 3. Yapılandırmada saklı son adres.
    pub fn connect_target(&self) -> Option<ConnectTarget> {
        let cfg = self.config.read();
        let paired = cfg.paired_teacher.as_ref()?;

        if let Some(manual) = cfg.manual_teacher_address.as_ref() {
            if let Some((host, port)) = parse_address(manual, paired.port) {
                return Some(ConnectTarget {
                    host,
                    port,
                    fingerprint: paired.cert_fingerprint.clone(),
                });
            }
        }

        if let Some(seen) = self.teachers.read().get(&paired.teacher_id) {
            return Some(ConnectTarget {
                host: seen.host.clone(),
                port: seen.control_port,
                fingerprint: paired.cert_fingerprint.clone(),
            });
        }

        Some(ConnectTarget {
            host: paired.host.clone(),
            port: paired.port,
            fingerprint: paired.cert_fingerprint.clone(),
        })
    }
}

/// Bağlanılacak öğretmen konsolu.
#[derive(Debug, Clone)]
pub struct ConnectTarget {
    pub host: String,
    pub port: u16,
    pub fingerprint: Vec<u8>,
}

/// `ip` veya `ip:port` biçimindeki metni ayrıştırır.
pub fn parse_address(input: &str, default_port: u16) -> Option<(String, u16)> {
    let text = input.trim();
    if text.is_empty() {
        return None;
    }
    // IPv6 köşeli parantezli biçim: [::1]:7643
    if let Some(rest) = text.strip_prefix('[') {
        let (host, tail) = rest.split_once(']')?;
        let port = tail
            .strip_prefix(':')
            .and_then(|p| p.parse().ok())
            .unwrap_or(default_port);
        return Some((host.to_string(), port));
    }
    match text.rsplit_once(':') {
        Some((host, port)) if !host.is_empty() => match port.parse::<u16>() {
            Ok(p) => Some((host.to_string(), p)),
            // İki nokta var ama port değilse muhtemelen çıplak IPv6'dır.
            Err(_) => Some((text.to_string(), default_port)),
        },
        _ => Some((text.to_string(), default_port)),
    }
}

/// Unix zaman damgası (saniye).
fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod arayuz_bicimi_testleri {
    //! `ConnectionStatus`'un arayüze giden biçimini kilitler.
    //!
    //! `apps/student/src/types.ts` bu alanları `camelCase` okur. Serde'de
    //! `rename_all` yalnızca varyant adlarını çevirir; alanlar için
    //! `rename_all_fields` gerekir. Eksikse arayüz `undefined` görür ve
    //! hiçbir derleyici bunu yakalamaz.

    use super::*;

    #[test]
    fn bagli_durumu_camelcase_uretir() {
        let json = serde_json::to_value(ConnectionStatus::Connected {
            teacher_name: "Test Öğretmen".into(),
        })
        .unwrap();

        assert_eq!(json["kind"], "connected");
        assert_eq!(
            json["teacherName"], "Test Öğretmen",
            "arayüz teacherName okuyor; üretilen: {json}"
        );
        assert!(
            json.get("teacher_name").is_none(),
            "snake_case alan kalmamalı: {json}"
        );
    }

    #[test]
    fn baglanti_yok_durumu_camelcase_uretir() {
        let json = serde_json::to_value(ConnectionStatus::Disconnected {
            reason: Some("ağ yok".into()),
            retry_in_secs: 5,
        })
        .unwrap();

        assert_eq!(json["kind"], "disconnected");
        assert_eq!(json["retryInSecs"], 5, "üretilen: {json}");
        assert!(json.get("retry_in_secs").is_none());
    }

    #[test]
    fn alansiz_varyantlar_camelcase_adlanir() {
        for (durum, beklenen) in [
            (ConnectionStatus::NotPaired, "notPaired"),
            (ConnectionStatus::Connecting, "connecting"),
            (ConnectionStatus::Pairing, "pairing"),
        ] {
            let json = serde_json::to_value(durum).unwrap();
            assert_eq!(json["kind"], beklenen);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adres_portsuz_girilince_varsayilan_port_kullanilir() {
        assert_eq!(
            parse_address("192.168.1.10", 7643),
            Some(("192.168.1.10".into(), 7643))
        );
    }

    #[test]
    fn adres_portlu_girilebilir() {
        assert_eq!(
            parse_address("192.168.1.10:9000", 7643),
            Some(("192.168.1.10".into(), 9000))
        );
    }

    #[test]
    fn alan_adi_kabul_edilir() {
        assert_eq!(
            parse_address("ogretmen-pc", 7643),
            Some(("ogretmen-pc".into(), 7643))
        );
    }

    #[test]
    fn ipv6_koseli_parantezle_ayristirilir() {
        assert_eq!(
            parse_address("[fe80::1]:7000", 7643),
            Some(("fe80::1".into(), 7000))
        );
        assert_eq!(
            parse_address("[fe80::1]", 7643),
            Some(("fe80::1".into(), 7643))
        );
    }

    #[test]
    fn bos_adres_reddedilir() {
        assert_eq!(parse_address("   ", 7643), None);
    }
}
