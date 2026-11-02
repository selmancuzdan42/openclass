# Güvenlik Politikası

## Açık bildirimi

Güvenlik açığı bulduysanız **herkese açık issue açmayın.** Bunun yerine
depo sahibine GitHub üzerinden özel güvenlik bildirimi (Security Advisory)
gönderin.

Bildirimde şunları belirtin:

1. Etkilenen bileşen (`teacher`, `student`, `protocol`, `common`)
2. Açığın türü ve etkisi
3. Tekrar üretme adımları
4. Varsa kavram kanıtı (PoC)

## Tehdit modeli

OpenClass'ın savunduğu senaryolar:

- Aynı yerel ağdaki bir saldırganın kendini öğrenci olarak tanıtıp öğretmen
  konsoluna kabul ettirmesi
- Yetkisiz bir konsolun gerçek öğrenci cihazlarını yönetmesi
- Ağı dinleyen birinin ekran görüntülerini okuması
- Ortadaki adam saldırısıyla bağlantının ele geçirilmesi

OpenClass'ın **savunmadığı** senaryolar:

- Öğrenci bilgisayarında yönetici yetkisine sahip bir kullanıcı. Cihaz kimliği
  o makinede saklanır; makineyi ele geçiren kişi kimliği de ele geçirir.
- Öğretmen bilgisayarının fiziksel olarak ele geçirilmesi.
- Okul ağ altyapısının tamamen saldırgan kontrolünde olması.

## Uygulanan korumalar

| Koruma | Nasıl |
|---|---|
| Taşıma şifrelemesi | TLS 1.2/1.3 (rustls + ring) |
| Sunucu kimliği | Kendinden imzalı sertifikanın SHA-256 parmak izi eşleştirme anında sabitlenir |
| Cihaz kimliği | Ed25519 anahtar çifti; özel anahtar Windows DPAPI ile korunur |
| Yeniden oynatma (replay) | Her oturumda rastgele challenge imzalanır |
| Relay saldırısı | İmzalar TLS sertifika parmak izini kapsar (kanal bağlama) |
| Eşleştirme kötüye kullanımı | Tek kullanımlık kod, 5 dakika ömür, 5 hatalı deneme sınırı |
| Bellek tüketimi | Çerçeve boyutu sınırı (8 MiB), cihaz başına tek kare, dolu kuyrukta kare düşürme |
| Veri sızıntısı | Ekran görüntüleri diske yazılmaz; loglara anahtar/parola/kod yazılmaz |
| Gizli izleme | Tepsi simgesi her zaman görünür ve paylaşım sırasında değişir; simgeyi gizleyen bir seçenek yoktur |

## Bilinen sınırlamalar

- **Eşleştirme kodu bir PAKE değildir.** 6 haneli kod, yakalanan bir kanıt
  üzerinde çevrimdışı denenebilir. Koruma kodun tek kullanımlık ve kısa ömürlü
  olmasından gelir. SPAKE2'ye geçiş planlanan bir iyileştirmedir.
- **İlk eşleştirmede TOFU** kullanılır. Araya giren biri kendi sertifikasını
  sunarsa eşleştirme kanıtı tutmaz ve bağlantı reddedilir; ancak bu, kodun
  doğru öğretmenden alındığı varsayımına dayanır.
- **Cihaz kimliği makine kapsamlıdır** (DPAPI `LOCAL_MACHINE`). Laboratuvar
  bilgisayarında oturum açabilen bir öğrenci kimliği okuyabilir. Bu kabul
  edilen bir ödünleşimdir: o öğrenci zaten o cihazın kendisidir.
- **Ajan kurcalamaya karşı korumalı değildir.** Tepsi menüsünde "Çıkış"
  bulunmaz, ancak süreç görev yöneticisinden sonlandırılabilir. Bunu
  engellemek işletim sistemi ilkesi (Grup İlkesi, hizmet olarak çalıştırma)
  gerektirir ve bu projenin kapsamı dışındadır. Gizleme ya da kalıcılık
  numaraları kullanılmayacaktır.

## Kapsam dışı

Bu projeye şu özellikler **bilinçli olarak eklenmeyecektir**:

- Gizli / görünmez izleme (NetSupport'taki "Quiet Mode" benzeri bir seçenek
  bilinçli olarak eklenmemiştir)
- Keylogger, parola toplama
- Kamera veya mikrofon erişimi
- Güvenlik yazılımı atlatma, kalıcılık gizleme
- Windows güvenli masaüstü (UAC, kilit ekranı) korumalarını aşma

Bu yönde gelen katkı önerileri reddedilir.
