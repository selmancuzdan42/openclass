//! Öğretmen konsolunun kalıcı deposu (SQLite).
//!
//! Üç şey saklanır:
//!
//! * **devices** — eşleştirilmiş cihazlar ve açık anahtarları. Bir cihazın
//!   yönetilebilir olmasının tek kaynağı bu tablodur; hostname beyanı değil.
//! * **sessions** — ders oturumlarının başlangıç/bitişi.
//! * **audit** — denetim kayıtları.
//!
//! Hassas veri saklanmaz: ekran görüntüleri diske hiç yazılmaz, eşleştirme
//! kodları kalıcı değildir, öğrenci kişisel verisi tutulmaz.

use anyhow::{Context, Result};
use openclass_protocol::control::DeviceStatus;
use parking_lot::Mutex;
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use uuid::Uuid;

/// Kayıtlı bir öğrenci cihazı.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DeviceRecord {
    pub device_id: Uuid,
    /// Son görülen bilgisayar adı (büyük harfe normalleştirilmiş).
    pub hostname: String,
    /// Öğretmenin verdiği isteğe bağlı görünen ad (ör. "1. sıra sol").
    pub display_name: Option<String>,
    #[serde(skip)]
    pub public_key: [u8; 32],
    pub status: DeviceStatus,
    pub paired_at: i64,
    pub last_seen: i64,
}

/// Denetim kaydı kategorileri.
///
/// Serbest metin yerine kapalı bir küme kullanılır; böylece loglar
/// filtrelenebilir kalır ve yanlışlıkla hassas veri yazılması zorlaşır.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditKind {
    DevicePaired,
    DeviceRejected,
    DeviceRevoked,
    DeviceConnected,
    DeviceDisconnected,
    AuthFailed,
    SessionStarted,
    SessionEnded,
    ScreenStreamStarted,
    ScreenStreamStopped,
    NoticeSent,
    HelpRequested,
    RemoteControlStarted,
    RemoteControlStopped,
    ScreenLocked,
    ScreenUnlocked,
    FileSent,
    BrowseOpened,
    BrowseListed,
    FileDownloaded,
}

impl AuditKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::DevicePaired => "device_paired",
            Self::DeviceRejected => "device_rejected",
            Self::DeviceRevoked => "device_revoked",
            Self::DeviceConnected => "device_connected",
            Self::DeviceDisconnected => "device_disconnected",
            Self::AuthFailed => "auth_failed",
            Self::SessionStarted => "session_started",
            Self::SessionEnded => "session_ended",
            Self::ScreenStreamStarted => "screen_stream_started",
            Self::ScreenStreamStopped => "screen_stream_stopped",
            Self::NoticeSent => "notice_sent",
            Self::HelpRequested => "help_requested",
            Self::RemoteControlStarted => "remote_control_started",
            Self::RemoteControlStopped => "remote_control_stopped",
            Self::ScreenLocked => "screen_locked",
            Self::ScreenUnlocked => "screen_unlocked",
            Self::FileSent => "file_sent",
            Self::BrowseOpened => "browse_opened",
            Self::BrowseListed => "browse_listed",
            Self::FileDownloaded => "file_downloaded",
        }
    }
}

/// Denetim kaydının arayüze dönen hâli.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditEntry {
    pub at: i64,
    pub kind: String,
    pub device_id: Option<Uuid>,
    pub detail: Option<String>,
}

pub fn status_to_str(status: DeviceStatus) -> &'static str {
    match status {
        DeviceStatus::Authorized => "authorized",
        DeviceStatus::PendingPairing => "pending",
        DeviceStatus::Rejected => "rejected",
    }
}

fn status_from_str(s: &str) -> DeviceStatus {
    match s {
        "authorized" => DeviceStatus::Authorized,
        "rejected" => DeviceStatus::Rejected,
        // Tanınmayan değer en kısıtlayıcı duruma düşer: yetki verilmez.
        _ => DeviceStatus::PendingPairing,
    }
}

/// Unix zaman damgası (saniye).
pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// SQLite bağlantısını saran depo.
///
/// Bağlantı tek ve `Mutex` korumalıdır. Yazma hacmi düşük olduğu için
/// (saniyede birkaç kayıt) havuz gereksizdir; ekran kareleri veritabanına
/// hiç uğramaz.
pub struct Db {
    conn: Mutex<Connection>,
}

impl Db {
    /// Veritabanını açar ve şemayı kurar.
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)
            .with_context(|| format!("veritabanı açılamadı: {}", path.display()))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let db = Self {
            conn: Mutex::new(conn),
        };
        db.migrate()?;
        Ok(db)
    }

    /// Testler için bellek içi veritabanı.
    #[cfg(test)]
    pub fn open_in_memory() -> Result<Self> {
        let db = Self {
            conn: Mutex::new(Connection::open_in_memory()?),
        };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> Result<()> {
        self.conn.lock().execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS devices (
                device_id    TEXT PRIMARY KEY NOT NULL,
                hostname     TEXT NOT NULL,
                display_name TEXT,
                public_key   BLOB NOT NULL,
                status       TEXT NOT NULL,
                paired_at    INTEGER NOT NULL,
                last_seen    INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_devices_hostname ON devices(hostname);

            CREATE TABLE IF NOT EXISTS sessions (
                session_id TEXT PRIMARY KEY NOT NULL,
                class_name TEXT NOT NULL,
                started_at INTEGER NOT NULL,
                ended_at   INTEGER
            );

            CREATE TABLE IF NOT EXISTS audit (
                id        INTEGER PRIMARY KEY AUTOINCREMENT,
                at        INTEGER NOT NULL,
                kind      TEXT NOT NULL,
                device_id TEXT,
                detail    TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_audit_at ON audit(at DESC);
            "#,
        )?;
        Ok(())
    }

    // ---------------------------------------------------------------- cihazlar

    /// Yeni bir cihazı yetkili olarak kaydeder (eşleştirme tamamlandığında).
    ///
    /// Aynı `device_id` yeniden eşleşirse açık anahtarı ve hostname'i güncellenir.
    pub fn upsert_paired_device(
        &self,
        device_id: Uuid,
        hostname: &str,
        public_key: &[u8; 32],
    ) -> Result<()> {
        let ts = now();
        self.conn.lock().execute(
            r#"
            INSERT INTO devices (device_id, hostname, display_name, public_key, status, paired_at, last_seen)
            VALUES (?1, ?2, NULL, ?3, 'authorized', ?4, ?4)
            ON CONFLICT(device_id) DO UPDATE SET
                hostname   = excluded.hostname,
                public_key = excluded.public_key,
                status     = 'authorized',
                paired_at  = excluded.paired_at,
                last_seen  = excluded.last_seen
            "#,
            params![device_id.to_string(), hostname, public_key.as_slice(), ts],
        )?;
        Ok(())
    }

    /// Cihaz kaydını kimliğe göre getirir.
    pub fn device(&self, device_id: Uuid) -> Result<Option<DeviceRecord>> {
        let conn = self.conn.lock();
        let rec = conn
            .query_row(
                "SELECT device_id, hostname, display_name, public_key, status, paired_at, last_seen
                 FROM devices WHERE device_id = ?1",
                params![device_id.to_string()],
                row_to_device,
            )
            .optional()?;
        // Option<Result<Option<_>>> → Result<Option<_>>: satır yoksa da,
        // satır bozuksa da sonuç None olur.
        rec.transpose().map(Option::flatten)
    }

    /// Kayıtlı tüm cihazlar, bilgisayar adına göre sıralı.
    pub fn all_devices(&self) -> Result<Vec<DeviceRecord>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT device_id, hostname, display_name, public_key, status, paired_at, last_seen
             FROM devices ORDER BY hostname ASC",
        )?;
        let rows = stmt.query_map([], row_to_device)?;
        let mut out = Vec::new();
        for row in rows {
            // İlk `?` SQLite hatasını, ikincisi satır dönüşüm hatasını açar.
            if let Some(rec) = row?? {
                out.push(rec);
            }
        }
        Ok(out)
    }

    /// Cihazın son görülme zamanını ve bilgisayar adını tazeler.
    pub fn touch_device(&self, device_id: Uuid, hostname: &str) -> Result<()> {
        self.conn.lock().execute(
            "UPDATE devices SET last_seen = ?2, hostname = ?3 WHERE device_id = ?1",
            params![device_id.to_string(), now(), hostname],
        )?;
        Ok(())
    }

    /// Cihazın yetkisini kaldırır. Kayıt silinmez; denetim izi korunur.
    pub fn revoke_device(&self, device_id: Uuid) -> Result<()> {
        self.conn.lock().execute(
            "UPDATE devices SET status = 'rejected' WHERE device_id = ?1",
            params![device_id.to_string()],
        )?;
        Ok(())
    }

    /// Cihaza öğretmenin verdiği görünen adı atar.
    pub fn set_display_name(&self, device_id: Uuid, name: Option<&str>) -> Result<()> {
        self.conn.lock().execute(
            "UPDATE devices SET display_name = ?2 WHERE device_id = ?1",
            params![device_id.to_string(), name],
        )?;
        Ok(())
    }

    // --------------------------------------------------------------- oturumlar

    pub fn start_session(&self, session_id: Uuid, class_name: &str) -> Result<()> {
        self.conn.lock().execute(
            "INSERT INTO sessions (session_id, class_name, started_at, ended_at) VALUES (?1, ?2, ?3, NULL)",
            params![session_id.to_string(), class_name, now()],
        )?;
        Ok(())
    }

    pub fn end_session(&self, session_id: Uuid) -> Result<()> {
        self.conn.lock().execute(
            "UPDATE sessions SET ended_at = ?2 WHERE session_id = ?1 AND ended_at IS NULL",
            params![session_id.to_string(), now()],
        )?;
        Ok(())
    }

    // ------------------------------------------------------------------ denetim

    /// Denetim kaydı yazar.
    ///
    /// `detail` kullanıcıya gösterilebilir kısa bir açıklamadır. Buraya parola,
    /// eşleştirme kodu, anahtar veya öğrenci kişisel verisi **yazılmaz**.
    pub fn audit(&self, kind: AuditKind, device_id: Option<Uuid>, detail: Option<&str>) {
        let result = self.conn.lock().execute(
            "INSERT INTO audit (at, kind, device_id, detail) VALUES (?1, ?2, ?3, ?4)",
            params![
                now(),
                kind.as_str(),
                device_id.map(|d| d.to_string()),
                detail
            ],
        );
        // Denetim yazımı başarısız olursa uygulama durmaz; sorun loglanır.
        if let Err(err) = result {
            tracing::error!(%err, kategori = kind.as_str(), "denetim kaydı yazılamadı");
        }
    }

    /// Son denetim kayıtları, yeniden eskiye.
    pub fn recent_audit(&self, limit: u32) -> Result<Vec<AuditEntry>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT at, kind, device_id, detail FROM audit ORDER BY at DESC, id DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], |row| {
            let raw_id: Option<String> = row.get(2)?;
            Ok(AuditEntry {
                at: row.get(0)?,
                kind: row.get(1)?,
                device_id: raw_id.and_then(|s| Uuid::parse_str(&s).ok()),
                detail: row.get(3)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }
}

/// `devices` satırını `DeviceRecord`'a çevirir.
///
/// Dış katman `Result<Result<..>>` görür: dıştaki SQLite hatası, içteki
/// bozuk veri (ör. 32 bayt olmayan açık anahtar). Bozuk satır atlanır,
/// tüm sorgu düşmez.
#[allow(clippy::type_complexity)]
fn row_to_device(row: &rusqlite::Row<'_>) -> rusqlite::Result<Result<Option<DeviceRecord>>> {
    let id_text: String = row.get(0)?;
    let hostname: String = row.get(1)?;
    let display_name: Option<String> = row.get(2)?;
    let key_blob: Vec<u8> = row.get(3)?;
    let status_text: String = row.get(4)?;
    let paired_at: i64 = row.get(5)?;
    let last_seen: i64 = row.get(6)?;

    Ok((|| {
        let device_id = match Uuid::parse_str(&id_text) {
            Ok(id) => id,
            Err(_) => {
                tracing::warn!(kayit = %id_text, "geçersiz cihaz kimliği, satır atlandı");
                return Ok(None);
            }
        };
        let public_key: [u8; 32] = match key_blob.try_into() {
            Ok(k) => k,
            Err(_) => {
                tracing::warn!(cihaz = %device_id, "açık anahtar 32 bayt değil, satır atlandı");
                return Ok(None);
            }
        };
        Ok(Some(DeviceRecord {
            device_id,
            hostname,
            display_name,
            public_key,
            status: status_from_str(&status_text),
            paired_at,
            last_seen,
        }))
    })())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eslestirilen_cihaz_kaydedilir_ve_okunur() {
        let db = Db::open_in_memory().unwrap();
        let id = Uuid::new_v4();
        db.upsert_paired_device(id, "LAB101", &[1u8; 32]).unwrap();

        let rec = db.device(id).unwrap().unwrap();
        assert_eq!(rec.hostname, "LAB101");
        assert_eq!(rec.public_key, [1u8; 32]);
        assert_eq!(rec.status, DeviceStatus::Authorized);
    }

    #[test]
    fn yeniden_eslestirme_acik_anahtari_gunceller() {
        let db = Db::open_in_memory().unwrap();
        let id = Uuid::new_v4();
        db.upsert_paired_device(id, "LAB101", &[1u8; 32]).unwrap();
        db.upsert_paired_device(id, "LAB105", &[2u8; 32]).unwrap();

        let rec = db.device(id).unwrap().unwrap();
        assert_eq!(rec.public_key, [2u8; 32]);
        assert_eq!(rec.hostname, "LAB105");
        assert_eq!(
            db.all_devices().unwrap().len(),
            1,
            "mükerrer kayıt oluşmamalı"
        );
    }

    #[test]
    fn yetki_kaldirilan_cihaz_reddedilmis_olur() {
        let db = Db::open_in_memory().unwrap();
        let id = Uuid::new_v4();
        db.upsert_paired_device(id, "LAB101", &[1u8; 32]).unwrap();
        db.revoke_device(id).unwrap();
        assert_eq!(
            db.device(id).unwrap().unwrap().status,
            DeviceStatus::Rejected
        );
    }

    #[test]
    fn tanimsiz_durum_metni_yetki_vermez() {
        assert_eq!(status_from_str("zirva"), DeviceStatus::PendingPairing);
        assert_eq!(status_from_str(""), DeviceStatus::PendingPairing);
    }

    #[test]
    fn denetim_kaydi_yazilir_ve_okunur() {
        let db = Db::open_in_memory().unwrap();
        let id = Uuid::new_v4();
        db.audit(
            AuditKind::DevicePaired,
            Some(id),
            Some("LAB101 eşleştirildi"),
        );
        db.audit(AuditKind::SessionStarted, None, Some("Lab 1"));

        let entries = db.recent_audit(10).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].kind, "session_started");
        assert_eq!(entries[1].device_id, Some(id));
    }

    #[test]
    fn oturum_baslatilip_bitirilebilir() {
        let db = Db::open_in_memory().unwrap();
        let sid = Uuid::new_v4();
        db.start_session(sid, "Lab 1").unwrap();
        db.end_session(sid).unwrap();
        // İkinci kez bitirmek sessizce hiçbir satırı etkilemez.
        db.end_session(sid).unwrap();
    }

    #[test]
    fn bozuk_satir_tum_sorguyu_dusurmez() {
        let db = Db::open_in_memory().unwrap();
        let saglam = Uuid::new_v4();
        db.upsert_paired_device(saglam, "LAB101", &[1u8; 32])
            .unwrap();
        // Açık anahtarı 32 bayt olmayan bozuk bir satır elle eklenir.
        db.conn
            .lock()
            .execute(
                "INSERT INTO devices (device_id, hostname, display_name, public_key, status, paired_at, last_seen)
                 VALUES ('bozuk-uuid', 'LAB999', NULL, X'00', 'authorized', 0, 0)",
                [],
            )
            .unwrap();

        let devices = db.all_devices().unwrap();
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].device_id, saglam);
    }
}
