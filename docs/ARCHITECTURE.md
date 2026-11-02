# OpenClass Mimarisi

Bu belge teknik kararların **gerekçelerini** kaydeder. Ne yapıldığını kod
anlatır; burada neden öyle yapıldığı yazar.

## 1. Genel yapı

Sistemde yalnızca iki çalıştırılabilir dosya vardır:

| Dosya | Rol |
|---|---|
| `OpenClass-Teacher.exe` | Öğretmen konsolu **ve** bağlantı sunucusu |
| `OpenClass-Student.exe` | Öğrenci ajanı |

Üçüncü bir sunucu programı yoktur. Öğretmen uygulaması açıldığında Rust
arka planında TCP dinleyicisini ve UDP keşif servisini başlatır; kapandığında
bunları düzgün biçimde sonlandırır.

```
openclass/
├── crates/protocol/   # İki uygulamanın paylaştığı TEK sözleşme
├── crates/common/     # Yollar, loglama, TLS kimliği, DPAPI
├── apps/teacher/      # Konsol: React arayüz + Rust sunucu
└── apps/student/      # Ajan: React arayüz + Rust istemci + ekran yakalama
```

`crates/*` bağımsız uygulama sayılmaz; yalnızca kod paylaşımıdır.

## 2. Üç katman, bilinçli olarak ayrı

```
Katman 1 — KEŞİF        UDP 7644, şifresiz, yetki vermez
Katman 2 — KİMLİK       Ed25519 + TLS parmak izi bağlama
Katman 3 — KONTROL/VERİ TLS + WebSocket, MessagePack
```

**Neden ayrı?** Okul ağlarının bir kısmı broadcast/multicast paketlerini keser.
Keşif çalışmasa bile bağlantı kurulabilmelidir: öğrenci, öğretmenin IP adresini
elle alarak doğrudan bağlanabilir. Keşif bir kolaylıktır, önkoşul değildir.

Aynı ayrım güvenlik için de gereklidir: bir cihazın kendini `LAB101` olarak
duyurması, onun yönetilebilir olduğu anlamına gelmez.

## 3. Keşif: neden öğretmen yayın yapıyor?

İki yön de mümkündü. Öğretmenin yayın yapması seçildi:

- 20 öğrenci sürekli duyuru yaparsa ağda sabit gereksiz trafik olur.
  Öğretmen yalnızca arama yaptığında trafik doğar.
- Öğretmen konsolu kapalıyken ağda hiç OpenClass paketi dolaşmaz.
- Cevaplar **unicast** döndüğü için öğretmen cihazın gerçek IP'sini doğrudan
  öğrenir — DHCP ile IP değişse bile sorun olmaz.

Öğretmen soketi **gelişigüzel bir porta** bağlanır, 7644'e değil. Böylece
geliştirme sırasında öğretmen ve öğrenci aynı makinede çalışabilir.

### Yayın engellenirse

1. Öğretmen ayarlarından **ek keşif adresleri** girilebilir (alt ağ yayın
   adresleri veya tek tek IP'ler); bunlara unicast probe gönderilir.
2. Öğrenci tarafından **sabit öğretmen adresi** girilebilir; keşif tamamen atlanır.

## 4. Kimlik ve eşleştirme

### Neden hostname yetmez?

Bilgisayar adı taklit edilebilir ve okul içinde değişebilir. Yetkilendirmenin
dayanağı cihaza özgü bir **Ed25519 anahtar çiftidir**. Hostname yalnızca
öğretmene gösterilen bir etikettir.

### Eşleştirme akışı

```
Öğretmen: 6 haneli kod üretir (tek kullanımlık, 5 dk, 5 deneme)
Öğrenci : TLS bağlanır → sunucu sertifikasının parmak izini (FP) görür
Öğrenci : proof = HMAC-SHA256(kod, "pairing" ‖ FP ‖ device_id ‖ pubkey)
Öğretmen: aynı HMAC'i hesaplar, sabit zamanlı karşılaştırır
Öğretmen: pubkey'i SQLite'a yazar, cihaz artık yetkili
Öğrenci : FP'yi kalıcı olarak sabitler (pinning)
```

**Kanıt neden parmak izini içeriyor?** Araya giren biri kendi sertifikasını
sunarsa, öğrencinin ürettiği kanıt saldırganın parmak izine bağlanır ve
öğretmenin hesabıyla tutmaz. Kanıt başka bir TLS oturumuna taşınamaz.

### Sonraki bağlantılar

```
Öğretmen: rastgele 32 baytlık challenge gönderir
Öğrenci : sign(Ed25519, "challenge" ‖ FP ‖ nonce)
Öğretmen: kayıtlı pubkey ile doğrular
```

Her oturumda yeni challenge → yeniden oynatma (replay) mümkün değil.
Parmak izi imzaya dahil → relay mümkün değil.

### Kabul edilen zayıflık

6 haneli kod bir PAKE değildir. Yakalanan bir kanıt üzerinde 10⁶ olasılık
çevrimdışı denenebilir. Koruma, kodun tek kullanımlık ve kısa ömürlü olması
ile TLS'in gizliliğidir. SPAKE2'ye geçiş planlanan iyileştirmedir.

## 5. Neden CA yok?

Okulda sertifika otoritesi kurmak gerçekçi değil. Öğretmen konsolu kendinden
imzalı sertifika üretir; güven, eşleştirme anında parmak izinin sabitlenmesine
dayanır (TOFU + eşleştirme kodu onayı). Sertifikanın alan adı, süresi ve
zinciri **kasıtlı olarak** doğrulanmaz — bunlar bir CA hiyerarşisi varsayar.
Kimlik tamamen parmak izinden gelir.

Özel anahtar Windows'ta **DPAPI** ile korunur; uygulamanın saklaması gereken
ikinci bir sır yoktur.

## 6. Mesaj biçimi: neden MessagePack?

| Seçenek | Neden seçilmedi / seçildi |
|---|---|
| JSON | Ekran karesi Base64'e dönerdi: %33 şişme + ayrıştırma maliyeti |
| JSON kontrol + ayrı ikili kare | İki kod yolu, iki hata kaynağı |
| **MessagePack** | ✅ Tek `enum`, ikili veriyi doğal taşır, ayıklanabilir |
| Protobuf | Şema derleme adımı; bu ölçekte kazanç yok |

`serde_bytes` sayesinde JPEG verisi ham bayt olarak gider. 100 KiB'lik bir
karenin kodlama ek yükü 512 baytın altındadır (birim testle doğrulanıyor).

## 7. Ekran yakalama

### Arka uç seçimi

1. **Windows Graphics Capture (WGC)** — birincil. Windows 10 1803+ ile gelir,
   DWM üzerinden çalışır, sanal makinelerde daha uyumludur.
2. **DXGI Desktop Duplication** — yedek. WGC başlatılamazsa denenir.

İkisi de başarısız olursa açık bir hata öğretmene iletilir. **Sahte görüntü
üretilmez.**

> VirtualBox/VMware sanal ekran sürücüleri DXGI Desktop Duplication'ı sık sık
> reddeder. WGC'nin birincil olması bu yüzden önemlidir.

### Kare hattı

```
Yakalama iş parçacığı → [tek kareli yuva] → FPS zamanlayıcı
                                              ↓
                              spawn_blocking: ölçekle + JPEG
                                              ↓
                                     try_send (kuyruk 8)
```

Tasarım kararları:

- **Tek kareli yuva.** Gönderici yetişemezse ara kareler düşer; bellek sabit kalır.
- **`try_send`.** Kuyruk doluysa kare atılır, yayın bloke olmaz.
- **Değişmeyen kare atlanır.** Ölçeklenmiş piksellerin özeti bir öncekiyle
  aynıysa kare hiç kodlanmaz ve gönderilmez. Durağan ekranda bant genişliği
  neredeyse sıfıra iner.
- **`spawn_blocking`.** JPEG kodlaması CPU yoğundur; async çalıştırıcıyı bloklamaz.

### Kalite profilleri

| Profil | Çözünürlük | FPS | JPEG kalitesi |
|---|---|---|---|
| Önizleme (ızgara) | ≤ 480×270 | 2 | 55 |
| Odaklı (büyütülmüş) | ≤ 1600×900 | 12 | 75 |

Bunlar **hedeftir, garanti değildir**; gerçek cihazlarda ölçülecektir.

### Neden saf Rust JPEG?

`turbojpeg-sys` (libjpeg-turbo) daha hızlıdır ama `cmake` + `nasm` ile C
derlemesi gerektirir ve Linux'tan Windows'a cross-compile'ı kırılgan hale
getirir. `jpeg-encoder` saf Rust'tır; derleme hattı temiz kalır. Ölçüm
gerektiğinde değiştirilebilir — protokolde `FrameCodec` enum'u zaten ayrılmıştır.

## 8. Kareler arayüze nasıl ulaşıyor?

Tauri IPC'si JSON'dur; kareyi oradan geçirmek Base64 şişmesini geri getirirdi.
Bunun yerine öğretmen uygulaması `ocframe://` özel URI şeması kaydeder:

```
<img src="ocframe://localhost/<device-id>?seq=<n>">
```

- Görüntü ham JPEG olarak iner, tarayıcı kod çözmeyi kendi yapar.
- `seq` yalnızca önbelleği atlatmak içindir.
- Arayüze giden olay yalnızca "yeni kare var" bilgisidir — 20 cihaz × 2 FPS
  altında JSON trafiği ihmal edilebilir kalır.

Windows'ta Tauri özel şemaları `http://ocframe.localhost` altında sunar;
taban adres `frame_base_url` komutuyla arayüze bildirilir.

## 9. Eşzamanlılık kuralı

`parking_lot` kilit guard'ları `Send` değildir. Bir guard `await` noktasını
geçerse görev `tokio::spawn` edilemez. Kural:

> Kilitten okunan değer **önce** kopyalanır, kilit bırakılır, sonra `await` edilir.

Kod tabanında bu kuralın uygulandığı yerler yorumla işaretlidir.

## 10. Veri saklama

| Veri | Nerede | Koruma |
|---|---|---|
| Cihaz kayıtları, denetim günlüğü | SQLite (`%APPDATA%\OpenClass\teacher\`) | Dosya sistemi |
| TLS özel anahtarı | Ayrı dosya | DPAPI (kullanıcı kapsamı) |
| Öğrenci cihaz kimliği | Ayrı dosya | DPAPI (makine kapsamı) |
| Ekran görüntüleri | **Hiçbir yerde** | Diske yazılmaz |

Öğrenci kimliği neden makine kapsamlı? Laboratuvar bilgisayarında farklı
Windows hesaplarıyla oturum açan öğrenciler aynı cihaz kimliğini görmelidir;
aksi hâlde her hesap ayrı bir cihaz gibi davranır ve öğretmen aynı makineyi
defalarca eşleştirmek zorunda kalır.

## 11. Loglama

- Günlük döndürme, 14 dosya sınırı.
- Seviye `OPENCLASS_LOG` ortam değişkeniyle değiştirilebilir.
- **Asla loglanmayanlar:** parola, eşleştirme kodu, özel anahtar, cihaz tohumu,
  ekran içeriği, öğrenci kişisel verisi.

## 12. Planlanan iyileştirmeler

| Konu | Öncelik |
|---|---|
| Gerçek Windows cihazlarda uçtan uca doğrulama | **Kritik** |
| SPAKE2 ile eşleştirme | Orta |
| Donanımsal video kodlama (H.264) / WebRTC | Orta |
| `if-addrs` ile alt ağ yayın adreslerinin otomatik bulunması | Orta |
| Öğretmen ekranını yansıtma | Aşama 2 |
| İzinli uzaktan kontrol | Aşama 2 |
