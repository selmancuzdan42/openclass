//! Öğretmen konsolunun çalışma anı durumu.
//!
//! Üç ayrı bilgi kümesi bilinçli olarak ayrı tutulur:
//!
//! * **Keşfedilen** cihazlar — UDP ile kendini duyuranlar. Doğrulanmamış.
//! * **Bağlı** cihazlar — TLS oturumu açmış ve kimliği doğrulanmış olanlar.
//! * **Kayıtlı** cihazlar — veritabanındaki eşleştirilmiş cihazlar ([`crate::db`]).
//!
//! Bir cihazın ekranını görmek için üçünün de sağlanması gerekir.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use openclass_protocol::control::{ServerMessage, StreamQuality};
use parking_lot::RwLock;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::db::{now, Db};

/// Keşif cevabı vermiş ama henüz bağlanmamış olabilecek cihaz.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DiscoveredDevice {
    pub device_id: Uuid,
    /// Cihazın beyan ettiği bilgisayar adı. **Doğrulanmamıştır.**
    pub hostname: String,
    pub address: String,
    pub app_version: String,
    pub last_seen: i64,
    /// Cihaz başka bir öğretmene mi eşleşmiş?
    pub paired_to_us: bool,
}

/// Bağlı ve kimliği doğrulanmış cihaz.
pub struct ConnectedDevice {
    pub device_id: Uuid,
    pub hostname: String,
    pub address: SocketAddr,
    pub connected_at: i64,
    /// Cihaza mesaj göndermek için kanal. Dolu kanal = yavaş istemci.
    pub sender: mpsc::Sender<ServerMessage>,
}

/// Bir cihazın arayüze dönen birleşik görünümü.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceView {
    pub device_id: Uuid,
    pub hostname: String,
    pub display_name: Option<String>,
    /// `authorized` | `pending` | `rejected` | `unknown`
    pub authorization: String,
    /// Şu anda TLS oturumu açık mı?
    pub online: bool,
    /// Yalnızca keşifte görüldü, bağlanmadı.
    pub discovered: bool,
    pub address: Option<String>,
    pub last_seen: i64,
    pub streaming: bool,
    pub help_requested: bool,
    /// Son alınan karenin sıra numarası. Arayüz görüntüyü bununla tazeler.
    pub frame_seq: u64,
    /// Son karenin çözünürlüğü (yayın gerçekten akıyor mu, hangi kalitede?).
    pub frame_width: u32,
    pub frame_height: u32,
    /// Son kare üzerinden geçen süre. Büyürse yayın donmuş demektir.
    pub frame_age_ms: u64,
    /// Cihazın bağlandığı an (Unix saniye); bağlı değilse 0.
    pub connected_at: i64,
    /// Ekran yakalama hatası varsa Türkçe açıklaması.
    pub screen_error: Option<String>,
}

/// Aktif ders oturumu.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassSession {
    pub session_id: Uuid,
    pub class_name: String,
    pub started_at: i64,
}

/// Ekranda gösterilen, kısa ömürlü eşleştirme kodu.
pub struct ActivePairing {
    pub code: String,
    pub expires_at: std::time::Instant,
    /// Hatalı deneme sayacı. Aşılınca kod iptal edilir.
    pub failed_attempts: u8,
}

/// Eşleştirme kodunun geçerlilik süresi.
pub const PAIRING_CODE_TTL: std::time::Duration = std::time::Duration::from_secs(5 * 60);

/// Kod iptal edilmeden önceki azami hatalı deneme.
///
/// 6 haneli kod için 10^6 olasılık var; 5 deneme ile kaba kuvvet
/// başarı olasılığı ihmal edilebilir seviyede kalır.
pub const PAIRING_MAX_ATTEMPTS: u8 = 5;

impl ActivePairing {
    pub fn new(code: String) -> Self {
        Self {
            code,
            expires_at: std::time::Instant::now() + PAIRING_CODE_TTL,
            failed_attempts: 0,
        }
    }

    pub fn is_valid(&self) -> bool {
        self.failed_attempts < PAIRING_MAX_ATTEMPTS && std::time::Instant::now() < self.expires_at
    }

    /// Kalan saniye (arayüzde geri sayım için).
    pub fn remaining_secs(&self) -> u64 {
        self.expires_at
            .saturating_duration_since(std::time::Instant::now())
            .as_secs()
    }
}

/// Cihaz başına tutulan geçici durum bayrakları.
#[derive(Debug, Default, Clone)]
pub struct DeviceFlags {
    pub streaming: bool,
    pub quality: Option<StreamQuality>,
    pub help_requested: bool,
    pub screen_error: Option<String>,
}

/// Tüm çalışma anı durumu.
pub struct Registry {
    discovered: RwLock<HashMap<Uuid, DiscoveredDevice>>,
    connected: RwLock<HashMap<Uuid, ConnectedDevice>>,
    flags: RwLock<HashMap<Uuid, DeviceFlags>>,
    screens: RwLock<HashMap<Uuid, openclass_screen::ScreenBuffer>>,
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}

impl Registry {
    pub fn new() -> Self {
        Self {
            discovered: RwLock::new(HashMap::new()),
            connected: RwLock::new(HashMap::new()),
            flags: RwLock::new(HashMap::new()),
            screens: RwLock::new(HashMap::new()),
        }
    }

    // ------------------------------------------------------------------ keşif

    pub fn record_discovery(&self, device: DiscoveredDevice) {
        self.discovered.write().insert(device.device_id, device);
    }

    /// Belirtilen süreden uzun süredir görülmeyen keşif kayıtlarını siler.
    pub fn prune_discovered(&self, max_age_secs: i64) {
        let cutoff = now() - max_age_secs;
        self.discovered.write().retain(|_, d| d.last_seen >= cutoff);
    }

    /// Keşif listesinin kopyası.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn discovered_devices(&self) -> Vec<DiscoveredDevice> {
        self.discovered.read().values().cloned().collect()
    }

    // --------------------------------------------------------------- bağlantı

    /// Cihazı bağlı olarak işaretler. Aynı cihazın önceki oturumu varsa onu döner
    /// (çağıran taraf eski oturumu kapatmalıdır).
    pub fn attach(&self, device: ConnectedDevice) -> Option<ConnectedDevice> {
        self.connected.write().insert(device.device_id, device)
    }

    /// Cihazın bağlantısını kaydı siler ve tüm geçici durumunu temizler.
    ///
    /// Bağlantı koptuğunda sistem güvenli duruma döner: yayın bayrağı düşer,
    /// son kare atılır, yardım talebi korunur (öğretmen henüz görmemiş olabilir).
    pub fn detach(&self, device_id: Uuid) -> Option<ConnectedDevice> {
        let removed = self.connected.write().remove(&device_id);
        self.screens.write().remove(&device_id);
        if let Some(flags) = self.flags.write().get_mut(&device_id) {
            flags.streaming = false;
            flags.quality = None;
            flags.screen_error = None;
        }
        removed
    }

    /// Cihazın şu anda TLS oturumu açık mı?
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn is_online(&self, device_id: Uuid) -> bool {
        self.connected.read().contains_key(&device_id)
    }

    pub fn online_count(&self) -> usize {
        self.connected.read().len()
    }

    /// Cihaza mesaj göndermek için kanalın bir kopyasını verir.
    pub fn sender(&self, device_id: Uuid) -> Option<mpsc::Sender<ServerMessage>> {
        self.connected
            .read()
            .get(&device_id)
            .map(|c| c.sender.clone())
    }

    /// Tüm bağlı cihazların kanalları.
    pub fn all_senders(&self) -> Vec<(Uuid, mpsc::Sender<ServerMessage>)> {
        self.connected
            .read()
            .iter()
            .map(|(id, c)| (*id, c.sender.clone()))
            .collect()
    }

    // ----------------------------------------------------------------- durum

    pub fn update_flags(&self, device_id: Uuid, f: impl FnOnce(&mut DeviceFlags)) {
        let mut guard = self.flags.write();
        f(guard.entry(device_id).or_default());
    }

    pub fn flags(&self, device_id: Uuid) -> DeviceFlags {
        self.flags
            .read()
            .get(&device_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Tüm yayınları durdurulmuş sayar (ders bitişinde).
    pub fn clear_all_streaming(&self) {
        for flags in self.flags.write().values_mut() {
            flags.streaming = false;
            flags.quality = None;
        }
        self.screens.write().clear();
    }

    // ------------------------------------------------------------ ekran verisi

    /// Yeni ekran güncellemesini ekler.
    ///
    /// Dönen değer, öğrenciden yeni bir anahtar kare istenmesi gerekip
    /// gerekmediğini söyler.
    pub fn push_screen_update(
        &self,
        device_id: Uuid,
        update: openclass_screen::StoredUpdate,
        width: u32,
        height: u32,
    ) -> bool {
        let mut guard = self.screens.write();
        let state = guard.entry(device_id).or_default();
        state.push(update, width, height);
        state.needs_keyframe()
    }

    /// Bu cihazdan ilk kez ekran verisi mi geliyor?
    ///
    /// İlk gelişi bir kez loglayabilmek için; sonraki çağrılarda `false`.
    pub fn first_screen_update(&self, device_id: Uuid) -> bool {
        !self.screens.read().contains_key(&device_id)
    }

    pub fn clear_keyframe_request(&self, device_id: Uuid) {
        if let Some(state) = self.screens.write().get_mut(&device_id) {
            state.clear_keyframe_request();
        }
    }

    /// Arayüzün `since` sırasından sonrasını ikili paket olarak döner.
    pub fn screen_blob(&self, device_id: Uuid, since: u64) -> Option<Vec<u8>> {
        self.screens.read().get(&device_id)?.encode_since(since)
    }

    // ------------------------------------------------------------- birleşik görünüm

    /// Kayıtlı, keşfedilen ve bağlı cihazları tek listede birleştirir.
    ///
    /// Aynı cihaz birden çok kaynakta görünürse tek satırda toplanır.
    /// Veritabanında olmayan ama ağda görünen cihazlar `unknown` yetkisiyle
    /// listelenir — öğretmen onları eşleştirmeye davet edebilir.
    pub fn build_views(&self, db: &Db) -> anyhow::Result<Vec<DeviceView>> {
        let mut views: HashMap<Uuid, DeviceView> = HashMap::new();

        for rec in db.all_devices()? {
            views.insert(
                rec.device_id,
                DeviceView {
                    device_id: rec.device_id,
                    hostname: rec.hostname,
                    display_name: rec.display_name,
                    authorization: crate::db::status_to_str(rec.status).to_string(),
                    online: false,
                    discovered: false,
                    address: None,
                    last_seen: rec.last_seen,
                    streaming: false,
                    help_requested: false,
                    frame_seq: 0,
                    frame_width: 0,
                    frame_height: 0,
                    frame_age_ms: 0,
                    connected_at: 0,
                    screen_error: None,
                },
            );
        }

        for d in self.discovered.read().values() {
            let view = views.entry(d.device_id).or_insert_with(|| DeviceView {
                device_id: d.device_id,
                hostname: d.hostname.clone(),
                display_name: None,
                authorization: "unknown".to_string(),
                online: false,
                discovered: false,
                address: None,
                last_seen: d.last_seen,
                streaming: false,
                help_requested: false,
                frame_seq: 0,
                frame_width: 0,
                frame_height: 0,
                frame_age_ms: 0,
                connected_at: 0,
                screen_error: None,
            });
            view.discovered = true;
            view.address = Some(d.address.clone());
            view.last_seen = view.last_seen.max(d.last_seen);
            // Ağdaki ad, kayıttaki addan tazedir.
            view.hostname = d.hostname.clone();
        }

        for c in self.connected.read().values() {
            if let Some(view) = views.get_mut(&c.device_id) {
                view.online = true;
                view.address = Some(c.address.to_string());
                view.hostname = c.hostname.clone();
                view.connected_at = c.connected_at;
            }
        }

        {
            let flags = self.flags.read();
            let screens = self.screens.read();
            for (id, view) in views.iter_mut() {
                if let Some(f) = flags.get(id) {
                    view.streaming = f.streaming;
                    view.help_requested = f.help_requested;
                    view.screen_error = f.screen_error.clone();
                }
                if let Some(screen) = screens.get(id) {
                    view.frame_seq = screen.latest_seq;
                    view.frame_width = screen.width;
                    view.frame_height = screen.height;
                    view.frame_age_ms = screen.received_at.elapsed().as_millis() as u64;
                }
            }
        }

        let mut out: Vec<_> = views.into_values().collect();
        out.sort_by(|a, b| a.hostname.cmp(&b.hostname));
        Ok(out)
    }
}

// ---------------------------------------------------------------------------
// Uygulama durumu
// ---------------------------------------------------------------------------

use crate::config::TeacherConfig;
use openclass_common::tls::TlsIdentity;

/// Tauri komutlarının ve ağ görevlerinin paylaştığı kök durum.
pub struct AppState {
    pub teacher_id: Uuid,
    pub tls: Arc<TlsIdentity>,
    pub db: Arc<Db>,
    pub registry: Arc<Registry>,
    pub config: RwLock<TeacherConfig>,
    pub config_path: std::path::PathBuf,
    /// Ekranda gösterilen, kısa ömürlü eşleştirme kodu.
    pub pairing: RwLock<Option<ActivePairing>>,
    /// Testte kullanılan sabit eşleştirme kodu (yalnızca hata ayıklama
    /// derlemesinde dolu olur; bkz. [`crate::testing`]).
    pub test_pairing_code: Option<String>,
    /// Aktif ders oturumu. `None` ise hiçbir ekran yayını başlatılamaz.
    pub session: RwLock<Option<ClassSession>>,
    /// Arayüze olay göndermek için Tauri tutamacı.
    pub app: RwLock<Option<tauri::AppHandle>>,
    /// Multicast paketleri ulaşmayan cihazlar; bunlara TLS unicast yapılır.
    pub broadcast_fallback: RwLock<std::collections::HashSet<Uuid>>,
    /// Açık uzak dosya gezgini oturumu; aynı anda tek öğrenci gezilir.
    pub browse: RwLock<Option<crate::browse::BrowseSession>>,
    /// Yürüyen ve biten dosya gönderimleri.
    pub transfers: RwLock<HashMap<Uuid, crate::transfer::TransferView>>,
    /// Uzaktan kontrol edilen cihaz; aynı anda yalnızca bir tane olabilir.
    pub controlled_device: RwLock<Option<Uuid>>,
    /// Çalışan öğretmen ekranı yayını.
    pub broadcast: RwLock<Option<crate::broadcast::BroadcastHandle>>,
    /// Bir öğrenci yayında eksik parça bildirdi; sonraki kare anahtar olmalı.
    pub(crate) broadcast_keyframe_requested: std::sync::atomic::AtomicBool,
    /// Ağ servisleri başlatılamadıysa sebebi.
    ///
    /// Geçici bir bildirim yetmiyor: kontrol sunucusu bağlanamadığında konsol
    /// sağlıklı görünmeye devam ediyor, öğrenciler ise anlaşılmaz hatalar
    /// alıyor. Bu yüzden durum kalıcı tutulur ve arayüzde sürekli gösterilir.
    pub network_error: RwLock<Option<String>>,
    /// Çalışan ağ görevlerini durdurmak için.
    pub cancel: tokio_util::sync::CancellationToken,
}

impl AppState {
    /// Yapılandırmayı diske yazar.
    pub fn save_config(&self) -> anyhow::Result<()> {
        self.config.read().save(&self.config_path)
    }

    /// Şu an aktif bir ders oturumu var mı?
    /// Yayın durumunu arayüze yeniden bildirir.
    ///
    /// TLS yedeğine düşen cihaz sayısı değiştiğinde çağrılır.
    pub fn emit_broadcast(&self) {
        let guard = self.broadcast.read();
        let Some(handle) = guard.as_ref() else { return };
        let info = serde_json::json!({
            "broadcastId": handle.broadcast_id.to_string(),
            "startedAt": handle.started_at,
            "fullscreen": handle.fullscreen,
            "multicast": handle.multicast.is_some(),
            "fallbackCount": self.broadcast_fallback.read().len(),
        });
        drop(guard);
        self.emit("broadcast-changed", info);
    }

    /// Dosya gönderim listesini arayüze bildirir.
    ///
    /// Tamamı gönderilir; liste kısa ve arayüz tarafı farkı kendi hesaplar.
    pub fn emit_transfers(&self) {
        let list: Vec<crate::transfer::TransferView> =
            self.transfers.read().values().cloned().collect();
        self.emit("transfers-changed", list);
    }

    pub fn session_active(&self) -> bool {
        self.session.read().is_some()
    }

    /// Arayüze olay yayar. Pencere henüz yoksa sessizce atlanır.
    pub fn emit<T: serde::Serialize + Clone>(&self, event: &str, payload: T) {
        use tauri::Emitter;
        if let Some(app) = self.app.read().as_ref() {
            if let Err(err) = app.emit(event, payload) {
                tracing::warn!(%err, event, "arayüze olay gönderilemedi");
            }
        }
    }

    /// Cihaz listesinin değiştiğini arayüze bildirir.
    pub fn notify_devices_changed(&self) {
        self.emit("devices-changed", ());
    }

    /// Kalıcı ağ hatasını ayarlar ve arayüzü bilgilendirir.
    pub fn set_network_error(&self, message: impl Into<String>) {
        let message = message.into();
        *self.network_error.write() = Some(message.clone());
        self.emit("network-error", message);
    }

    /// Bir öğrenci yayında eksik parça bildirdiğinde çağrılır.
    pub fn request_broadcast_keyframe(&self) {
        self.broadcast_keyframe_requested
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    /// Bekleyen anahtar kare isteğini alır ve sıfırlar.
    pub fn take_broadcast_keyframe_request(&self) -> bool {
        self.broadcast_keyframe_requested
            .swap(false, std::sync::atomic::Ordering::Relaxed)
    }

    /// Yayın şu anda çalışıyor mu?
    pub fn broadcasting(&self) -> bool {
        self.broadcast.read().is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ornek_guncelleme(seq: u64, keyframe: bool) -> openclass_screen::StoredUpdate {
        let data = vec![0xAB_u8; 256];
        openclass_screen::StoredUpdate {
            seq,
            keyframe,
            bytes: data.len(),
            rects: vec![openclass_protocol::control::ScreenRect {
                x: 0,
                y: 0,
                width: 64,
                height: 64,
                data,
            }],
        }
    }

    fn kesfedilen(hostname: &str) -> DiscoveredDevice {
        DiscoveredDevice {
            device_id: Uuid::new_v4(),
            hostname: hostname.to_string(),
            address: "192.168.1.50:7644".to_string(),
            app_version: "0.1.0".to_string(),
            last_seen: now(),
            paired_to_us: false,
        }
    }

    #[test]
    fn kesfedilen_cihaz_kaydedilir() {
        let reg = Registry::new();
        reg.record_discovery(kesfedilen("LAB101"));
        assert_eq!(reg.discovered_devices().len(), 1);
    }

    #[test]
    fn ayni_cihaz_iki_kez_duyurursa_tek_kayit_kalir() {
        let reg = Registry::new();
        let mut d = kesfedilen("LAB101");
        reg.record_discovery(d.clone());
        d.address = "192.168.1.77:7644".to_string();
        reg.record_discovery(d);
        let all = reg.discovered_devices();
        assert_eq!(all.len(), 1);
        assert_eq!(
            all[0].address, "192.168.1.77:7644",
            "IP değişimi güncellenmeli"
        );
    }

    #[test]
    fn eski_kesif_kayitlari_temizlenir() {
        let reg = Registry::new();
        let mut eski = kesfedilen("LAB101");
        eski.last_seen = now() - 600;
        reg.record_discovery(eski);
        reg.record_discovery(kesfedilen("LAB102"));

        reg.prune_discovered(60);
        let kalan = reg.discovered_devices();
        assert_eq!(kalan.len(), 1);
        assert_eq!(kalan[0].hostname, "LAB102");
    }

    #[test]
    fn baglanti_kopunca_yayin_durur_ve_kare_atilir() {
        let reg = Registry::new();
        let id = Uuid::new_v4();
        let (tx, _rx) = mpsc::channel(8);
        reg.attach(ConnectedDevice {
            device_id: id,
            hostname: "LAB101".into(),
            address: "192.168.1.50:50000".parse().unwrap(),
            connected_at: now(),
            sender: tx,
        });
        reg.update_flags(id, |f| f.streaming = true);
        reg.push_screen_update(id, ornek_guncelleme(1, true), 100, 100);
        assert!(reg.is_online(id));

        reg.detach(id);
        assert!(!reg.is_online(id));
        assert!(
            !reg.flags(id).streaming,
            "bağlantı kopunca yayın bayrağı düşmeli"
        );
        assert!(
            reg.screen_blob(id, 0).is_none(),
            "bağlantı kopunca eski ekran verisi atılmalı"
        );
    }

    #[test]
    fn anahtar_kare_birikimi_sifirlar() {
        let reg = Registry::new();
        let id = Uuid::new_v4();
        reg.push_screen_update(id, ornek_guncelleme(1, true), 640, 480);
        for seq in 2..20 {
            reg.push_screen_update(id, ornek_guncelleme(seq, false), 640, 480);
        }
        // Yeni anahtar kare geldiğinde arayüz tek pakette toparlanabilmeli.
        reg.push_screen_update(id, ornek_guncelleme(20, true), 640, 480);
        let blob = reg.screen_blob(id, 0).unwrap();
        assert_eq!(&blob[..4], b"OCU1");
        assert_eq!(blob[4], 1, "görüntüsü olmayan arayüze anahtar kare gitmeli");
    }

    #[test]
    fn guncel_arayuze_bos_doner() {
        let reg = Registry::new();
        let id = Uuid::new_v4();
        reg.push_screen_update(id, ornek_guncelleme(5, true), 640, 480);
        assert!(
            reg.screen_blob(id, 5).is_none(),
            "arayüz güncelse yeniden veri gönderilmemeli"
        );
    }

    #[test]
    fn ders_bitince_tum_yayinlar_durur() {
        let reg = Registry::new();
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        reg.update_flags(a, |f| f.streaming = true);
        reg.update_flags(b, |f| f.streaming = true);
        reg.clear_all_streaming();
        assert!(!reg.flags(a).streaming);
        assert!(!reg.flags(b).streaming);
    }

    #[test]
    fn eslestirme_kodu_denemeler_tukenince_gecersizlesir() {
        let mut p = ActivePairing::new("123456".into());
        assert!(p.is_valid());
        p.failed_attempts = PAIRING_MAX_ATTEMPTS;
        assert!(!p.is_valid(), "deneme hakkı bitince kod iptal olmalı");
    }

    #[test]
    fn eslestirme_kodu_suresi_dolunca_gecersizlesir() {
        let mut p = ActivePairing::new("123456".into());
        p.expires_at = std::time::Instant::now() - std::time::Duration::from_secs(1);
        assert!(!p.is_valid());
        assert_eq!(p.remaining_secs(), 0);
    }
}
