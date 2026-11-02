//! Kontrol ve veri kanalı mesajları (TLS üzerinde WebSocket).
//!
//! Tüm mesajlar MessagePack ile kodlanır ve WebSocket **binary** çerçevesi
//! olarak taşınır. Ekran kareleri `serde_bytes` sayesinde ham ikili kalır —
//! Base64 şişmesi yoktur.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Bir ekran yayınının kalite profili. Öğretmen konsolu, ekran grid'de mi
/// yoksa büyütülmüş mü olduğuna göre bunu çalışma anında değiştirir.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamQuality {
    /// Kare bu genişliğe sığacak şekilde küçültülür (en-boy oranı korunur).
    pub max_width: u32,
    pub max_height: u32,
    /// Saniyedeki hedef kare sayısı. Garanti değil, üst sınırdır.
    pub fps: u8,
    /// JPEG kalite faktörü (1-100).
    pub jpeg_quality: u8,
    /// Bu akış için azami verim (kilobit/saniye).
    ///
    /// Sınır aşılırsa kare düşürülür. Referans ürünler de benzer bir tavan
    /// kullanır (NetSupport kablosuzda varsayılan 8 Mbit/s).
    pub max_kbps: u32,
}

impl StreamQuality {
    /// Izgara önizlemesi: düşük çözünürlük, düşük kare hızı.
    ///
    /// Değişen bölge aktarımı sayesinde durağan ekranda trafik sıfıra yakındır;
    /// bu değerler yalnızca üst sınırdır.
    pub const PREVIEW: Self = Self {
        max_width: 480,
        max_height: 270,
        fps: 4,
        jpeg_quality: 55,
        max_kbps: 1_024,
    };

    /// Öğretmen ekranının öğrencilere yayını.
    ///
    /// Tek kodlama, çok sayıda alıcı. Metin okunurluğu önemli olduğu için
    /// çözünürlük düşürülmez; verim tavanı hareketli içerikte kare hızını
    /// düşürerek ağı korur.
    pub const BROADCAST: Self = Self {
        max_width: 1920,
        max_height: 1080,
        fps: 25,
        jpeg_quality: 70,
        max_kbps: 8_192,
    };

    /// Büyütülmüş tek ekran: okunabilir metin hedeflenir.
    ///
    /// Ölçüm, 1920x1080'i küçültmenin kodlamaktan pahalı olduğunu gösterdi;
    /// bu yüzden sınır doğrudan 1080p. Verim tavanı NetSupport'un kablosuz
    /// varsayılanıyla aynı mertebede tutuldu (8 Mbit/s).
    pub const FOCUSED: Self = Self {
        max_width: 1920,
        max_height: 1080,
        fps: 25,
        jpeg_quality: 75,
        max_kbps: 8_192,
    };
}

/// Bir ekran bölgesinin sıkıştırma biçimi.
///
/// Enum, ileride donanımsal H.264'e geçerken protokolü kırmamak için
/// genişletilebilir bırakılmıştır.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrameCodec {
    Jpeg,
}

/// Ekranın değişmiş bir dikdörtgen bölgesi.
///
/// Sınıf ekranının büyük kısmı kareden kareye değişmez; yalnızca değişen
/// bölgeleri göndermek, tüm kareyi yeniden kodlayıp yollamaktan kat kat
/// ucuzdur. Referans ürünler (NetSupport dâhil) video kodeki yerine bu
/// yöntemi kullanır.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenRect {
    /// Karenin sol üst köşesine göre konum.
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    /// Bu bölgenin sıkıştırılmış görüntüsü.
    #[serde(with = "serde_bytes")]
    pub data: Vec<u8>,
}

/// Öğretmen ekranı yayınının multicast taşıma bilgisi.
///
/// Oturum anahtarı burada, **TLS kanalının içinde** gider: ağdaki herkes
/// multicast paketlerini görebilir ama anahtarı yalnızca kimliği
/// doğrulanmış öğrenciler alır.
#[derive(Clone, Serialize, Deserialize)]
pub struct MulticastInfo {
    /// Katılınacak grup adresi.
    pub group: String,
    pub port: u16,
    /// Bu yayına özel 32 baytlık ChaCha20-Poly1305 anahtarı.
    pub key: [u8; 32],
}

impl std::fmt::Debug for MulticastInfo {
    /// Anahtar günlüğe sızmamalı.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MulticastInfo")
            .field("group", &self.group)
            .field("port", &self.port)
            .field("key", &"gizli")
            .finish()
    }
}

/// Uzak dosya gezgininde tek bir girdi.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowseEntry {
    pub name: String,
    pub is_dir: bool,
    /// Klasörlerde 0.
    pub size: u64,
    /// Son değişiklik zamanı (Unix saniye); okunamazsa 0.
    pub modified: i64,
}

/// Öğrencide gezilmesine izin verilen bir kök.
///
/// Kapsam bilinçli olarak kısıtlıdır: tüm disk değil, öğrencinin kendi
/// belge klasörleri. Sistem dizinlerinde dolaşmanın dersle ilgisi yok.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowseRoot {
    /// Öğretmene gösterilecek ad ("Masaüstü", "Belgeler"…).
    pub label: String,
    /// Sonraki isteklerde kullanılacak yol.
    pub path: String,
}

/// Fare düğmesi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// Öğretmenden öğrenciye iletilen tek bir girdi olayı.
///
/// Fare konumu **normalleştirilmiş** (0.0–1.0) gönderilir. Öğretmenin
/// gördüğü görüntü ölçeklenmiştir ve öğrencinin ekran çözünürlüğü farklı
/// olabilir; piksel göndermek iki tarafı da yanlış yere tıklatırdı.
///
/// # Tel üzerindeki biçim
///
/// Varyant adları `snake_case`'e çevrilir (`mouse_move`, `mouse_button`…).
/// Bu tip öğretmen **arayüzünden** Rust tarafına geçer; TypeScript nesnesi
/// birebir bu adları kullanır. `rename_all` olmadan Rust `"MouseMove"`
/// bekler, arayüz `"mouse_move"` gönderir ve her çağrı sessizce
/// çözümlemede düşer — uzaktan kontrol tam olarak bu yüzden çalışmıyordu.
/// [`super::control::tests`] içindeki biçim testi bunu kilitler.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum InputEvent {
    MouseMove {
        x: f32,
        y: f32,
    },
    MouseButton {
        button: MouseButton,
        pressed: bool,
    },
    /// Tekerlek adımları; pozitif değer yukarı/sağa.
    MouseWheel {
        dx: i32,
        dy: i32,
    },
    /// Windows sanal tuş kodu (VK_*).
    Key {
        code: u16,
        pressed: bool,
    },
}

/// Öğrenci cihazının öğretmen konsolundaki durumu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceStatus {
    /// Cihaz tanınıyor ve yetkili.
    Authorized,
    /// Cihaz kendini tanıttı ama henüz eşleştirilmedi.
    PendingPairing,
    /// Cihaz reddedildi veya yetkisi kaldırıldı.
    Rejected,
}

/// Öğretmenin dosyayı bırakacağı kök klasör.
///
/// **Serbest yol taşınmaz.** Öğretmen yalnızca bu üç kökten birini seçebilir;
/// öğrenci tarafında her biri kullanıcının kendi klasörüne çözülür ve altına
/// `OpenClass` eklenir. Böylece hedef seçilebilir olurken sistem dizinlerine
/// yazma kapısı açılmaz — serbest metin bir yol alanı tam bunu yapardı.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileDestination {
    /// Belgeler — 0.1.0 davranışı, alan gelmediğinde de bu kullanılır.
    #[default]
    Documents,
    Desktop,
    Downloads,
}

impl FileDestination {
    /// Arayüzde ve bildirimlerde görünen Türkçe ad.
    pub fn label(self) -> &'static str {
        match self {
            Self::Documents => "Belgeler",
            Self::Desktop => "Masaüstü",
            Self::Downloads => "İndirilenler",
        }
    }
}

/// Öğrenci → öğretmen.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "t")]
pub enum ClientMessage {
    /// Bağlantının ilk mesajı.
    Hello {
        protocol: u16,
        device_id: Uuid,
        hostname: String,
        app_version: String,
        /// Daha önce eşleştiyse açık anahtar burada gelir; öğretmen kaydı bulur.
        #[serde(with = "serde_bytes")]
        public_key: Vec<u8>,
        /// Öğrencinin ilk açılış sihirbazında girdiği laboratuvar/oda adı.
        ///
        /// `#[serde(default)]` **zorunludur**: 0.1.0 ajanları bu alanı hiç
        /// göndermez ve alan zorunlu olsaydı çözümleme patlar, eski bütün
        /// kurulumlar bağlanamaz hale gelirdi. Alanı göndermeyen cihazda
        /// öğretmen kayıtlı odayı korur.
        #[serde(default)]
        room: Option<String>,
    },

    /// İlk eşleştirme talebi. Öğrenci, öğretmenin ekranındaki kodu kullanarak
    /// kanıt üretmiştir (bkz. `crate::auth::pairing_proof`).
    PairRequest {
        #[serde(with = "serde_bytes")]
        public_key: Vec<u8>,
        #[serde(with = "serde_bytes")]
        proof: Vec<u8>,
    },

    /// `ServerMessage::AuthChallenge` cevabı.
    AuthResponse {
        #[serde(with = "serde_bytes")]
        signature: Vec<u8>,
    },

    /// Öğrencinin "Yardım İste" butonu.
    HelpRequest {
        note: Option<String>,
    },

    /// Yardım talebinin öğrenci tarafından geri çekilmesi.
    HelpCancel,

    /// Dosya teklifi kabul edildi; parçalar beklenebilir.
    FileAccepted {
        transfer_id: Uuid,
    },

    /// Dosya teklifi reddedildi (yer yok, boyut aşıldı, ad geçersiz…).
    FileRejected {
        transfer_id: Uuid,
        reason: String,
    },

    /// Aktarım tamamlandı.
    FileDone {
        transfer_id: Uuid,
        /// Özet doğrulandı ve dosya yerine taşındı mı?
        ok: bool,
        /// Başarısızlık nedeni veya kaydedilen yol.
        detail: String,
    },

    /// Klasör içeriği.
    ///
    /// `path` boş gelen istekte `roots` doldurulur, `entries` boş kalır.
    BrowseListing {
        request_id: Uuid,
        path: String,
        roots: Vec<BrowseRoot>,
        entries: Vec<BrowseEntry>,
    },

    /// İndirme başlıyor; parçalar takip eder.
    BrowseFileStart {
        request_id: Uuid,
        name: String,
        size: u64,
        sha256: [u8; 32],
    },

    /// İndirme parçası.
    BrowseChunk {
        request_id: Uuid,
        seq: u32,
        #[serde(with = "serde_bytes")]
        data: Vec<u8>,
    },

    /// İndirmenin son parçası gönderildi.
    BrowseFileEnd {
        request_id: Uuid,
    },

    /// İstek karşılanamadı (kapsam dışı yol, okuma hatası, çok büyük…).
    BrowseFailed {
        request_id: Uuid,
        reason: String,
    },

    /// Öğretmen ekranı yayınında eksik parça kaldı; tam kare isteniyor.
    RequestTeacherKeyframe,

    /// Multicast paketleri bu öğrenciye ulaşmıyor.
    ///
    /// Öğretmen bu cihaza kareleri TLS üzerinden tek tek göndermeye geçer.
    /// Anahtarın ağa düşmesi gibi bir risk yok; yalnızca bant genişliği
    /// maliyeti artar.
    MulticastUnavailable {
        broadcast_id: Uuid,
    },

    /// Ekranın değişen bölgeleri.
    ///
    /// `keyframe` doğruysa bölgeler tüm kareyi kapsar ve alıcı önceki
    /// durumu atabilir. Yanlışsa bölgeler bir öncekinin üzerine çizilir.
    ScreenUpdate {
        stream_id: u32,
        /// Artan sıra numarası. Atlanan numaralar düşürülmüş kareleri gösterir.
        seq: u64,
        /// Tam karenin boyutu (bölgeler bunun içine yerleşir).
        width: u32,
        height: u32,
        /// Tüm kareyi kapsayan bağımsız güncelleme mi?
        keyframe: bool,
        codec: FrameCodec,
        rects: Vec<ScreenRect>,
    },

    /// Ekran yakalama başlatılamadı veya durdu. Konsol sahte görüntü göstermez,
    /// bu hatayı kullanıcıya iletir.
    ScreenError {
        stream_id: u32,
        reason: String,
    },

    /// Öğretmenden gelen mesajın görüldüğü bilgisi.
    MessageAck {
        message_id: Uuid,
    },

    Pong {
        token: u64,
    },
}

/// Öğretmen → öğrenci.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "t")]
pub enum ServerMessage {
    /// `Hello` cevabı.
    HelloAck {
        protocol: u16,
        teacher_id: Uuid,
        teacher_name: String,
        status: DeviceStatus,
    },

    /// Eşleştirme sonucu.
    PairResult {
        accepted: bool,
        reason: Option<String>,
    },

    /// Yetkili cihazdan imza istenir.
    AuthChallenge {
        #[serde(with = "serde_bytes")]
        challenge: Vec<u8>,
    },

    /// Kimlik doğrulama sonucu. Başarısızsa bağlantı kapatılır.
    AuthResult {
        ok: bool,
        reason: Option<String>,
        /// Öğrenci kayıtlı eşleştirmesini silmeli mi?
        ///
        /// Öğretmen cihazın yetkisini kaldırdığında `true` gelir. Öğrenci
        /// eski kimliğiyle yeniden bağlanmayı denemeyi bırakır ve
        /// "eşleştirilmedi" durumuna döner; aksi hâlde geçersiz kimlikle
        /// sonsuza kadar denerdi.
        ///
        /// Geçici bir doğrulama hatası bu bayrağı taşımaz: ağ sorunu
        /// yüzünden eşleştirmenin silinmesi istenmez.
        #[serde(default)]
        reset_pairing: bool,
    },

    /// Ekran yayınını başlat.
    ///
    /// Öğrencide yayın **her zaman görünürdür**: tepsi simgesi değişir ve
    /// "EKRANINIZ PAYLAŞILIYOR" göstergesi çıkar. Ders oturumu kavramı
    /// kaldırıldığından bu gösterge tek koruma katmanıdır ve hiçbir mesajla
    /// bastırılamaz.
    StartScreenStream {
        stream_id: u32,
        quality: StreamQuality,
    },

    /// Yayının kalitesini değiştir (grid ↔ büyütülmüş geçişi).
    UpdateStreamQuality {
        stream_id: u32,
        quality: StreamQuality,
    },

    /// Ekran yayınını durdur.
    StopScreenStream {
        stream_id: u32,
    },

    /// Öğretmen ekranı yayını başladı.
    ///
    /// Öğrenci bir gösterim penceresi açar. Yayın süresince pencerenin açık
    /// olduğu öğrenciden gizlenmez.
    TeacherScreenStart {
        broadcast_id: Uuid,
        teacher_name: String,
        /// Penceresiz (tam ekran) mod mu?
        ///
        /// `false` ise pencere normaldir ve öğrenci Alt+Tab ile başka
        /// uygulamalara geçebilir. `true` ise tam ekran ve üstte durur.
        fullscreen: bool,
        /// Multicast açıksa taşıma bilgisi.
        ///
        /// `None` ise kareler yalnızca TLS kanalından gelir.
        multicast: Option<MulticastInfo>,
    },

    /// Öğrencinin klavye ve faresini kilitler.
    ///
    /// Ekranda neden kilitlendiğini açıklayan bir pencere gösterilir.
    /// Kilit **oturuma bağlıdır**: ders bitince, bağlantı kopunca veya
    /// zaman aşımında kendiliğinden kalkar. Ctrl+Alt+Del engellenmez —
    /// öğrenci her zaman makineyi kurtarabilmelidir.
    LockStudent {
        /// Öğrenciye gösterilecek açıklama.
        message: String,
    },

    /// Kilidi kaldırır.
    UnlockStudent,

    /// Dosya gönderme teklifi.
    ///
    /// Öğrenci yer ve boyut kontrolünü yapıp kabul veya ret bildirir.
    /// Dosya sessizce bırakılmaz; öğrenciye her zaman bildirilir.
    FileOffer {
        transfer_id: Uuid,
        /// Dosya adı. Yol bileşeni içermez; alıcı yine de temizler.
        name: String,
        size: u64,
        /// Tamamlandığında doğrulanacak SHA-256 özeti.
        sha256: [u8; 32],
        /// Dosyanın bırakılacağı kök klasör.
        ///
        /// `#[serde(default)]`: 0.1.0 öğretmeni bu alanı göndermez ve o
        /// durumda Belgeler kullanılır — eski konsol yeni ajanla çalışmaya
        /// devam eder.
        #[serde(default)]
        destination: FileDestination,
        /// Kökün altındaki hedef klasör (`Ödevler/Hafta 3` gibi).
        ///
        /// **Göreli yoldur, mutlak yol değil.** Öğretmen hedefi bir
        /// öğrencinin gezgininde seçer ama gönderim birden çok cihaza
        /// gidebilir ve her makinede kullanıcı klasörü başka yerdedir;
        /// taşınan değer bu yüzden köke göre görelidir. Her öğrenci onu
        /// kendi kökünün altında çözer.
        ///
        /// Alıcı `..` içeren veya kök dışına çıkan değeri reddeder.
        /// Boşsa dosya kökün altındaki `OpenClass` klasörüne iner.
        #[serde(default)]
        subpath: Option<String>,
    },

    /// Dosya parçası. Sıra numarası boşluk tespiti içindir.
    FileChunk {
        transfer_id: Uuid,
        seq: u32,
        #[serde(with = "serde_bytes")]
        data: Vec<u8>,
    },

    /// Tüm parçalar gönderildi; alıcı özeti doğrular.
    FileEnd {
        transfer_id: Uuid,
    },

    /// Aktarım öğretmen tarafından iptal edildi.
    FileCancel {
        transfer_id: Uuid,
        reason: String,
    },

    /// Uzak dosya gezgini açıldı.
    ///
    /// Öğrencide görünür bir gösterge belirir. Gezgin **yalnızca listeleme
    /// ve indirme** yapar; yazma ve silme protokolde tanımlı değildir.
    BrowseStart {
        teacher_name: String,
    },

    /// Klasör içeriği isteniyor.
    ///
    /// `path` boşsa izin verilen köklerin listesi döner.
    BrowseList {
        request_id: Uuid,
        path: String,
    },

    /// Dosya indirilmek isteniyor.
    BrowseFetch {
        request_id: Uuid,
        path: String,
    },

    /// Gezgin kapandı; gösterge kalkar.
    BrowseEnd,

    /// Öğretmen ekranının değişen bölgeleri.
    ///
    /// Öğrenci akışıyla aynı biçim: `keyframe` doğruysa bölgeler tüm kareyi
    /// kapsar, değilse öncekinin üzerine çizilir.
    TeacherScreenUpdate {
        seq: u64,
        width: u32,
        height: u32,
        keyframe: bool,
        codec: FrameCodec,
        rects: Vec<ScreenRect>,
    },

    /// Öğretmen ekranı yayını bitti; gösterim penceresi kapatılır.
    TeacherScreenStop,

    /// Öğretmen uzaktan kontrolü devraldı.
    ///
    /// Öğrenci ekranında kapatılamayan bir gösterge belirir. Kontrol, ders
    /// oturumu bittiğinde veya bağlantı koptuğunda kendiliğinden biter.
    RemoteControlStart {
        teacher_name: String,
    },

    /// Uzaktan kontrol bırakıldı.
    RemoteControlStop,

    /// Öğrenci makinesine uygulanacak girdi olayları.
    ///
    /// Toplu gönderilir: fare hareketi saniyede onlarca olay üretir ve her
    /// birini ayrı mesajda yollamak kanalı gereksiz doldurur.
    RemoteInput {
        events: Vec<InputEvent>,
    },

    /// Bir sonraki güncellemenin anahtar kare olmasını iste.
    ///
    /// Öğretmen tarafı biriken değişiklikleri kaybettiğinde (ör. arayüz geç
    /// kaldı ve tampon taştı) görüntüyü baştan kurabilmek için gönderilir.
    RequestKeyframe {
        stream_id: u32,
    },

    /// Öğretmenden öğrenciye mesaj.
    Notice {
        message_id: Uuid,
        title: String,
        body: String,
    },

    /// Yardım talebinin öğretmen tarafından alındığı bilgisi.
    HelpAcknowledged,

    Ping {
        token: u64,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{decode, encode, MAX_CONTROL_FRAME};

    fn ornek_guncelleme(payload: Vec<u8>) -> ClientMessage {
        ClientMessage::ScreenUpdate {
            stream_id: 1,
            seq: 42,
            width: 1920,
            height: 1080,
            keyframe: true,
            codec: FrameCodec::Jpeg,
            rects: vec![ScreenRect {
                x: 0,
                y: 0,
                width: 1920,
                height: 1080,
                data: payload,
            }],
        }
    }

    #[test]
    fn ekran_verisi_base64_sismesi_olmadan_tasinir() {
        // 100 KiB'lik sahte bir JPEG. MessagePack ikili tipi kullandığı için
        // kodlanmış boyut ham veriden yalnızca birkaç yüz bayt büyük olmalı.
        let payload = vec![0xAB_u8; 100 * 1024];
        let raw = encode(&ornek_guncelleme(payload.clone()), MAX_CONTROL_FRAME).unwrap();
        let overhead = raw.len() - payload.len();
        assert!(overhead < 512, "beklenmeyen ek yük: {overhead} bayt");

        let back: ClientMessage = decode(&raw, MAX_CONTROL_FRAME).unwrap();
        match back {
            ClientMessage::ScreenUpdate {
                rects,
                seq,
                keyframe,
                ..
            } => {
                assert_eq!(seq, 42);
                assert!(keyframe);
                assert_eq!(rects.len(), 1);
                assert_eq!(rects[0].data.len(), payload.len());
            }
            _ => panic!("yanlış mesaj tipi"),
        }
    }

    #[test]
    fn cok_bolgeli_guncelleme_tasinir() {
        let msg = ClientMessage::ScreenUpdate {
            stream_id: 1,
            seq: 7,
            width: 640,
            height: 480,
            keyframe: false,
            codec: FrameCodec::Jpeg,
            rects: vec![
                ScreenRect {
                    x: 0,
                    y: 0,
                    width: 64,
                    height: 64,
                    data: vec![1u8; 300],
                },
                ScreenRect {
                    x: 128,
                    y: 64,
                    width: 128,
                    height: 64,
                    data: vec![2u8; 500],
                },
            ],
        };
        let raw = encode(&msg, MAX_CONTROL_FRAME).unwrap();
        let back: ClientMessage = decode(&raw, MAX_CONTROL_FRAME).unwrap();
        match back {
            ClientMessage::ScreenUpdate {
                rects, keyframe, ..
            } => {
                assert!(!keyframe);
                assert_eq!(rects.len(), 2);
                assert_eq!((rects[1].x, rects[1].y), (128, 64));
                assert_eq!(rects[1].data.len(), 500);
            }
            _ => panic!("yanlış mesaj tipi"),
        }
    }

    #[test]
    fn asiri_buyuk_cerceve_reddedilir() {
        assert!(encode(&ornek_guncelleme(vec![0u8; 1024]), 256).is_err());
    }

    #[test]
    fn sunucu_mesajlari_gidip_gelebilir() {
        let msg = ServerMessage::StartScreenStream {
            stream_id: 7,
            quality: StreamQuality::PREVIEW,
        };
        let raw = encode(&msg, MAX_CONTROL_FRAME).unwrap();
        let back: ServerMessage = decode(&raw, MAX_CONTROL_FRAME).unwrap();
        match back {
            ServerMessage::StartScreenStream { stream_id, quality } => {
                assert_eq!(stream_id, 7);
                assert_eq!(quality, StreamQuality::PREVIEW);
            }
            _ => panic!("yanlış mesaj tipi"),
        }
    }
}

#[cfg(test)]
mod auth_result_tests {
    use super::*;

    #[test]
    fn yetki_kaldirma_bayragi_tasinir() {
        let msg = ServerMessage::AuthResult {
            ok: false,
            reason: Some("Bu cihazın yetkisi kaldırıldı.".into()),
            reset_pairing: true,
        };
        let bytes = crate::encode(&msg, crate::MAX_CONTROL_FRAME).unwrap();
        let back: ServerMessage = crate::decode(&bytes, crate::MAX_CONTROL_FRAME).unwrap();

        match back {
            ServerMessage::AuthResult { reset_pairing, .. } => assert!(reset_pairing),
            other => panic!("beklenmeyen mesaj: {other:?}"),
        }
    }

    #[test]
    fn gecici_hata_eslestirmeyi_silmez() {
        // Yanlış kod veya ağ hatası eşleştirmeyi silmemeli.
        let msg = ServerMessage::AuthResult {
            ok: false,
            reason: Some("Kod yanlış.".into()),
            reset_pairing: false,
        };
        let bytes = crate::encode(&msg, crate::MAX_CONTROL_FRAME).unwrap();
        let back: ServerMessage = crate::decode(&bytes, crate::MAX_CONTROL_FRAME).unwrap();

        match back {
            ServerMessage::AuthResult { reset_pairing, .. } => assert!(!reset_pairing),
            other => panic!("beklenmeyen mesaj: {other:?}"),
        }
    }
}

#[cfg(test)]
mod arayuz_bicimi_testleri {
    //! Arayüzle paylaşılan tiplerin **tel üzerindeki biçimini** kilitler.
    //!
    //! `cargo test` Rust tarafını, `tsc` TypeScript tarafını doğrular; ama
    //! ikisinin birbirine uyduğunu hiçbir şey kontrol etmiyordu. Serde
    //! özniteliği değiştiğinde (ya da eklenmeyi unutulduğunda) her iki
    //! taraf da kendi içinde geçerli kalır, veri sessizce düşer.
    //!
    //! Buradaki testler arayüzün gerçekten ürettiği JSON'u birebir yazar.
    //! Biçim değişirse test kırılır — çalışma anında değil, derlemede.

    use super::*;

    /// Öğretmen arayüzünün `api.sendRemoteInput` ile gönderdiği nesneler.
    ///
    /// `apps/teacher/src/types.ts` içindeki `InputEvent` birliğiyle birebir
    /// aynı olmalı.
    #[test]
    fn arayuzun_gonderdigi_girdi_olaylari_cozulur() {
        let ornekler = [
            (
                r#"{"t":"mouse_move","x":0.25,"y":0.5}"#,
                InputEvent::MouseMove { x: 0.25, y: 0.5 },
            ),
            (
                r#"{"t":"mouse_button","button":"left","pressed":true}"#,
                InputEvent::MouseButton {
                    button: MouseButton::Left,
                    pressed: true,
                },
            ),
            (
                r#"{"t":"mouse_button","button":"right","pressed":false}"#,
                InputEvent::MouseButton {
                    button: MouseButton::Right,
                    pressed: false,
                },
            ),
            (
                r#"{"t":"mouse_button","button":"middle","pressed":true}"#,
                InputEvent::MouseButton {
                    button: MouseButton::Middle,
                    pressed: true,
                },
            ),
            (
                r#"{"t":"mouse_wheel","dx":0,"dy":-1}"#,
                InputEvent::MouseWheel { dx: 0, dy: -1 },
            ),
            (
                r#"{"t":"key","code":65,"pressed":true}"#,
                InputEvent::Key {
                    code: 65,
                    pressed: true,
                },
            ),
        ];

        for (json, beklenen) in ornekler {
            let cozulen: InputEvent = serde_json::from_str(json)
                .unwrap_or_else(|e| panic!("arayüzün gönderdiği {json} çözülemedi: {e}"));
            assert_eq!(cozulen, beklenen, "{json}");
        }
    }

    /// Rust'ın ürettiği biçim de arayüzün beklediğiyle aynı olmalı.
    #[test]
    fn girdi_olaylari_arayuzun_bekledigi_bicimde_uretilir() {
        let uretilen = serde_json::to_string(&InputEvent::MouseMove { x: 1.0, y: 0.0 }).unwrap();
        assert!(
            uretilen.contains(r#""t":"mouse_move""#),
            "varyant adı snake_case olmalı, üretilen: {uretilen}"
        );
    }

    /// Bilinmeyen bir varyant adı sessizce kabul edilmemeli.
    ///
    /// Bu testin amacı, `rename_all` kaldırılırsa yukarıdaki testlerin
    /// gerçekten kırıldığından emin olmak.
    #[test]
    fn pascal_case_varyant_adi_reddedilir() {
        let sonuc: Result<InputEvent, _> =
            serde_json::from_str(r#"{"t":"MouseMove","x":0.5,"y":0.5}"#);
        assert!(
            sonuc.is_err(),
            "arayüz snake_case gönderiyor; PascalCase kabul edilmemeli"
        );
    }

    // ------------------------------------------------- Hello'nun oda alanı

    #[test]
    fn oda_alani_olmayan_hello_cozulur() {
        // 0.1.0 ajanları `room` alanını hiç göndermez. Alan zorunlu olsaydı
        // çözümleme burada patlar ve sahadaki bütün eski kurulumlar
        // bağlanamaz hale gelirdi — sürüm atlamanın en pahalı hatası.
        let eski_ajan = serde_json::json!({
            "t": "Hello",
            "protocol": crate::PROTOCOL_VERSION,
            "device_id": "00000000-0000-4000-8000-000000000001",
            "hostname": "LAB101",
            "app_version": "0.1.0",
            "public_key": vec![0u8; 32],
        });
        let msg: ClientMessage = serde_json::from_value(eski_ajan)
            .expect("oda alanı olmayan Hello çözülebilmeli");
        match msg {
            ClientMessage::Hello { room, hostname, .. } => {
                assert_eq!(room, None, "alan yoksa oda bilinmiyor olmalı");
                assert_eq!(hostname, "LAB101");
            }
            other => panic!("Hello beklenirken {other:?} geldi"),
        }
    }

    #[test]
    fn oda_alani_tasinir() {
        let msg = ClientMessage::Hello {
            protocol: crate::PROTOCOL_VERSION,
            device_id: Uuid::nil(),
            hostname: "LAB101".into(),
            app_version: "0.2.0".into(),
            public_key: vec![3u8; 32],
            room: Some("Lab 1".into()),
        };
        let geri: ClientMessage =
            rmp_serde::from_slice(&rmp_serde::to_vec_named(&msg).unwrap()).unwrap();
        match geri {
            ClientMessage::Hello { room, .. } => {
                assert_eq!(room.as_deref(), Some("Lab 1"));
            }
            other => panic!("Hello beklenirken {other:?} geldi"),
        }
    }
}
