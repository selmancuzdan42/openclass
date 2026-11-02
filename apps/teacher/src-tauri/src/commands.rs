//! Arayüzün (React) çağırdığı Tauri komutları.
//!
//! Komutlar ince tutulur: doğrulama ve iş mantığı [`crate::state`],
//! [`crate::db`] ve [`crate::net`] katmanlarına aittir. Buradaki her komut
//! kullanıcıya **Türkçe** hata mesajı döndürür.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use openclass_protocol::control::{FileDestination, ServerMessage, StreamQuality};
use tauri::State;
use uuid::Uuid;

use crate::db::{AuditEntry, AuditKind};
use crate::state::{ActivePairing, AppState, DeviceView};

/// Komut hatası. Arayüze düz metin olarak iner.
pub type CmdResult<T> = Result<T, String>;

/// `anyhow::Error`'u kullanıcıya gösterilebilir Türkçe metne çevirir.
fn fail(context: &str, err: impl std::fmt::Display) -> String {
    tracing::error!(%err, context, "komut başarısız");
    format!("{context}: {err}")
}

fn parse_id(raw: &str) -> CmdResult<Uuid> {
    Uuid::parse_str(raw).map_err(|_| format!("Geçersiz cihaz kimliği: {raw}"))
}

// ---------------------------------------------------------------- cihazlar

/// Arama kutusundaki metne göre cihazları filtreler.
///
/// Boş sorgu tüm cihazları döndürür. Sorgu hem bilgisayar adında (ön ek,
/// büyük/küçük harf duyarsız: `LAB1` → `LAB101`…) hem IP adresinde aranır;
/// öğretmen hangisini yazdığını belirtmek zorunda değil.
#[tauri::command]
pub fn search_devices(
    state: State<'_, Arc<AppState>>,
    query: String,
) -> CmdResult<Vec<DeviceView>> {
    let all = state
        .registry
        .build_views(&state.db)
        .map_err(|e| fail("Cihaz listesi okunamadı", e))?;
    Ok(all
        .into_iter()
        .filter(|d| {
            openclass_protocol::discovery::matches_device(&query, &d.hostname, d.address.as_deref())
        })
        .collect())
}

/// Cihaza öğretmenin verdiği görünen adı atar.
#[tauri::command]
pub fn set_display_name(
    state: State<'_, Arc<AppState>>,
    device_id: String,
    name: Option<String>,
) -> CmdResult<()> {
    let id = parse_id(&device_id)?;
    let trimmed = name.map(|n| n.trim().to_string()).filter(|n| !n.is_empty());
    state
        .db
        .set_display_name(id, trimmed.as_deref())
        .map_err(|e| fail("Görünen ad kaydedilemedi", e))?;
    state.notify_devices_changed();
    Ok(())
}

/// Cihazın yetkisini kaldırır ve bağlıysa bağlantısını keser.
#[tauri::command]
pub async fn revoke_device(state: State<'_, Arc<AppState>>, device_id: String) -> CmdResult<()> {
    let id = parse_id(&device_id)?;
    state
        .db
        .revoke_device(id)
        .map_err(|e| fail("Yetki kaldırılamadı", e))?;
    state.db.audit(AuditKind::DeviceRevoked, Some(id), None);

    // Bağlıysa yayınını durdur ve oturumunu kapat.
    if let Some(tx) = state.registry.sender(id) {
        let _ = tx
            .send(ServerMessage::StopScreenStream { stream_id: 0 })
            .await;
        let _ = tx
            .send(ServerMessage::AuthResult {
                ok: false,
                reason: Some("Bu cihazın yetkisi kaldırıldı.".into()),
                // Öğrenci kayıtlı eşleştirmesini silsin: geçersiz kimlikle
                // sonsuza kadar yeniden bağlanmayı denemesin.
                reset_pairing: true,
            })
            .await;
    }
    state.registry.detach(id);
    state.notify_devices_changed();
    Ok(())
}

// ------------------------------------------------------------- eşleştirme

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PairingInfo {
    pub code: String,
    pub remaining_secs: u64,
    pub max_attempts: u8,
    pub failed_attempts: u8,
}

/// Yeni bir eşleştirme kodu üretir ve ekranda gösterilmek üzere döndürür.
///
/// Kod tek kullanımlıktır, süresi doludur ve hatalı deneme sayısı sınırlıdır.
#[tauri::command]
pub fn start_pairing(state: State<'_, Arc<AppState>>) -> CmdResult<PairingInfo> {
    let code = openclass_protocol::auth::generate_pairing_code();
    let active = ActivePairing::new(code.clone());
    let info = PairingInfo {
        code,
        remaining_secs: active.remaining_secs(),
        max_attempts: crate::state::PAIRING_MAX_ATTEMPTS,
        failed_attempts: 0,
    };
    *state.pairing.write() = Some(active);
    tracing::info!("eşleştirme kodu üretildi");
    Ok(info)
}

/// Etkin eşleştirme kodunu iptal eder.
#[tauri::command]
pub fn cancel_pairing(state: State<'_, Arc<AppState>>) -> CmdResult<()> {
    *state.pairing.write() = None;
    Ok(())
}

/// Etkin eşleştirme kodunun güncel durumu (geri sayım için).
#[tauri::command]
pub fn get_pairing(state: State<'_, Arc<AppState>>) -> CmdResult<Option<PairingInfo>> {
    let mut guard = state.pairing.write();
    let Some(active) = guard.as_ref() else {
        return Ok(None);
    };
    if !active.is_valid() {
        *guard = None;
        return Ok(None);
    }
    Ok(Some(PairingInfo {
        code: active.code.clone(),
        remaining_secs: active.remaining_secs(),
        max_attempts: crate::state::PAIRING_MAX_ATTEMPTS,
        failed_attempts: active.failed_attempts,
    }))
}


// ----------------------------------------------------------- ekran yayını

/// Arayüzden gelen kalite seçimi.
#[derive(serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QualityKind {
    /// Grid önizlemesi.
    Preview,
    /// Büyütülmüş tek ekran.
    Focused,
}

impl From<QualityKind> for StreamQuality {
    fn from(k: QualityKind) -> Self {
        match k {
            QualityKind::Preview => StreamQuality::PREVIEW,
            QualityKind::Focused => StreamQuality::FOCUSED,
        }
    }
}

/// Seçili cihazların ekran yayınını başlatır.
///
/// Yayın öğrencide **her zaman görünür**: tepsi simgesi değişir ve
/// "EKRANINIZ PAYLAŞILIYOR" göstergesi çıkar.
#[tauri::command]
pub async fn start_screen_streams(
    state: State<'_, Arc<AppState>>,
    device_ids: Vec<String>,
    quality: QualityKind,
) -> CmdResult<u32> {
    let quality: StreamQuality = quality.into();
    let mut started = 0u32;

    for raw in device_ids {
        let id = parse_id(&raw)?;
        let Some(tx) = state.registry.sender(id) else {
            continue;
        };
        if tx
            .send(ServerMessage::StartScreenStream {
                stream_id: 1,
                quality,
            })
            .await
            .is_ok()
        {
            state.registry.update_flags(id, |f| {
                f.streaming = true;
                f.quality = Some(quality);
            });
            state
                .db
                .audit(AuditKind::ScreenStreamStarted, Some(id), None);
            started += 1;
        }
    }

    state.notify_devices_changed();
    Ok(started)
}

/// Seçili cihazların ekran yayınını durdurur.
#[tauri::command]
pub async fn stop_screen_streams(
    state: State<'_, Arc<AppState>>,
    device_ids: Vec<String>,
) -> CmdResult<()> {
    for raw in device_ids {
        let id = parse_id(&raw)?;
        if let Some(tx) = state.registry.sender(id) {
            let _ = tx
                .send(ServerMessage::StopScreenStream { stream_id: 1 })
                .await;
        }
        state.registry.update_flags(id, |f| {
            f.streaming = false;
            f.quality = None;
        });
        state
            .db
            .audit(AuditKind::ScreenStreamStopped, Some(id), None);
    }
    state.notify_devices_changed();
    Ok(())
}

/// Bir ekranı büyütür: o cihaz yüksek kaliteye, diğerleri önizlemeye geçer.
#[tauri::command]
pub async fn focus_device(state: State<'_, Arc<AppState>>, device_id: String) -> CmdResult<()> {
    let focused = parse_id(&device_id)?;

    for (id, tx) in state.registry.all_senders() {
        let flags = state.registry.flags(id);
        if !flags.streaming && id != focused {
            continue;
        }
        let quality = if id == focused {
            StreamQuality::FOCUSED
        } else {
            StreamQuality::PREVIEW
        };
        let msg = if flags.streaming {
            ServerMessage::UpdateStreamQuality {
                stream_id: 1,
                quality,
            }
        } else {
            ServerMessage::StartScreenStream {
                stream_id: 1,
                quality,
            }
        };
        if tx.send(msg).await.is_ok() {
            state.registry.update_flags(id, |f| {
                f.streaming = true;
                f.quality = Some(quality);
            });
        }
    }

    state.notify_devices_changed();
    Ok(())
}

/// Büyütmeden çıkar: tüm yayınlar önizleme kalitesine döner.
#[tauri::command]
pub async fn unfocus_devices(state: State<'_, Arc<AppState>>) -> CmdResult<()> {
    for (id, tx) in state.registry.all_senders() {
        if !state.registry.flags(id).streaming {
            continue;
        }
        let _ = tx
            .send(ServerMessage::UpdateStreamQuality {
                stream_id: 1,
                quality: StreamQuality::PREVIEW,
            })
            .await;
        state
            .registry
            .update_flags(id, |f| f.quality = Some(StreamQuality::PREVIEW));
    }
    Ok(())
}

// --------------------------------------------------------------- mesajlar

/// Seçili cihazlara mesaj gönderir.
#[tauri::command]
pub async fn send_notice(
    state: State<'_, Arc<AppState>>,
    device_ids: Vec<String>,
    title: String,
    body: String,
) -> CmdResult<u32> {
    let body = body.trim();
    if body.is_empty() {
        return Err("Mesaj boş olamaz.".into());
    }
    let title = if title.trim().is_empty() {
        state.config.read().teacher_name.clone()
    } else {
        title.trim().to_string()
    };

    let mut sent = 0u32;
    for raw in device_ids {
        let id = parse_id(&raw)?;
        let Some(tx) = state.registry.sender(id) else {
            continue;
        };
        let message_id = Uuid::new_v4();
        if tx
            .send(ServerMessage::Notice {
                message_id,
                title: title.clone(),
                body: body.to_string(),
            })
            .await
            .is_ok()
        {
            state.db.audit(AuditKind::NoticeSent, Some(id), None);
            sent += 1;
        }
    }

    if sent == 0 {
        return Err("Mesaj gönderilecek bağlı cihaz yok.".into());
    }
    Ok(sent)
}

/// Yardım talebini görüldü olarak işaretler.
#[tauri::command]
pub async fn clear_help_request(
    state: State<'_, Arc<AppState>>,
    device_id: String,
) -> CmdResult<()> {
    let id = parse_id(&device_id)?;
    state
        .registry
        .update_flags(id, |f| f.help_requested = false);
    if let Some(tx) = state.registry.sender(id) {
        let _ = tx.send(ServerMessage::HelpAcknowledged).await;
    }
    state.notify_devices_changed();
    Ok(())
}

// ------------------------------------------------------- ayarlar / durum

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerInfo {
    pub teacher_id: String,
    pub teacher_name: String,
    pub class_name: String,
    pub control_port: u16,
    pub discovery_port: u16,
    pub cert_fingerprint: String,
    /// Öğrencilerin elle girebileceği yerel IP adresleri.
    pub local_addresses: Vec<String>,
    pub online_count: usize,
    pub extra_discovery_hosts: Vec<String>,
    pub setup_completed: bool,
    /// Konsolun yönettiği laboratuvar; `None` ise bütün cihazlar listelenir.
    pub active_room: Option<String>,
    /// Konsolun süzdüğü bilgisayar adı ön eki.
    pub active_prefix: Option<String>,
    /// Ağ servisleri başlatılamadıysa sebebi; arayüz bunu kalıcı gösterir.
    pub network_error: Option<String>,
}

#[tauri::command]
pub fn get_server_info(state: State<'_, Arc<AppState>>) -> CmdResult<ServerInfo> {
    let cfg = state.config.read();
    Ok(ServerInfo {
        teacher_id: state.teacher_id.to_string(),
        teacher_name: cfg.teacher_name.clone(),
        class_name: cfg.class_name.clone(),
        control_port: cfg.control_port,
        discovery_port: openclass_protocol::DEFAULT_DISCOVERY_PORT,
        cert_fingerprint: state.tls.fingerprint_hex(),
        local_addresses: local_addresses(),
        online_count: state.registry.online_count(),
        extra_discovery_hosts: cfg.extra_discovery_hosts.clone(),
        setup_completed: cfg.setup_completed,
        active_room: cfg.active_room.clone(),
        active_prefix: cfg.active_prefix.clone(),
        network_error: state.network_error.read().clone(),
    })
}

/// Makinenin yerel IPv4 adreslerini listeler.
///
/// Öğrenci tarafında yayın engelliyse öğretmen bu adreslerden birini öğrenciye
/// söyler ve bağlantı doğrudan kurulur.
fn local_addresses() -> Vec<String> {
    match local_ip_address::list_afinet_netifas() {
        Ok(ifaces) => ifaces
            .into_iter()
            .filter_map(|(_, ip)| match ip {
                std::net::IpAddr::V4(v4) if !v4.is_loopback() && !v4.is_link_local() => {
                    Some(v4.to_string())
                }
                _ => None,
            })
            .collect(),
        Err(err) => {
            tracing::warn!(%err, "yerel IP adresleri listelenemedi");
            Vec::new()
        }
    }
}

/// Öğretmen adı, sınıf adı ve ek keşif hedeflerini günceller.
#[tauri::command]
pub fn update_settings(
    state: State<'_, Arc<AppState>>,
    teacher_name: Option<String>,
    class_name: Option<String>,
    extra_discovery_hosts: Option<Vec<String>>,
    setup_completed: Option<bool>,
    active_room: Option<String>,
    active_prefix: Option<String>,
) -> CmdResult<()> {
    {
        let mut cfg = state.config.write();
        if let Some(name) = teacher_name {
            let name = name.trim();
            if !name.is_empty() {
                cfg.teacher_name = name.to_string();
            }
        }
        if let Some(class) = class_name {
            let class = class.trim();
            if !class.is_empty() {
                cfg.class_name = class.to_string();
            }
        }
        if let Some(hosts) = extra_discovery_hosts {
            cfg.extra_discovery_hosts = hosts
                .into_iter()
                .map(|h| h.trim().to_string())
                .filter(|h| !h.is_empty())
                .collect();
        }
        if let Some(done) = setup_completed {
            cfg.setup_completed = done;
        }
        // Boş metin "bütün laboratuvarlar" demektir; `None` ile aynı sonucu
        // verir ama arayüzün seçimi açıkça temizlemesine izin verir.
        if let Some(room) = active_room {
            let room = room.trim();
            cfg.active_room = if room.is_empty() {
                None
            } else {
                Some(room.to_string())
            };
        }
        if let Some(prefix) = active_prefix {
            let prefix = prefix.trim().to_ascii_uppercase();
            cfg.active_prefix = if prefix.is_empty() {
                None
            } else {
                Some(prefix)
            };
        }
    }
    state
        .save_config()
        .map_err(|e| fail("Ayarlar kaydedilemedi", e))?;
    Ok(())
}

/// Son denetim kayıtları.
#[tauri::command]
pub fn get_audit_log(
    state: State<'_, Arc<AppState>>,
    limit: Option<u32>,
) -> CmdResult<Vec<AuditEntry>> {
    state
        .db
        .recent_audit(limit.unwrap_or(200).min(1000))
        .map_err(|e| fail("Denetim kayıtları okunamadı", e))
}

// ----------------------------------------------------------------- gruplar

/// Kayıtlı laboratuvar grupları.
#[tauri::command]
pub fn list_groups(state: State<'_, Arc<AppState>>) -> CmdResult<Vec<crate::config::DeviceGroup>> {
    Ok(state.config.read().groups.clone())
}

/// Yeni bir grup ekler veya aynı adlı grubu günceller.
#[tauri::command]
pub fn save_group(state: State<'_, Arc<AppState>>, name: String, prefix: String) -> CmdResult<()> {
    let name = name.trim().to_string();
    let prefix = prefix.trim().to_ascii_uppercase();
    if name.is_empty() {
        return Err("Grup adı boş olamaz.".into());
    }
    if prefix.is_empty() {
        return Err("Bilgisayar adı ön eki boş olamaz.".into());
    }

    {
        let mut cfg = state.config.write();
        match cfg
            .groups
            .iter_mut()
            .find(|g| g.name.eq_ignore_ascii_case(&name))
        {
            Some(existing) => existing.prefix = prefix,
            None => cfg.groups.push(crate::config::DeviceGroup { name, prefix }),
        }
        cfg.groups.sort_by(|a, b| a.name.cmp(&b.name));
    }
    state
        .save_config()
        .map_err(|e| fail("Grup kaydedilemedi", e))?;
    state.emit("groups-changed", ());
    Ok(())
}

/// Grubu siler. Cihazlar etkilenmez; grup yalnızca bir filtredir.
#[tauri::command]
pub fn delete_group(state: State<'_, Arc<AppState>>, name: String) -> CmdResult<()> {
    state
        .config
        .write()
        .groups
        .retain(|g| !g.name.eq_ignore_ascii_case(name.trim()));
    state
        .save_config()
        .map_err(|e| fail("Grup silinemedi", e))?;
    state.emit("groups-changed", ());
    Ok(())
}

/// Bilinen bilgisayar adlarından olası grup ön eklerini çıkarır.
///
/// Kural: addaki sondaki rakamlar atılır, kalan kısım aday ön ektir. `LAB101`
/// ve `LAB102` → `LAB`. Tek bir aday çıkarsa bu bir ayrım sağlamaz; bu yüzden
/// bir basamak daha korunarak (`LAB1`, `LAB2`) ikinci bir aday kümesi de
/// üretilir ve yalnızca en az iki cihazı kapsayanlar önerilir.
#[tauri::command]
pub fn suggest_groups(state: State<'_, Arc<AppState>>) -> CmdResult<Vec<GroupSuggestion>> {
    let devices = state
        .registry
        .build_views(&state.db)
        .map_err(|e| fail("Cihaz listesi okunamadı", e))?;
    let odalar: Vec<String> = devices.iter().filter_map(|d| d.room.clone()).collect();
    let hostnames: Vec<String> = devices.into_iter().map(|d| d.hostname).collect();

    let mut out = derive_room_suggestions(&odalar);
    out.extend(derive_group_suggestions(&hostnames));
    Ok(out)
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupSuggestion {
    /// Bilgisayar adı ön eki. Oda tabanlı öneride boştur.
    pub prefix: String,
    pub device_count: usize,
    /// Öneri bir odadan geliyorsa oda adı.
    ///
    /// Oda önerileri öncelikli: öğrencinin kendi bildirdiği değer, okulun
    /// bilgisayar adı disiplinine bağlı olmayan tek ölçüttür.
    pub room: Option<String>,
}

/// Bildirilen oda adlarından öneri üretir.
///
/// Ön ek tahmininin tersine burada tahmin yok: oda adı öğrencinin kendi
/// girdisidir. Tek cihazlı odalar da önerilir — bir laboratuvarda şu an tek
/// makine açık olabilir, bu onun oda olmadığını göstermez.
fn derive_room_suggestions(rooms: &[String]) -> Vec<GroupSuggestion> {
    use std::collections::BTreeMap;

    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for raw in rooms {
        let oda = raw.trim();
        if oda.is_empty() {
            continue;
        }
        *counts.entry(oda.to_string()).or_insert(0) += 1;
    }

    let mut out: Vec<_> = counts
        .into_iter()
        .map(|(oda, device_count)| GroupSuggestion {
            prefix: String::new(),
            device_count,
            room: Some(oda),
        })
        .collect();

    // Çok cihazlı oda önce; eşitlikte ada göre kararlı sıra.
    out.sort_by(|a, b| {
        b.device_count
            .cmp(&a.device_count)
            .then_with(|| a.room.cmp(&b.room))
    });
    out
}

/// [`suggest_groups`] için saf yardımcı — test edilebilir olsun diye ayrı.
fn derive_group_suggestions(hostnames: &[String]) -> Vec<GroupSuggestion> {
    use std::collections::BTreeMap;

    let mut counts: BTreeMap<String, usize> = BTreeMap::new();

    for raw in hostnames {
        let host = raw.trim().to_ascii_uppercase();
        if host.is_empty() {
            continue;
        }
        // Sondaki rakamları say.
        let trailing_digits = host
            .chars()
            .rev()
            .take_while(|c| c.is_ascii_digit())
            .count();
        if trailing_digits == 0 {
            continue;
        }
        let stem_len = host.len() - trailing_digits;

        // `LAB` (tüm rakamlar atılmış) ve `LAB1`, `LAB10` … (birer basamak
        // korunmuş) adaylarının hepsi sayılır.
        for keep in 0..trailing_digits {
            let candidate = &host[..stem_len + keep];
            if candidate.is_empty() {
                continue;
            }
            *counts.entry(candidate.to_string()).or_insert(0) += 1;
        }
    }

    let mut out: Vec<_> = counts
        .into_iter()
        .filter(|(_, n)| *n >= 2)
        .map(|(prefix, device_count)| GroupSuggestion {
            prefix,
            device_count,
            room: None,
        })
        .collect();

    // Önce daha çok cihaz kapsayan, eşitlikte daha uzun (daha özgül) ön ek.
    out.sort_by(|a, b| {
        b.device_count
            .cmp(&a.device_count)
            .then_with(|| b.prefix.len().cmp(&a.prefix.len()))
            .then_with(|| a.prefix.cmp(&b.prefix))
    });
    out.truncate(12);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hosts(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn iki_laboratuvar_ayri_on_ek_olarak_onerilir() {
        let s =
            derive_group_suggestions(&hosts(&["LAB101", "LAB102", "LAB103", "LAB201", "LAB202"]));
        let prefixes: Vec<&str> = s.iter().map(|g| g.prefix.as_str()).collect();
        assert!(prefixes.contains(&"LAB1"), "LAB1 önerilmeli: {prefixes:?}");
        assert!(prefixes.contains(&"LAB2"), "LAB2 önerilmeli: {prefixes:?}");

        let lab1 = s.iter().find(|g| g.prefix == "LAB1").unwrap();
        assert_eq!(lab1.device_count, 3);
    }

    #[test]
    fn tek_cihazli_on_ek_onerilmez() {
        let s = derive_group_suggestions(&hosts(&["LAB101"]));
        assert!(s.is_empty(), "tek cihaz grup önerisi doğurmamalı: {s:?}");
    }

    #[test]
    fn rakamsiz_adlar_atlanir() {
        let s = derive_group_suggestions(&hosts(&["OGRETMEN-PC", "SUNUCU"]));
        assert!(s.is_empty());
    }

    #[test]
    fn farkli_adlandirma_duzenleri_calisir() {
        let s = derive_group_suggestions(&hosts(&["PC-101", "PC-102", "ATOLYE01", "ATOLYE02"]));
        let prefixes: Vec<&str> = s.iter().map(|g| g.prefix.as_str()).collect();
        assert!(prefixes.contains(&"PC-1"), "{prefixes:?}");
        assert!(prefixes.contains(&"ATOLYE0"), "{prefixes:?}");
    }

    #[test]
    fn oneriler_cihaz_sayisina_gore_siralanir() {
        let s = derive_group_suggestions(&hosts(&[
            "LAB101", "LAB102", "LAB103", "LAB104", "LAB201", "LAB202",
        ]));
        assert!(s[0].device_count >= s[s.len() - 1].device_count);
    }
}

// -------------------------------------------------------- ekran yayını

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BroadcastInfo {
    pub broadcast_id: String,
    pub started_at: i64,
    pub fullscreen: bool,
    /// Multicast açık mı? Arayüz bunu durum çubuğunda gösterir.
    pub multicast: bool,
    /// Multicast ulaşmadığı için TLS yedeğine düşen cihaz sayısı.
    ///
    /// Sıfırdan büyükse laboratuvar anahtarı IGMP'yi süzüyor olabilir;
    /// öğretmenin bunu görmesi sorunu teşhis etmesini sağlar.
    pub fallback_count: usize,
}

/// Öğretmen ekranını bağlı öğrencilere yayınlamaya başlar.
///
/// `fullscreen` penceresiz modu seçer: öğrencide pencere ekranı kaplar.
/// `false` ise öğrenci Alt+Tab ile başka uygulamalara geçebilir — anlatım
/// sırasında not almasına izin vermek isteyen öğretmen için.
#[tauri::command]
pub fn start_broadcast(
    state: State<'_, Arc<AppState>>,
    fullscreen: bool,
) -> CmdResult<BroadcastInfo> {
    if state.broadcasting() {
        return Err("Ekran yayını zaten açık.".into());
    }
    if state.registry.online_count() == 0 {
        return Err("Yayın yapılacak bağlı cihaz yok.".into());
    }

    let handle = crate::broadcast::start(Arc::clone(&state), fullscreen)
        .map_err(|e| fail("Ekran yayını başlatılamadı", e))?;
    let info = BroadcastInfo {
        broadcast_id: handle.broadcast_id.to_string(),
        started_at: handle.started_at,
        fullscreen: handle.fullscreen,
        multicast: handle.multicast.is_some(),
        fallback_count: 0,
    };
    *state.broadcast.write() = Some(handle);
    state.emit("broadcast-changed", Some(&info));
    Ok(info)
}

/// Ekran yayınını durdurur.
#[tauri::command]
pub fn stop_broadcast(state: State<'_, Arc<AppState>>) -> CmdResult<()> {
    let Some(handle) = state.broadcast.read().as_ref().map(|h| h.broadcast_id) else {
        return Err("Açık bir ekran yayını yok.".into());
    };
    if let Some(h) = state.broadcast.read().as_ref() {
        h.stop();
    }
    tracing::info!(%handle, "ekran yayını durduruluyor");
    Ok(())
}

#[tauri::command]
pub fn get_broadcast(state: State<'_, Arc<AppState>>) -> CmdResult<Option<BroadcastInfo>> {
    Ok(state.broadcast.read().as_ref().map(|h| BroadcastInfo {
        broadcast_id: h.broadcast_id.to_string(),
        started_at: h.started_at,
        fullscreen: h.fullscreen,
        multicast: h.multicast.is_some(),
        fallback_count: state.broadcast_fallback.read().len(),
    }))
}

// ----------------------------------------------------- uzaktan kontrol

/// Gönderilen ve düşürülen girdi yığınlarının sayacı.
///
/// Uzaktan kontrol çalışmadığında ilk soru "olaylar hiç gidiyor mu?"
/// oluyor; bu sayaçlar cevabı günlüğe yazar.
static INPUT_SENT: AtomicU64 = AtomicU64::new(0);
static INPUT_DROPPED: AtomicU64 = AtomicU64::new(0);

/// Bir öğrenci bilgisayarının uzaktan kontrolünü devralır.
///
/// Aynı anda yalnızca bir cihaz kontrol edilebilir: iki ekranı birden
/// sürmek öğretmen için anlamsız, öğrenci için kafa karıştırıcı olurdu.
#[tauri::command]
pub async fn start_remote_control(
    state: State<'_, Arc<AppState>>,
    device_id: String,
) -> CmdResult<()> {
    let id = parse_id(&device_id)?;
    let Some(tx) = state.registry.sender(id) else {
        return Err("Cihaz bağlı değil.".into());
    };

    // Başka bir cihaz kontrol ediliyorsa önce onu bırak.
    let previous = *state.controlled_device.read();
    if let Some(previous) = previous {
        if previous != id {
            if let Some(old) = state.registry.sender(previous) {
                let _ = old.send(ServerMessage::RemoteControlStop).await;
            }
        }
    }

    let teacher_name = state.config.read().teacher_name.clone();
    tx.send(ServerMessage::RemoteControlStart { teacher_name })
        .await
        .map_err(|_| "Cihazla bağlantı kapandı.".to_string())?;

    *state.controlled_device.write() = Some(id);
    state
        .db
        .audit(AuditKind::RemoteControlStarted, Some(id), None);
    state.emit("control-changed", Some(device_id));
    tracing::info!(%id, "uzaktan kontrol devralındı");
    Ok(())
}

/// Uzaktan kontrolü bırakır.
#[tauri::command]
pub async fn stop_remote_control(state: State<'_, Arc<AppState>>) -> CmdResult<()> {
    let Some(id) = state.controlled_device.write().take() else {
        return Ok(());
    };
    if let Some(tx) = state.registry.sender(id) {
        let _ = tx.send(ServerMessage::RemoteControlStop).await;
    }
    state
        .db
        .audit(AuditKind::RemoteControlStopped, Some(id), None);
    state.emit("control-changed", Option::<String>::None);
    tracing::info!(%id, "uzaktan kontrol bırakıldı");
    Ok(())
}

/// Girdi olaylarını kontrol edilen cihaza iletir.
///
/// Toplu gelir: fare hareketi saniyede onlarca olay üretir ve her birini
/// ayrı çağrıda göndermek IPC'yi gereksiz doldurur.
#[tauri::command]
pub async fn send_remote_input(
    state: State<'_, Arc<AppState>>,
    events: Vec<openclass_protocol::control::InputEvent>,
) -> CmdResult<()> {
    if events.is_empty() {
        return Ok(());
    }
    let Some(id) = *state.controlled_device.read() else {
        return Err("Uzaktan kontrol açık değil.".into());
    };
    let Some(tx) = state.registry.sender(id) else {
        return Err("Cihaz bağlı değil.".into());
    };

    let count = events.len();

    // Kuyruk doluysa girdi düşürülür: gecikmeli uygulanan fare hareketi
    // birikmiş hâlde uygulanmaktan iyidir. Ama sessizce düşürmek, kontrolün
    // neden çalışmadığını anlaşılmaz kılar — sayaçla görünür tutuluyor.
    match tx.try_send(ServerMessage::RemoteInput { events }) {
        Ok(()) => {
            let n = INPUT_SENT.fetch_add(1, Ordering::Relaxed) + 1;
            if n == 1 {
                tracing::info!(%id, olay = count, "ilk girdi yığını gönderildi");
            }
        }
        Err(_) => {
            let n = INPUT_DROPPED.fetch_add(1, Ordering::Relaxed) + 1;
            // İlk düşüşü ve sonra her 50'de birini bildir; her olayı
            // yazmak günlüğü boğardı.
            if n == 1 || n % 50 == 0 {
                tracing::warn!(
                    %id,
                    dusen = n,
                    "girdi kuyruğu dolu, olay düşürüldü (ekran akışı kanalı dolduruyor olabilir)"
                );
            }
        }
    }
    Ok(())
}

// ------------------------------------------------------------ ekran kilidi

/// Seçili öğrencilerin klavye ve faresini kilitler.
///
/// Kilit **oturuma bağlıdır**: ders bitince, bağlantı kopunca veya öğrenci
/// tarafındaki zaman aşımı dolunca kendiliğinden kalkar. Öğrenci ekranında
/// nedenini açıklayan bir pencere durur; Ctrl+Alt+Del engellenmez.
#[tauri::command]
pub async fn lock_devices(
    state: State<'_, Arc<AppState>>,
    device_ids: Vec<String>,
    message: String,
) -> CmdResult<usize> {

    let message = message.trim().to_string();
    let mut sent = 0usize;
    for raw in &device_ids {
        let id = parse_id(raw)?;
        if let Some(tx) = state.registry.sender(id) {
            if tx
                .send(ServerMessage::LockStudent {
                    message: message.clone(),
                })
                .await
                .is_ok()
            {
                state.db.audit(AuditKind::ScreenLocked, Some(id), None);
                sent += 1;
            }
        }
    }

    if sent == 0 {
        return Err("Hiçbir cihaza ulaşılamadı.".into());
    }
    tracing::info!(adet = sent, "ekran kilidi gönderildi");
    Ok(sent)
}

/// Seçili öğrencilerin kilidini kaldırır.
///
/// Liste boşsa **bağlı tüm cihazların** kilidi açılır: "hepsini aç" en çok
/// ihtiyaç duyulan hareket ve seçim yapmayı gerektirmemeli.
#[tauri::command]
pub async fn unlock_devices(
    state: State<'_, Arc<AppState>>,
    device_ids: Vec<String>,
) -> CmdResult<usize> {
    let targets: Vec<Uuid> = if device_ids.is_empty() {
        state
            .registry
            .all_senders()
            .into_iter()
            .map(|(id, _)| id)
            .collect()
    } else {
        device_ids
            .iter()
            .map(|r| parse_id(r))
            .collect::<Result<_, _>>()?
    };

    let mut sent = 0usize;
    for id in targets {
        if let Some(tx) = state.registry.sender(id) {
            if tx.send(ServerMessage::UnlockStudent).await.is_ok() {
                state.db.audit(AuditKind::ScreenUnlocked, Some(id), None);
                sent += 1;
            }
        }
    }
    tracing::info!(adet = sent, "ekran kilidi kaldırıldı");
    Ok(sent)
}

// -------------------------------------------------------- dosya gönderme

/// Seçili öğrencilere dosya gönderir.
///
/// Dosya TLS kanalından parçalı gönderilir ve alıcı SHA-256 özetiyle
/// doğrular. Öğrenciye her zaman bildirilir — sessiz bırakma yoktur.
///
/// Öğretmen hedef **kökü** seçebilir (`documents` · `desktop` · `downloads`)
/// ama serbest yol gönderemez; dosya her zaman o kökün altındaki `OpenClass`
/// klasörüne yazılır. Tanınmayan bir değer gelirse Belgeler kullanılır.
#[tauri::command]
pub fn send_file(
    state: State<'_, Arc<AppState>>,
    device_ids: Vec<String>,
    path: String,
    destination: Option<String>,
    subpath: Option<String>,
) -> CmdResult<String> {
    if device_ids.is_empty() {
        return Err("Dosya gönderilecek cihaz seçilmedi.".into());
    }

    let targets: Vec<Uuid> = device_ids
        .iter()
        .map(|r| parse_id(r))
        .collect::<Result<_, _>>()?;

    // Arayüzden gelen metin kapalı bir kümeye çözülür; bilinmeyen değer
    // hataya değil varsayılana düşer, böylece eski arayüz de çalışır.
    let destination = match destination.as_deref() {
        Some("desktop") => FileDestination::Desktop,
        Some("downloads") => FileDestination::Downloads,
        _ => FileDestination::Documents,
    };

    let (transfer_id, name, size) =
        crate::transfer::send(Arc::clone(&state), path.into(), targets, destination, subpath)
            .map_err(|e| format!("{e}"))?;

    tracing::info!(
        %transfer_id, %name, size, hedef = destination.label(),
        "dosya gönderimi kuyruğa alındı"
    );
    Ok(transfer_id.to_string())
}

/// Yürüyen ve biten gönderimlerin listesi.
#[tauri::command]
pub fn get_transfers(
    state: State<'_, Arc<AppState>>,
) -> CmdResult<Vec<crate::transfer::TransferView>> {
    Ok(state.transfers.read().values().cloned().collect())
}

/// Bitmiş gönderimleri listeden siler.
#[tauri::command]
pub fn clear_finished_transfers(state: State<'_, Arc<AppState>>) -> CmdResult<()> {
    state.transfers.write().retain(|_, t| !t.finished);
    state.emit_transfers();
    Ok(())
}

// ------------------------------------------------------ uzak dosya gezgini

/// Bir öğrencinin dosya gezginini açar.
///
/// Gezgin **yalnızca listeler ve indirir**; yazma, silme ve çalıştırma
/// protokolde tanımlı değildir. Öğrencide gezgin açık olduğu sürece
/// görünür bir gösterge durur. Aynı anda tek öğrenci gezilebilir.
#[tauri::command]
pub async fn open_browser(state: State<'_, Arc<AppState>>, device_id: String) -> CmdResult<()> {
    let id = parse_id(&device_id)?;
    let Some(tx) = state.registry.sender(id) else {
        return Err("Cihaz bağlı değil.".into());
    };

    // Başka bir öğrenci geziliyorsa önce onun gezgini kapatılır.
    let previous = {
        let guard = state.browse.read();
        guard.as_ref().map(|s| s.device_id)
    };
    if let Some(previous) = previous {
        if previous != id {
            if let Some(old) = state.registry.sender(previous) {
                let _ = old.send(ServerMessage::BrowseEnd).await;
            }
        }
    }

    let hostname = state
        .registry
        .build_views(&state.db)
        .ok()
        .and_then(|v| {
            v.into_iter()
                .find(|d| d.device_id == id)
                .map(|d| d.hostname)
        })
        .unwrap_or_else(|| device_id.clone());

    let teacher_name = state.config.read().teacher_name.clone();
    tx.send(ServerMessage::BrowseStart { teacher_name })
        .await
        .map_err(|_| "Cihazla bağlantı kapandı.".to_string())?;

    // Oturum kurulur ve kök listesi istenir.
    {
        let mut guard = state.browse.write();
        if let Some(old) = guard.take() {
            if let Some(download) = old.download {
                download.abort();
            }
        }
        *guard = Some(crate::browse::BrowseSession::new(id, hostname));
    }
    state.db.audit(AuditKind::BrowseOpened, Some(id), None);
    emit_browse(&state);

    let request_id = Uuid::new_v4();
    let _ = tx
        .send(ServerMessage::BrowseList {
            request_id,
            path: String::new(),
        })
        .await;
    Ok(())
}

/// Gezgini kapatır ve öğrencideki göstergeyi kaldırır.
#[tauri::command]
pub async fn close_browser(state: State<'_, Arc<AppState>>) -> CmdResult<()> {
    let session = state.browse.write().take();
    let Some(session) = session else {
        return Ok(());
    };
    if let Some(download) = session.download {
        download.abort();
    }
    if let Some(tx) = state.registry.sender(session.device_id) {
        let _ = tx.send(ServerMessage::BrowseEnd).await;
    }
    state.emit("browse-changed", Option::<crate::browse::BrowseView>::None);
    tracing::info!(device = %session.device_id, "dosya gezgini kapatıldı");
    Ok(())
}

/// Bir klasörün içeriğini ister.
///
/// Boş yol izin verilen köklerin listesini döndürür. Yolun kapsam içinde
/// olup olmadığına **öğrenci** karar verir; öğretmenin yazdığı yol kapsamı
/// genişletmez.
#[tauri::command]
pub async fn browse_list(state: State<'_, Arc<AppState>>, path: String) -> CmdResult<()> {
    let device_id = {
        let mut guard = state.browse.write();
        let Some(session) = guard.as_mut() else {
            return Err("Dosya gezgini açık değil.".into());
        };
        session.loading = true;
        session.error = None;
        session.device_id
    };
    emit_browse(&state);

    let Some(tx) = state.registry.sender(device_id) else {
        return Err("Cihaz bağlı değil.".into());
    };

    // Her listeleme denetim kaydına yazılır.
    state.db.audit(
        AuditKind::BrowseListed,
        Some(device_id),
        Some(if path.is_empty() { "(kökler)" } else { &path }),
    );

    tx.send(ServerMessage::BrowseList {
        request_id: Uuid::new_v4(),
        path,
    })
    .await
    .map_err(|_| "Cihazla bağlantı kapandı.".to_string())?;
    Ok(())
}

/// Bir dosyayı öğretmenin makinesine indirir.
///
/// Dosya `Belgeler\OpenClass\Toplanan\<bilgisayar adı>` altına yazılır ve
/// SHA-256 özetiyle doğrulanır. Öğrenciye indirme ayrıca bildirilir.
#[tauri::command]
pub async fn browse_fetch(state: State<'_, Arc<AppState>>, path: String) -> CmdResult<()> {
    let device_id = {
        let guard = state.browse.read();
        let Some(session) = guard.as_ref() else {
            return Err("Dosya gezgini açık değil.".into());
        };
        if session.download.is_some() {
            return Err("Başka bir indirme sürüyor.".into());
        }
        session.device_id
    };

    let Some(tx) = state.registry.sender(device_id) else {
        return Err("Cihaz bağlı değil.".into());
    };

    tx.send(ServerMessage::BrowseFetch {
        request_id: Uuid::new_v4(),
        path,
    })
    .await
    .map_err(|_| "Cihazla bağlantı kapandı.".to_string())?;
    Ok(())
}

/// Gezginin o anki durumu.
#[tauri::command]
pub fn get_browse(state: State<'_, Arc<AppState>>) -> CmdResult<Option<crate::browse::BrowseView>> {
    Ok(state
        .browse
        .read()
        .as_ref()
        .map(crate::browse::BrowseView::from))
}

/// İndirilen dosyaların klasörünü işletim sisteminde açar.
#[tauri::command]
pub fn open_downloads_dir(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
) -> CmdResult<()> {
    use tauri_plugin_opener::OpenerExt;

    let kok = state.download_root.read().clone();
    let dir = crate::browse::downloads_dir(kok.as_deref())
        .map_err(|e| fail("Klasör bulunamadı", e))?;
    app.opener()
        .open_path(dir.to_string_lossy().to_string(), None::<&str>)
        .map_err(|e| format!("Klasör açılamadı: {e}"))
}

/// Öğretmenin seçtiği indirme klasörünü döner (seçilmemişse varsayılan).
#[tauri::command]
pub fn get_download_root(state: State<'_, Arc<AppState>>) -> CmdResult<String> {
    let kok = state.download_root.read().clone();
    let dir =
        crate::browse::downloads_dir(kok.as_deref()).map_err(|e| fail("Klasör bulunamadı", e))?;
    Ok(dir.to_string_lossy().into_owned())
}

/// İndirme klasörünü ayarlar; `None` varsayılana döner.
///
/// Klasör öğretmenin kendi makinesindedir, bu yüzden serbest yol kabul
/// edilir. Var olduğu ve klasör olduğu burada doğrulanır — yanlış yol
/// aktarımın ortasında değil, seçim anında hata vermeli.
#[tauri::command]
pub fn set_download_root(
    state: State<'_, Arc<AppState>>,
    path: Option<String>,
) -> CmdResult<String> {
    match path {
        Some(p) if !p.trim().is_empty() => {
            let yol = std::path::PathBuf::from(p);
            if !yol.is_dir() {
                return Err(format!("Klasör bulunamadı: {}", yol.display()));
            }
            // Yazılabilirlik sınaması: salt okunur bir klasör seçilirse
            // indirme ilk parçada değil, burada hata verir.
            let deneme = yol.join(".openclass-yazma-denemesi");
            std::fs::write(&deneme, b"")
                .map_err(|e| format!("Bu klasöre yazılamıyor: {e}"))?;
            let _ = std::fs::remove_file(&deneme);

            let gosterim = yol.to_string_lossy().into_owned();
            *state.download_root.write() = Some(yol);
            tracing::info!(klasor = %gosterim, "indirme klasörü değişti");
            Ok(gosterim)
        }
        _ => {
            *state.download_root.write() = None;
            let dir = crate::browse::downloads_dir(None)
                .map_err(|e| fail("Klasör bulunamadı", e))?;
            tracing::info!("indirme klasörü varsayılana döndü");
            Ok(dir.to_string_lossy().into_owned())
        }
    }
}

fn emit_browse(state: &Arc<AppState>) {
    let view = state
        .browse
        .read()
        .as_ref()
        .map(crate::browse::BrowseView::from);
    state.emit("browse-changed", view);
}
