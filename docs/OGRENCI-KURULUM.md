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

## Eşleştirme (bilgisayar başına bir kez)

İlk açılışta **yapılandırma penceresi** gelir. Eşleştirilmiş bir cihazda bu
pencere açılmaz; ajan doğrudan tepside çalışmaya başlar.

1. Öğretmen konsolunda **Cihaz Eşleştir**'e basın; ekranda 6 haneli bir kod belirir.
2. Öğrenci bilgisayarındaki yapılandırma penceresinde öğretmeni listeden seçin.
   - Liste boşsa öğretmenin IP adresini **adres** kutusuna yazın.
3. Kodu girip **Eşleştir ve Bağlan**'a basın.
4. **Kapat**'a basın. Pencere gizlenir, ajan tepside çalışmaya devam eder.

Eşleştirme kalıcıdır. IP adresi değişse veya bilgisayar yeniden başlatılsa
bile tekrarlanmaz.

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

Ekran **yalnızca** iki koşul birlikte sağlandığında paylaşılır:

1. Öğretmen bir ders oturumu başlatmış olmalı,
2. Öğretmen o bilgisayarın ekranını açıkça göstermeyi seçmiş olmalı.

Paylaşım başladığında tepsi simgesi yeşil noktaya döner. Ders bittiğinde
paylaşım kendiliğinden durur ve simge eski hâline geri gelir.

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
