# Öğrenci Kurulum Rehberi

> Bu rehber **öğretmen ve sistem yöneticisi** içindir. Öğrenci günlük
> kullanımda hiçbir ayar yapmaz.

## Kurulum

1. `OpenClass Student_0.1.0_x64-setup.exe` dosyasını yönetici olarak çalıştırın.
2. Windows Güvenlik Duvarı sorduğunda **Özel ağlarda izin ver** seçin.
3. Bilgisayar adının laboratuvar düzenine uyduğundan emin olun:

```powershell
Rename-Computer -NewName "LAB101" -Restart
```

Uygulama bilgisayar adını otomatik okur; elle girmeniz gerekmez.

## İlk kurulum (bilgisayar başına bir kez)

İlk açılışta **kurulum sihirbazı** gelir. İki adımdır.

### 1. adım — laboratuvar adı

Bu bilgisayarın bulunduğu laboratuvarın adını yazın, örneğin `Lab 1` veya
`201 Nolu Atölye`. Öğretmen konsolu bilgisayarları bu ada göre gruplar, bu
yüzden **aynı laboratuvardaki bütün bilgisayarlara aynı adı** yazın.

Ad yalnızca listelemeye yarar; yetki kararında kullanılmaz. Öğretmen yine de
her bilgisayarı tek tek eşleştirmek zorundadır.

**Şimdilik atla**'ya basarsanız oda adı boş kalır — öğretmen listesinde "—"
görünür. Sonradan yapılandırma penceresindeki **Laboratuvar / oda** kartından
girebilirsiniz.

### 2. adım — eşleştirme

1. Öğretmen konsolunda **Cihaz Eşleştir**'e basın; ekranda 6 haneli bir kod belirir.
2. Öğrenci bilgisayarında öğretmeni listeden seçin.
   - Liste boşsa öğretmenin IP adresini **adres** kutusuna yazın.
3. Kodu girip **Eşleştir ve Bağlan**'a basın.
4. **Kapat**'a basın. Pencere gizlenir, ajan tepside çalışmaya devam eder.

Eşleştirme kalıcıdır. IP adresi değişse veya bilgisayar yeniden başlatılsa
bile tekrarlanmaz.

Sihirbaz yarıda kapatılırsa yalnızca kaydedilmiş adımlar kalır: oda adı diske
yazılmıştır, eşleştirme yazılmamıştır.

> **0.1.0'dan yükseltenler:** zaten eşleştirilmiş bir cihazda sihirbaz
> açılmaz. Oda adı boş kalır; girmek isterseniz yapılandırma penceresini
> tepsi simgesinden açın.

## Öğrencinin gördüğü tek şey: tepsi simgesi

Öğrenci tarafında pencere yoktur. Saat yanındaki tepsi simgesi durumu gösterir:

| Simge | Anlamı |
|---|---|
| Soluk ekran + kırmızı çapraz | Öğretmene bağlı değil |
| Net ekran | Bağlı, ekran paylaşımı kapalı |
| Net ekran + **yeşil nokta** | **Ekran paylaşılıyor** |

Simgenin üzerine gelince ipucu metni bilgisayar adını, bağlantı durumunu ve
paylaşım durumunu yazar.

### Sağ tık menüsü

```
Bağlı — Cihan Güngör  ·  EKRANINIZ PAYLAŞILIYOR
──────────────────────────────────────────────
Yardım İste
──────────────────────────────────────────────
Yapılandırma…
```

* **Yardım İste** — öğretmen konsolunda bilgisayarın yanında işaret belirir.
  Aynı menüden *Yardım talebini geri çek* ile iptal edilir.
* **Yapılandırma…** — bu rehberdeki ayar penceresini açar. Simgeye çift
  tıklamak da aynı işi yapar.

Tepsi menüsünde **Çıkış yoktur**: ders ortasında öğrencinin tek tıkla
izlemeyi kapatabilmesi istenmez. Ajanı durdurma düğmesi yapılandırma
penceresindedir.

> Bu bir kurcalama koruması değildir. Kararlı bir kullanıcı süreci görev
> yöneticisinden sonlandırabilir. Bunu engellemek Grup İlkesi veya hizmet
> olarak çalıştırma gerektirir ve bu sürümün kapsamı dışındadır.

### Öğretmen mesajları

Öğretmen mesaj gönderdiğinde küçük bir pencere açılır. Öğrenci okur,
**Tamam**'a basar, pencere kapanır. Mesajlar ekranda birikmez.

## Ekran paylaşımı ne zaman açılır?

Ekran, öğretmen konsolu açık ve bu bilgisayar ona bağlıyken paylaşılır.

Paylaşım başladığında tepsi simgesi yeşil noktaya döner ve "EKRANINIZ
PAYLAŞILIYOR" göstergesi çıkar. **Bu gösterge kapatılamaz** — paylaşımın
gizlice yapılmasını engelleyen tek şey odur. Öğretmen konsolu kapandığında
veya bağlantı koptuğunda paylaşım kendiliğinden durur, simge eski hâline
döner.

**Simgeyi gizleyen bir seçenek yoktur.** Bazı ticari yazılımlarda bulunan
"sessiz mod" benzeri gizleme özelliği OpenClass'ta bilinçli olarak
bulunmaz ve eklenmeyecektir.

## Gizlilik

OpenClass şunları **yapmaz**:

- Ekran görüntülerini diske kaydetmez
- Tuş vuruşlarını kaydetmez
- Kamera veya mikrofona erişmez
- Dosyaları okumaz
- Göstergesi olmadan ekran paylaşmaz

## Laboratuvara toplu dağıtım

```powershell
Start-Process -Wait -FilePath ".\OpenClass Student_0.1.0_x64-setup.exe" -ArgumentList "/S"
```

> **Önemli:** Her bilgisayar ayrı eşleştirilmelidir. Eşleştirme cihaza özgü
> bir anahtar üretir; disk görüntüsü klonlanırsa tüm makineler aynı cihaz
> kimliğini taşır ve konsolda tek cihaz gibi görünür. Klonlama yapacaksanız
> görüntüyü **OpenClass kurulu ama hiç çalıştırılmamış** hâlde alın.

## Sorun giderme

| Belirti | Çözüm |
|---|---|
| Tepsi simgesi yok | Ajan çalışmıyor olabilir. Başlat menüsünden OpenClass Student'ı açın. |
| Simge hep kırmızı çaprazlı | Öğretmen konsolu kapalı veya ağ erişimi yok. Simgeye çift tıklayıp durumu okuyun. |
| Öğretmen listede yok | Yapılandırma penceresinde **adres** kutusuna öğretmenin IP'sini yazın. |
| "Eşleştirme kodu hatalı" | Kodun süresi dolmuş olabilir; öğretmenden yeni kod isteyin. |
| "Sertifika eşleştirme anındakinden farklı" | Öğretmen uygulaması yeniden kurulmuş. Yapılandırmadan **Eşleştirmeyi kaldır** deyip yeniden eşleştirin. |
| "Ekran yakalama başlatılamadı" | Sanal makinede olabilirsiniz; öğretmene bildirin. |

Log dosyaları: `%APPDATA%\OpenClass\OpenClass\student\logs\`
