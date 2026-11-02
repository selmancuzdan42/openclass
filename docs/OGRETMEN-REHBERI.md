# Öğretmen Kullanım Rehberi

## İlk açılış

1. **OpenClass Teacher**'ı açın.
2. Windows Güvenlik Duvarı sorduğunda **Özel ağlarda izin ver**'i seçin.
3. **Ayarlar** → adınızı ve varsayılan sınıf adını girin, **Kaydet**.

Üst çubuktaki yeşil nokta ve "N bağlı cihaz" yazısı bağlantı sunucusunun
çalıştığını gösterir.

## Cihaz eşleştirme (her bilgisayar için bir kez)

1. Üst çubukta **Cihaz Eşleştir**'e basın. 6 haneli bir kod belirir.
2. Öğrenci bilgisayarında tepsi simgesine çift tıklayıp yapılandırma
   penceresini açın (ilk kurulumda zaten açık gelir).
3. Listeden konsolunuzu seçin; görünmüyorsa pencerede yazan IP adresini girin.
4. Kodu girip **Eşleştir ve Bağlan**'a basın, sonra pencereyi kapatın.

Kod **tek kullanımlıktır** ve 5 dakika geçerlidir. Her bilgisayar için yeni
kod üretin. Eşleştirme bir kez yapılır; IP değişse bile tekrarlanmaz.

## Ders yürütme

Ayrı bir "ders başlat" adımı yoktur. Konsol açıldığında bağlanan
bilgisayarların ekran önizlemesi kendiliğinden gelir; konsolu kapattığınızda
yayın durur.

> Öğrenci ekranının paylaşıldığını **her zaman görür**: tepsi simgesi değişir
> ve "EKRANINIZ PAYLAŞILIYOR" göstergesi çıkar. Bu gösterge kapatılamaz ve
> hiçbir ayarla bastırılamaz.

### 1. Bilgisayarları bul

**Bilgisayarları Ara** ekranında arama kutusuna bilgisayar adı ön ekini, oda
adını veya IP adresini yazın:

| Yazdığınız | Bulunanlar |
|---|---|
| `LAB1` | LAB101, LAB102 … LAB120 |
| `LAB105` | Yalnızca LAB105 |
| `Lab 1` | Oda adı "Lab 1" olan bütün bilgisayarlar |
| `192.168.1.` | O adres aralığındaki cihazlar |
| (boş) | Tüm cihazlar |

Büyük/küçük harf fark etmez.

#### Gruplar ve oda adı

Cihazlar iki ölçütle gruplanır ve **oda adı önce gelir**:

1. **Oda adı** — öğrencinin ilk kurulumda girdiği laboratuvar adı. Grup adıyla
   aynıysa cihaz o gruba düşer.
2. **Bilgisayar adı ön eki** — odası bilinmeyen cihazlar için. `LAB1` ön eki
   `LAB101`, `LAB102` … cihazlarını kapsar.

Odası bilinen bir cihaz yalnızca o odanın grubunda görünür; öneki başka bir
grupla eşleşse bile oraya düşmez.

Oda adı `DESKTOP-4F9K2A` gibi varsayılan adlara sahip laboratuvarlarda tek
çalışan yöntemdir. Listedeki **Oda** sütunu boşsa (—) o bilgisayarda oda adı
girilmemiştir; öğrenci tarafındaki yapılandırma penceresinden girilebilir.

**Grup Ekle** penceresi hem bildirilen odaları hem ağdaki adlardan çıkarılan
ön ekleri önerir. Yalnızca odaya göre eşleşen bir grup için ön ek alanını boş
bırakın.

### 2. Ekranları aç

**Ekranları Göster**'e basın. Seçili cihaz yoksa işlem bağlı tüm cihazlara
uygulanır.

Kart üzerindeki renkli nokta:

- 🟢 Bağlı
- 🟠 Ağda görünüyor ama bağlı değil
- ⚪ Çevrimdışı

Görüntü yerine açıklama görüyorsanız sebep orada yazar. **Sahte ekran
görüntüsü hiçbir zaman gösterilmez.**

### 3. Bir ekranı büyüt

Kartına **çift tıklayın**. O cihaz yüksek kaliteye (12 FPS, 1600×900),
diğerleri önizlemeye geçer. Pencereyi kapatınca hepsi önizlemeye döner.

### 4. Dosya gönder ve dosya al

Şeritteki **Sınıf → Dosyalar** grubunda üç komut var.

**Dosya Gönder** — dosyayı seçtikten sonra **öğrencinin dosya gezgini
açılır** ve hedef klasörü oradan seçersiniz. Gezinti Masaüstü, Belgeler ve
İndirilenler ile sınırlıdır; sistem klasörlerine erişilemez.

Birden çok cihaza gönderirken seçim ilk bilgisayarın gezgininde yapılır ama
taşınan değer **köke göre görelidir**: `Belgeler\Ödevler` seçerseniz dosya
her öğrencinin kendi `Belgeler\Ödevler` klasörüne iner. Kökün kendisini
seçerseniz altındaki `OpenClass` klasörü kullanılır.

Aynı adlı dosya varsa üzerine yazılmaz, yeni bir ad verilir (`ders.pdf` →
`ders (2).pdf`). Öğrenciye her gönderim, dosyanın hangi klasöre indiğiyle
birlikte bildirilir.

**Dosya Al** — tek bir cihaz seçiliyken etkindir. Öğrencinin Masaüstü,
Belgeler, İndirilenler ve OpenClass klasörlerinde gezip dosya indirebilirsiniz.
**Yalnızca listeleme ve indirme**: yazma, silme ve yeniden adlandırma yoktur.
İndirilenler `Belgeler\OpenClass\Toplanan\<bilgisayar adı>` altına düşer;
**Klasörü Değiştir** ile başka bir yere toplayabilirsiniz (seçim o oturum
boyunca korunur). Öğrencide "dosyalarınıza bakılıyor" göstergesi durur ve her
indirme denetim kaydına yazılır.

**Gönderimler** — yürüyen ve bitmiş aktarımların durumu.

### 5. Mesaj gönder

**Mesajlar** ekranı. Kart üzerindeki kutularla cihaz seçebilirsiniz; seçim
yoksa mesaj bağlı tüm cihazlara gider.

### 6. Yardım taleplerini karşıla

Öğrenci, tepsi simgesine sağ tıklayıp **Yardım İste**'yi seçtiğinde kartında
turuncu rozet belirir ve sol raydaki sayaç artar. **Yardım** ekranından
**Görüldü** ile kapatın.

## Cihaz yönetimi

**Cihaz Yönetimi** ekranında:

- Her bilgisayara **görünen ad** verebilirsiniz ("1. sıra sol" gibi).
- **Yetkiyi kaldır** ile bir cihazın erişimini kesebilirsiniz. Bağlıysa
  bağlantısı anında kopar ve yeniden eşleştirilmeden bağlanamaz.

## Sorun giderme

| Belirti | Olası sebep ve çözüm |
|---|---|
| Hiç bilgisayar görünmüyor | Ağ yayını engelleniyor olabilir. Ayarlar → **Ek keşif adresleri** alanına alt ağın yayın adresini (`192.168.1.255`) ekleyin. Ya da öğrenciye konsolunuzun IP'sini elle girdirin. |
| "Bağlantı sunucusu başlatılamadı" | 7643 portu başka bir uygulamada veya güvenlik duvarı engelliyor. |
| Cihaz ağda görünüyor ama bağlanmıyor | Henüz eşleştirilmemiş olabilir. **Cihaz Eşleştir** ile ekleyin. |
| "Ekran yakalama başlatılamadı" | Sanal makinede sanal ekran sürücüsü desteklemiyor olabilir. Gerçek donanımda tekrar deneyin. |
| Görüntü donuk, "N sn önce" yazıyor | Öğrenci bilgisayarında UAC istemi veya kilit ekranı açık olabilir; Windows bu ekranları yakalatmaz. |

Ayrıntılı kayıtlar: **Ayarlar → Denetim kayıtları**.
