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

### 1. Dersi başlat

Üst çubukta **Dersi Başlat**. Ekran izleme yalnızca ders açıkken çalışır —
bu bilinçli bir güvenlik kısıtıdır.

### 2. Bilgisayarları bul

**Bilgisayarları Ara** ekranında arama kutusuna laboratuvar ön ekini yazın:

| Yazdığınız | Bulunanlar |
|---|---|
| `LAB1` | LAB101, LAB102 … LAB120 |
| `LAB2` | LAB201, LAB202 … LAB220 |
| `LAB105` | Yalnızca LAB105 |
| `LAB` | Yetkili tüm LAB bilgisayarları |
| (boş) | Tüm cihazlar |

Büyük/küçük harf fark etmez.

### 3. Ekranları aç

**Ekranları Göster**'e basın. Seçili cihaz yoksa işlem bağlı tüm cihazlara
uygulanır.

Kart üzerindeki renkli nokta:

- 🟢 Bağlı
- 🟠 Ağda görünüyor ama bağlı değil
- ⚪ Çevrimdışı

Görüntü yerine açıklama görüyorsanız sebep orada yazar. **Sahte ekran
görüntüsü hiçbir zaman gösterilmez.**

### 4. Bir ekranı büyüt

Kartına **çift tıklayın**. O cihaz yüksek kaliteye (12 FPS, 1600×900),
diğerleri önizlemeye geçer. Pencereyi kapatınca hepsi önizlemeye döner.

### 5. Mesaj gönder

**Mesajlar** ekranı. Kart üzerindeki kutularla cihaz seçebilirsiniz; seçim
yoksa mesaj bağlı tüm cihazlara gider.

### 6. Yardım taleplerini karşıla

Öğrenci, tepsi simgesine sağ tıklayıp **Yardım İste**'yi seçtiğinde kartında
turuncu rozet belirir ve sol raydaki sayaç artar. **Yardım** ekranından
**Görüldü** ile kapatın.

### 7. Dersi bitir

**Dersi Bitir**. Tüm ekran yayınları durur ve öğrencilerin göstergesi kapanır.

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
