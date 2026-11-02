# Test Planı

## Otomatik testler

```bash
cargo test --workspace      # 73 birim testi
npm run typecheck
cargo clippy --workspace --all-targets -- -D warnings
```

Kapsam: protokol kodlama/çözme, hostname filtreleme, grup ön eki önerisi, eşleştirme kanıtı,
challenge imzalama, parmak izi sabitleme, kare ölçekleme/kodlama, veritabanı
işlemleri, cihaz kayıt defteri, adres ayrıştırma.

**Kapsam dışı:** gerçek ekran yakalama, gerçek ağ trafiği, Windows Güvenlik
Duvarı davranışı, WebView2. Bunlar aşağıdaki elle testlerle doğrulanır.

## Elle testler — Windows gerekir

### Ortam A: 1 öğretmen + 2 öğrenci

Öğrenci VM'lerinin bilgisayar adları `LAB101` ve `LAB102` olmalı, bir tanesi
de `LAB201` olarak ayarlanıp LAB1 aramasından dışlandığı doğrulanmalı.

> **VirtualBox notu:** Ağ modu **Bridged Adapter** olmalı. NAT modunda VM
> kendi izole ağındadır ve UDP yayın keşfi çalışmaz.

| # | Senaryo | Beklenen | Durum |
|---|---|---|---|
| 1 | Teacher ve Student aynı ağda açılır | Student, öğretmeni listede görür | ✅ |
| 2 | Öğretmen `LAB1` arar | Yalnızca LAB101, LAB102 listelenir | ⬜ |
| 3 | `LAB2` bilgisayarı LAB1 aramasında | Görünmez | ⬜ |
| 4 | Öğrenci VM'i yeniden başlatılıp IP değişir | Cihaz yeniden bulunur, yeniden eşleştirme istenmez | ⬜ |
| 5 | Öğrenci eşleşir | Ekranı **kendiliğinden** konsola düşer (ders başlatmak gerekmez) | ✅ |
| 6 | Bir karta çift tıklanır | Çözünürlük ve FPS artar, metin okunur hale gelir | ⬜ |
| 7 | 20 eşzamanlı akış (Ortam B) | CPU/RAM/ağ kabul edilebilir | ⬜ |
| 8 | Ağ kablosu çekilir / VM duraklatılır | İki uygulama da kilitlenmez, yeniden bağlanır | ⬜ |
| 9 | Yetkisiz cihaz bağlanmaya çalışır | Reddedilir, denetim kaydına `auth_failed` düşer | ⬜ |
| 10 | Ders bitirilir | Tüm yayınlar durur, öğrenci göstergesi kapanır | ⬜ |
| 11 | Mesaj + yardım talebi | Doğru cihazlara ulaşır | ⬜ |
| 12 | Teacher kapatılır | Öğrenci ajanı güvenli duruma geçer, çökmez | ⬜ |

### Ek güvenlik testleri

| # | Senaryo | Beklenen | Durum |
|---|---|---|---|
| S1 | Yanlış eşleştirme kodu 5 kez girilir | Kod iptal olur, yenisi gerekir | ⬜ |
| S2 | Kod üretildikten 6 dakika sonra kullanılır | Reddedilir | ⬜ |
| S3 | Öğretmen cihaz yetkisini kaldırır | Bağlantı kopar; öğrenci "eşleştirilmedi" durumuna döner ve yeni kodla yeniden eşleşebilir | ⬜ |
| S4 | Öğretmen konsolu silinip yeniden kurulur (yeni sertifika) | Öğrenci bağlantıyı reddeder, yeniden eşleştirme ister | ⬜ |
| S5 | Ders kapalıyken ekran yayını istenir | Reddedilir | ⬜ |
| S6 | UAC yükseltme istemi açılır | Kare akışı durur, çökme olmaz, görüntü donmuş gösterilmez | ⬜ |

### Uzaktan kontrol, kilit ve dosya (doğrulanmadı)

Bu özelliklerin kodu tamamlandı ama **hiçbiri gerçek donanımda
çalıştırılmadı.** `SendInput` ve `BlockInput` gibi işletim sistemi
çağrıları ancak sahada kanıtlanır.

| # | Senaryo | Beklenen | Durum |
|---|---|---|---|
| K1 | Cihaza sağ tık → Kontrolü al | Büyütülmüş görünüm açılır; öğrencide kapatılamaz şerit belirir | ⬜ |
| K2 | Tuvalde fare gezdirilir, tıklanır | İmleç öğrencide **aynı yere** gider, tıklama uygulanır | ⬜ |
| K3 | Klavyeden yazılır (Türkçe Q klavye) | Harfler doğru düşer — eşleme fiziksel tuşa göre yapılır | ⬜ |
| K4 | Tekerlek çevrilir | Öğrencide kaydırma olur | ⬜ |
| K5 | Büyütülmüş pencere kapatılır | Kontrol kendiliğinden biter, şerit kalkar | ⬜ |
| K6 | Kontrol açıkken ağ kablosu çekilir | Kontrol düşer, şerit kalkar | ⬜ |
| K7 | Kontrol açıkken UAC istemi çıkar | Girdi güvenli masaüstüne **gitmez**, çökme olmaz | ⬜ |
| L1 | Ekranı Kilitle → açıklama yazılır | Öğrencide tam ekran pencere, yazılan metin görünür | ⬜ |
| L2 | Kilitliyken klavye/fare denenir | Tepki vermez | ⬜ |
| L3 | Kilitliyken Ctrl+Alt+Del | **Çalışır** — engellenmemeli | ⬜ |
| L4 | Kilidi Aç | Kilit kalkar, pencere kapanır | ⬜ |
| L5 | Kilitliyken öğretmen konsolu kapatılır | Kilit kendiliğinden kalkar | ⬜ |
| L6 | `BlockInput` reddedilirse | Günlüğe `girdi engeli uygulanamadı` düşer; yönetici olarak denenir | ⬜ |
| D1 | Dosya Gönder → bir PDF seçilir | Öğrenciye bildirim gelir, `Belgeler\OpenClass`'a düşer | ⬜ |
| D2 | Aynı dosya tekrar gönderilir | Üzerine yazmaz, `(2)` ekiyle kaydedilir | ⬜ |
| D3 | Gönderimler penceresi | Her alıcının durumu ayrı satırda, özet doğrulandı yazar | ⬜ |
| D4 | Disk doluyken gönderilir | Öğrenci reddeder, sebebi konsolda görünür | ⬜ |
| G1 | Sağ tık → Dosyalarını görüntüle | Kök listesi gelir; öğrencide gösterge belirir | ⬜ |
| G2 | Klasörlerde gezilir | İçerik listelenir, üst klasöre çıkılır | ⬜ |
| G3 | Bir dosya indirilir | `Belgeler\OpenClass\Toplanan\<ad>` altına düşer, özet doğrulanır | ⬜ |
| G4 | Öğrenciye indirme bildirimi | Her indirmede bildirim çıkar | ⬜ |
| G5 | Kapsam dışına çıkma denenir | `C:\Windows` reddedilir | ⬜ |

### Ekran yayını ve multicast (doğrulanmadı)

> Öğretmen konsolu **Windows'ta** çalışmalı; Linux'ta gerçek ekran
> yakalama yoktur, yalnızca test deseni yayınlanır.

| # | Senaryo | Beklenen | Durum |
|---|---|---|---|
| Y1 | Ekranımı Yayınla → penceresiz | Öğrencide tam ekran, üstte kalır | ⬜ |
| Y2 | Ekranımı Yayınla → pencereli | Öğrenci Alt+Tab ile çıkabilir | ⬜ |
| Y3 | Durum çubuğu | `Yayın · multicast` yazar (UDP 7646 açıkken) | ⬜ |
| Y4 | UDP 7646 kapatılır | 4 sn sonra `multicast (1 yedek)` olur, görüntü **kesilmez** | ⬜ |
| Y5 | Yayın sürerken öğrenci bağlanır | Geç katılan da görüntüyü alır | ⬜ |
| Y6 | Yayın durdurulur | Öğrencide pencere kapanır | ⬜ |

### Otomatik başlatma

| # | Senaryo | Beklenen | Durum |
|---|---|---|---|
| O1 | Yapılandırmada kutu işaretlenir | Kayıt defterinde `OpenClass Student` görünür | ⬜ |
| O2 | Windows yeniden başlatılır | Ajan kendiliğinden açılır, eşleştirme korunur | ⬜ |
| O3 | Kutu kaldırılır | Kayıt defteri girdisi silinir | ⬜ |

### Ortam B: 1 öğretmen + 20 öğrenci (okul laboratuvarı)

`LAB101`–`LAB120`. Ölçülecekler:

- Öğretmen bilgisayarında CPU ve RAM kullanımı
- Her öğrenci bilgisayarında CPU kullanımı
- Toplam ağ trafiği (önizleme modunda ve bir ekran büyütülmüşken)
- Keşif turunun tamamlanma süresi
- Ders başlatmadan ilk karenin gelmesine kadar geçen süre

Ölçümler bu dosyadaki ilgili senaryonun altına not edilir.
