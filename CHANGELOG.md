# Değişiklik Günlüğü

Bu dosya [Keep a Changelog](https://keepachangelog.com/tr/1.1.0/) biçimini,
sürüm numaraları [Semantic Versioning](https://semver.org/lang/tr/) kurallarını izler.

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
