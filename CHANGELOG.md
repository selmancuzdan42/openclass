# Değişiklik Günlüğü

Bu dosya [Keep a Changelog](https://keepachangelog.com/tr/1.1.0/) biçimini,
sürüm numaraları [Semantic Versioning](https://semver.org/lang/tr/) kurallarını izler.

## [0.5.2] — 2026-11-26

### Eklendi
- **Onaylı hata raporu.** Uygulama çökerse nedeni kaydedilir; öğretmen
  konsolu bir sonraki açılışta raporun gönderilmesini sorar ve gidecek
  bilginin tamamını gösterir. Onay verilmezse hiçbir şey gönderilmez. Rapor
  bilgisayar adı, IP adresi ya da ekran içeriği taşımaz.
- **Bağlantı kopma bildirimi.** Bir öğrencinin bağlantısı kopunca öğretmen
  konsolunda bildirim çıkar; aynı anda düşenler tek bildirimde toplanır.

### Değiştirildi
- Öğrenci ajanı ilk çalıştırmada otomatik başlatmayı kendiliğinden açar.
  Sonradan elle kapatılırsa kapalı kalır.

### Düzeltildi
- Öğretmen konsolu kapatılıp açıldığında bazı öğrencilerde ekran yerine
  "DXGI Desktop Duplication başlatılamadı: Parametre hatalı (0x80070057)"
  çıkıyordu. Bağlantı kopunca ekran yakalama durdurulmuyordu ve Windows
  ikinci yakalamayı reddediyordu.

## [0.5.1] — 2026-11-25

### Eklendi
- **Öğrenci ekranını sınıfa yansıtma.** Seçilen öğrencinin ekranı diğer
  öğrencilere gösterilir. Görüntü yeniden kodlanmadan öğretmen yayınının
  taşıma yoluna (multicast + TLS yedeği) aktarılır; izleyen ajanların eski
  sürümde olması sorun değildir. Ekranı gösterilen öğrenciye bildirilir.
- **Ekran görüntüsü kaydetme.** Büyütülmüş görünümden ya da sağ tık
  menüsünden o anki öğrenci ekranı `Resimler/OpenClass` altına PNG olarak
  kaydedilir ve denetim günlüğüne yazılır. Ekranlar kendiliğinden hâlâ
  diske yazılmaz.
- **USB bellek kısıtlama** (Windows). Kısıtlama oturuma bağlıdır: bağlantı
  kopunca ya da ajan yeniden başlayınca kendiliğinden kalkar. Okulun önceden
  kapattığı ayar geri açılmaz. Klavye ve fare etkilenmez. Öğrenciye her
  değişiklik bildirilir.
- **Uzaktan yeniden başlatma.**

### Değiştirildi
- Ajan ve konsol tanımadıkları mesajı atlayıp bağlantıyı korur. Önceden
  bilinmeyen bir komut oturumu koparıyordu.
- Yeni komutlar (yeniden başlatma, USB kısıtlama) yalnızca 0.5.1 ve üstü
  ajanlara gönderilir; eski ajanlar için konsol uyarı verir.

## [0.5.0] — 2026-11-25

### Eklendi
- **Sınıf seçim penceresi.** Konsol her açılışta hangi sınıfın yönetileceğini
  sorar (NetSupport'un Class Wizard'ı gibi). Kayıtlı sınıflar listelenir;
  sınıf, bilgisayar adı başlangıcıyla (`LAB1`) ya da oda adıyla tanımlanır.
  "Açılışta sorma" ile son sınıfla doğrudan açılır; durum çubuğundaki ya da
  başlıktaki sınıf adına tıklanarak ders sırasında değiştirilir.
- **Bilgisayar adından sınıf önerisi.** Öğretmen bilgisayarının adı
  `LAB1-OGR` gibi bilinen bir ekle bitiyorsa `LAB1` sınıfı önerilir.

### Değiştirildi
- **Laboratuvar ayrımı artık bağlantı düzeyinde.** Konsol yönettiği kapsamı
  (oda adı / ad başlangıcı) keşif paketinde duyurur. Öğrenci yalnızca kendisini
  kapsayan konsola başvurur; konsol yalnızca kapsamına giren yeni cihazı
  kendiliğinden yetkilendirir. Aynı ağdaki iki laboratuvar birbirine karışmaz.
- **Otomatik kabul koda gömülü `LAB`/`BLAB` önekine bağlı değil.** Ölçüt
  öğretmenin seçtiği sınıftır; `BIL101`, `PC-01` gibi adlandırmalar da çalışır.
  Henüz sınıf seçilmemiş konsol kimseyi kendiliğinden kabul etmez; "Tüm
  bilgisayarlar" ayrı ve açık bir seçenektir.
- Arayüz rötuşları: kartlarda gölge ve yuvarlak köşe, pencere açılış
  animasyonu, yeni başlık çubuğu.

### Düzeltildi
- Küçük pencerede küçük resimler sıkışıp bilgisayar adı şeridi kayboluyordu.
- Ön eki boş (yalnızca odaya göre eşleşen) grup kaydedilemiyordu.
- Bölmeli seçicilerin ve bilgi kutularının arka plan rengi tanımsızdı.

## [0.2.0] — 2026-11-03

### Eklendi
- **Açılışta laboratuvar seçimi.** Konsol ilk açılışta hangi bilgisayarların
  yönetileceğini sorar: laboratuvar adı, bilgisayar adı ön eki veya tümü.
  Ölçüt yazılır — ağda henüz makine görünmese de girilebilir, bilgisayarlar
  açıldıkça süzgece takılırlar. Ayarlar'dan değiştirilir, **Yeniden sor** ile
  ekran tekrar getirilir.
- **Oda adı keşif duyurusunda taşınıyor.** Öğretmen bir laboratuvarı
  eşleştirme yapmadan, cihaz bağlanmadan önce süzebilir.
- **IP aralığı taraması.** Ek keşif adresleri `192.168.1.10-40` biçimini
  kabul eder; yayının engellendiği ağlarda tek tek unicast probe gönderilir.
  En fazla 254 adres, yalnızca son sekizlide aralık.
- **Laboratuvar/oda adı.** Öğrenci ilk kurulumda bulunduğu laboratuvarın adını
  girer; öğretmen konsolu cihazları buna göre gruplar. Listede **Oda** sütunu,
  aramada oda adı, Grup Ekle penceresinde oda önerileri.
- **İlk açılış sihirbazı** (öğrenci). İki adım: laboratuvar adı, ardından
  eşleştirme. Yalnızca oda adı **ve** eşleştirme ikisi de yokken açılır;
  0.1.0'dan yükselen eşleşmiş cihazlar sihirbaz görmez.
- **İndirme klasörü seçimi.** Uzak gezginden alınan dosyalar öğretmenin
  seçtiği klasöre toplanabilir; seçim oturum boyunca korunur, bilgisayar adı
  alt klasörü korunur.
- Veritabanı şema sürümleme (`PRAGMA user_version`). Geçişler yeniden
  çalıştırılabilir; yarıda kalan geçiş sonraki açılışta tamamlanır.

### Değiştirildi
- **Ders oturumu kavramı kaldırıldı.** "Dersi Başlat/Bitir" düğmeleri,
  `SessionStart`/`SessionEnd` protokol mesajları, `sessions` tablosu ve
  oturuma bağlı bütün kapılar gitti. Konsol açıkken bağlanan bilgisayarların
  önizlemesi kendiliğinden gelir, konsol kapanınca yayın durur.

  Öğrenci tarafında paylaşımın gizlenmesini engelleyen tek koruma artık
  **görünür göstergedir**: tepsi simgesi ve "EKRANINIZ PAYLAŞILIYOR" yazısı.
  Bu gösterge hiçbir mesajla bastırılamaz.
- **Dosya gönderiminde hedef, öğrencinin gezgininden seçiliyor.** Üç seçenekli
  liste yerine öğrencinin klasörlerinde gezilip hedef işaretleniyor. Birden
  çok cihaza gönderirken seçim köke göre göreli taşınır: `Belgeler\Ödevler`
  her öğrencinin kendi klasöründe çözülür.
- **Açılışta laboratuvar seçimi.** Konsol ilk açılışta hangi laboratuvarın
  yönetileceğini sorar; liste o odaya süzülür. Seçim Ayarlar'dan
  değiştirilebilir, boş bırakılırsa bütün bilgisayarlar listelenir.
- Şeritte **Sınıf → Dosyalar** grubu: `Dosya Gönder` · `Dosya Al` ·
  `Gönderimler` yan yana. Daha önce alma komutu (`Dosya Gezgini`) Yönetim
  sekmesindeydi ve bulunması zordu.
- Gruplama önceliği: **oda adı > bilgisayar adı öneki**. Odası bilinen cihaz
  yalnızca o odanın grubunda görünür.
- Şeritte klavye odağı halkası, basılı durum geri bildirimi ve daha okunur
  grup başlıkları.

### Düzeltildi
- `CREATE TABLE IF NOT EXISTS` var olan bir veritabanında sütun eklemediği
  için yeni alanlar sessizce eksik kalıyordu; şema sürümleme bunu kapattı.

## [0.1.0] — 2026-11-02

### Eklendi
- Monorepo iskeleti: `crates/protocol`, `crates/common`, `apps/teacher`, `apps/student`.
- Ağ protokolü: UDP keşif, TLS + WebSocket kontrol kanalı, MessagePack çerçeveleme.
- Cihaz eşleştirme: 6 haneli tek kullanımlık kod, Ed25519 cihaz kimliği,
  TLS sertifika parmak izine kanal bağlama.
- Öğretmen konsolu: cihaz keşfi, ön ek araması (`LAB1` → `LAB101`…),
  cihaz ızgarası, canlı ekran önizleme, ekran büyütme, mesajlaşma,
  yardım talepleri, ders oturumu yönetimi, denetim kayıtları.
- Öğrenci ajanı: hostname okuma, keşif cevabı, eşleştirme, otomatik yeniden
  bağlanma, Windows Graphics Capture (DXGI Desktop Duplication yedekli)
  ile ekran yakalama, görünür paylaşım göstergesi, yardım talebi.
- **UDP multicast yayın.** Kare telden bir kez geçiyor; 30 öğrenciye
  gönderirken 27 MB/s yerine ~918 KB/s. Yük ChaCha20-Poly1305 ile
  şifreleniyor, oturum anahtarı yalnızca TLS kanalından dağıtılıyor.
  Paketi ulaşmayan öğrenci kendini bildiriyor ve ona TLS unicast yapılıyor.
- **Pencereli / penceresiz yayın modu.** Penceresiz modda öğretmen ekranı
  öğrencinin ekranını kaplar; pencereli modda öğrenci Alt+Tab ile not
  alabilir.
- **Öğrenci klavye/fare kilidi.** Ekranda nedenini açıklayan pencere durur,
  bir saatlik zaman aşımı var, Ctrl+Alt+Del engellenmiyor, ders bitince
  kendiliğinden kalkıyor.
- **Dosya gönderme.** TLS kanalından 64 KiB'lık parçalar, SHA-256
  doğrulaması, disk payı kontrolü, öğrenciye bildirim. Dosya adı
  temizleniyor ve var olan dosyanın üzerine yazılmıyor.
- **Uzak dosya gezgini.** Yalnızca listeleme ve indirme; yazma/silme
  protokolde bile yok. Kapsam öğrenci tarafında kısıtlı, her listeleme ve
  indirme denetim kaydında, öğrencide görünür gösterge var.
- **Uzaktan fare/klavye/scroll kontrolü.** Öğretmen büyütülmüş görünümde
  kontrolü devralabiliyor. Koordinatlar normalleştirilmiş gönderiliyor,
  olaylar ~30 Hz'de toplu iletiliyor. Öğrenci ekranında kapatılamayan bir
  gösterge duruyor; kontrol ders bitince, pencere kapanınca veya bağlantı
  kopunca kendiliğinden sona eriyor.
- **Otomatik başlatma** (Windows `Run` anahtarı, gizlenmeden, yalnızca
  geçerli kullanıcı için).
- **IP adresine göre arama**: arama kutusu hem bilgisayar adını hem IP'yi
  kabul ediyor.
- **Öğretmen ekranı yayını.** Öğretmen kendi ekranını bağlı tüm öğrencilere
  yayınlayabiliyor; öğrencide tam ekran, üstte kalan ve kimin yayınladığını
  yazan bir pencere açılıyor. Kare bir kez kodlanıp çok sayıda alıcıya
  gönderiliyor. Multicast henüz yok, gönderim öğrenci başına.
- **Paylaşılan `openclass-screen` crate'i** ve `@openclass/screen-view`
  paketi: yakalama/kodlama/çizim artık iki uygulamanın ortak kodu.
- Kontrol portu bağlanamadığında öğretmen konsolu artık sessizce bozuk
  çalışmıyor; kalıcı bir uyarı şeridi gösteriyor. Yerel test betiği de
  ikinci bir örnek başlatmayı reddediyor.
- `ocframe://` ve `ocbcast://` yanıtlarına CORS başlığı (fetch için gerekli).
- **Değişen bölge aktarımı.** Ekran artık tam kare olarak değil, 64×64
  karolara bölünüp değişen dikdörtgenler hâlinde gönderiliyor. Ölçülen
  kazanç: durağan ekranda trafik sıfır, metin yazarken 62×, pencere
  kaydırırken 9,5×, tam hareketli içerikte 2,8× azalma. Birleştirme
  öğretmen tarafında tarayıcı tuvalinde yapılır; Rust hiç görüntü işlemez.
- Akış başına verim tavanı (`StreamQuality::max_kbps`), periyodik anahtar
  kare ve öğretmenin anahtar kare isteyebilmesi.
- Başarım: kaynak piksellerde sıfır kopya, tam sayı katlarında kutu
  filtresiyle hızlı küçültme, 1,5x altı oranlarda hiç ölçekleme yapmama.
  Öğrenci ajanı penceresiz çalışıyor (WebView talep üzerine açılır).
- Laboratuvar grupları: bilgisayar adı ön ekine dayalı, kalıcı, ağdaki
  adlardan otomatik öneri üreten grup çubuğu.
- Öğretmen konsolu arayüzü sınıf yönetimi yazılımlarının bilgi mimarisine
  göre yeniden kuruldu: komut şeridi, grup çubuğu, görünüm rayı, durum
  çubuğu, küçük resim ızgarası ↔ ayrıntılı tablo, sağ tık menüsü
  (bkz. `docs/ARAYUZ.md`).
- Öğrenci ajanı sınıf yönetimi yazılımlarının modeline geçirildi: pencere
  yerine **sistem tepsisi**. Simge bağlantı ve ekran paylaşımı durumunu
  gösterir, menüden yardım talebi gönderilir. Pencere yalnızca
  **yapılandırma** içindir ve öğretmen/yönetici tarafından kullanılır;
  öğretmen mesajları açılır pencerede gösterilir.
- Arayüzü Rust derlemeden geliştirebilmek için tarayıcıda çalışan sahte
  arka uç (`src/dev-mock.ts`, yalnızca geliştirme derlemesinde).
- Windows kurulum paketleri (NSIS), Linux'tan cross-compile desteği.

[0.1.0]: https://github.com/selmancuzdan42/openclass/releases/tag/v0.1.0
