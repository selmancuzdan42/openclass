# OpenClass

**NetSupport School'a ücretsiz ve açık kaynak alternatif.** Okul bilgisayar
laboratuvarları için tasarlanmış sınıf yönetim yazılımı: öğretmen yerel ağdaki
öğrenci bilgisayarlarını bulur, güvenli biçimde eşleştirir ve ders
boyunca ekranlarını canlı izler.

> **Durum:** v0.6.1 yayında. Aşağıdaki "Mevcut durum" bölümü neyin
> çalıştığını açıkça listeler.

## Neden?

Okullar sınıf yönetimi için pahalı, kapalı kaynak lisanslara bağımlı. OpenClass:

- **Tamamen ücretsiz ve açık kaynak** (GPL-3.0-or-later)
- **Sunucu gerektirmez** — öğretmen uygulaması bağlantı sunucusunu kendi içinde barındırır
- **İnternet gerektirmez** — yerel ağda çalışır, bulut hesabı veya abonelik yok
- **İki uygulamadan ibaret** — `OpenClass-Teacher.exe` ve `OpenClass-Student.exe`
- **Gizli izleme yok** — ekran paylaşımı öğrenciye her zaman görünür

## Mevcut durum

### Çalışanlar

| Özellik | Durum |
|---|---|
| Monorepo, iki ayrı derlenebilir uygulama | ✅ |
| Protokol katmanı (keşif / kimlik / kontrol) | ✅ birim testli |
| Cihaz eşleştirme (6 haneli kod + Ed25519 + parmak izi sabitleme) | ✅ birim testli |
| Hostname ön ek araması (`LAB1` → `LAB101`…) | ✅ birim testli |
| Değişen bölge (tile) aktarımı, anahtar kare, verim tavanı | ✅ birim testli |
| SQLite cihaz kaydı ve denetim günlüğü | ✅ birim testli |
| Öğretmen konsolu arayüzü (komut şeridi, grup çubuğu, durum çubuğu) | ✅ |
| Laboratuvar grupları + ad ön ekinden otomatik öneri | ✅ birim testli |
| Küçük resim ızgarası ↔ ayrıntılı tablo, sağ tık menüsü | ✅ |
| Öğrenci ajanı: sistem tepsisi + yalnızca yapılandırma penceresi | ✅ |
| Öğretmen ekranını öğrencilere yayınlama (tek kodlama, çok gönderim) | ✅ |
| İzinli uzaktan fare/klavye/scroll kontrolü | ✅ |
| UDP multicast yayın (şifreli, TLS yedekli) | ✅ |
| Pencereli / penceresiz yayın modu | ✅ |
| Öğrenci klavye/fare kilidi (zaman aşımlı) | ✅ |
| Öğrencilere dosya gönderme (özet doğrulamalı) | ✅ |
| Uzak dosya gezgini (yalnızca listeleme + indirme) | ✅ |
| Otomatik başlatma (Windows) | ✅ Task Scheduler, yönetici |
| İnternet engelleme + tarayıcıda "İnternet kesildi" uyarı sayfası | ✅ Windows |
| Klavye tuşlarını canlı izleme (şeffaf) | ✅ Windows |
| NetSupport tarzı otomatik bağlanma (konsolun seçtiği oda / ad ön ekine uyan cihazlar kod sormadan bağlanır) | ✅ |
| Toplu dosya toplama ("Topla") — kök + desen, kaynaktan silme seçenekli | ✅ |
| Uzaktan bilgisayar kapatma ve yeniden başlatma | ✅ `shutdown.exe` |
| Öğrenci ekranını sınıfa yansıtma (yeniden kodlamadan aktarım) | ✅ |
| Öğrenci ekranının görüntüsünü dosyaya kaydetme (öğretmen komutuyla) | ✅ |
| USB bellek kısıtlama (bağlantı kopunca kendiliğinden kalkar) | ✅ Windows — gerçek donanımda henüz doğrulanmadı |
| Native Windows dosya kutuları (gönder: dosya+hedef · al: uzak gezgin + hedef) | ✅ |
| Zorla kapatınca kendinden yeniden açılma (watchdog) | ✅ Windows |
| Windows ekran yakalama (WGC + DXGI yedek) | ✅ gerçek Windows'ta doğrulandı |
| Linux'tan Windows'a cross-compile + NSIS kurulum paketi | ✅ |

### Henüz yapılmadı

- Sınav / anket / yoklama modülleri
- Linux'ta gerçek ekran yakalama (öğretmen ekranını yayınlamak Windows
  gerektirir; Linux'ta yalnızca test deseni yayınlanabilir)
- Çoklu monitör desteği

> **Dürüst durum raporu.** Gerçek Windows donanımında uçtan uca
> doğrulananlar: keşif, eşleştirme, TLS bağlantısı, ekran yakalama ve
> öğrenci ekranının öğretmen konsolunda görüntülenmesi. v0.4.0 özellikleri
> (internet engelleme, uzaktan kapatma, toplu dosya toplama, Task Scheduler
> otomatik başlatma, tarayıcı uyarı sayfası) gerçek laboratuvar donanımında
> doğrulandı.

## Videolar

| Tanıtım (20 sn) | Kurulum (34 sn) |
|---|---|
| [![Tanıtım videosu](docs/assets/tanitim.jpg)](https://selmancuzdan42.github.io/openclass/#video) | [![Kurulum videosu](docs/assets/kurulum.jpg)](https://selmancuzdan42.github.io/openclass/#kurulum) |

Görsele tıklayınca video [tanıtım sitesinde](https://selmancuzdan42.github.io/openclass/) açılır.
Dosyalar: [`tanitim.mp4`](docs/assets/tanitim.mp4) · [`kurulum.mp4`](docs/assets/kurulum.mp4)

## Ekran görüntüleri

| Öğretmen konsolu | Öğrenci yapılandırması |
|---|---|
| ![Öğretmen konsolu](docs/ekran-goruntuleri/ogretmen-konsolu-acik.png) | ![Öğrenci yapılandırma penceresi](docs/ekran-goruntuleri/ogrenci-yapilandirma.png) |

Öğrenci normal kullanımda bu pencereyi görmez; ajan sistem tepsisinde çalışır.

Koyu tema ve ayrıntılı liste görünümü:
[`docs/ekran-goruntuleri/`](docs/ekran-goruntuleri/)

> Görüntüler arayüz geliştirme için kullanılan sahte veriyle alınmıştır;
> ekran içerikleri gerçek öğrenci masaüstleri değildir.

## Hızlı başlangıç

### Öğretmen

1. `OpenClass-Teacher.exe` kurun ve açın.
2. Windows Güvenlik Duvarı sorarsa **Özel ağlarda izin ver** seçin.
3. **Cihaz Eşleştir** → ekranda beliren 6 haneli kodu öğrenci bilgisayarına girin.
4. Eşleşen bilgisayarın ekranı **kendiliğinden** gelir; bir karta çift
   tıklayarak büyütebilirsiniz.
5. Arama kutusuna bilgisayar adı (`LAB1`) veya IP parçası (`192.168.1.`)
   yazarak süzebilirsiniz.

Ayrı bir "ders başlat" adımı yoktur: konsol açıkken bağlanan bilgisayarların
önizlemesi gelir, konsolu kapatınca yayın durur. Öğrenci ekranının
paylaşıldığını her zaman görür — tepsi simgesi değişir ve "EKRANINIZ
PAYLAŞILIYOR" göstergesi çıkar.

### Öğrenci (kurulumu öğretmen/yönetici yapar)

1. `OpenClass-Student.exe` kurun. İlk açılışta yapılandırma penceresi gelir.
2. Listeden öğretmen konsolunu seçin (görünmüyorsa adresi elle girin).
3. Öğretmenin verdiği 6 haneli kodu girip **Eşleştir ve Bağlan**'a basın.
4. Pencereyi kapatın. Ajan artık sistem tepsisinde çalışır.

Öğrenci günlük kullanımda hiçbir ayar yapmaz. Tepsi simgesi bağlantı ve
ekran paylaşımı durumunu gösterir; sağ tık menüsünden yardım isteyebilir.

Ayrıntılı rehberler: [`docs/`](docs/)

## Mimari

```
                    UDP 7644/7645 (keşif)
   ┌─────────────────┐ ◄──────────────► ┌──────────────────┐
   │ OpenClass       │                  │ OpenClass        │
   │ Teacher         │ ◄══════════════► │ Student          │
   │ (konsol+sunucu) │  TCP 7643        │ (ajan)           │
   │                 │  TLS+WebSocket   │                  │
   │                 │  MessagePack     │                  │
   │                 │                  │                  │
   │                 │ ──────────────►  │                  │
   └─────────────────┘  UDP 7646        └──────────────────┘
                        multicast
                        (ChaCha20-Poly1305)
```

- **Keşif** şifresiz UDP'dir ve hiçbir yetki vermez.
- **Yetkilendirme** Ed25519 cihaz anahtarı + TLS parmak izi bağlamasıyla yapılır.
- **Ekran verisi** yalnızca değişen bölgeler hâlinde, ham JPEG olarak ikili WebSocket çerçevelerinde taşınır (Base64 yok).
- **Öğretmen ekranı yayını** UDP multicast ile gider: kare telden bir kez
  geçer. Yük ChaCha20-Poly1305 ile şifrelenir, oturum anahtarı yalnızca
  TLS kanalından dağıtılır. Paketi ulaşmayan öğrenciye TLS unicast yapılır.

Tasarım kararlarının gerekçeleri: [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) ·
arayüz yerleşimi: [`docs/ARAYUZ.md`](docs/ARAYUZ.md)

## Teknoloji

Tauri v2 · Rust · React 19 · TypeScript · Tailwind CSS v4 · Tokio · rustls ·
SQLite · Windows Graphics Capture

## Geliştirme

```bash
npm install
cargo test --workspace          # 160 birim testi
npm run typecheck
npm run dev:teacher             # arayüz geliştirme (Linux'ta da çalışır)

# Arayüzü Rust derlemeden, tarayıcıda sahte veriyle açmak için:
npm run dev --workspace @openclass/teacher   # http://127.0.0.1:1420
```

Windows paketleri (Linux'tan):

```bash
cd apps/teacher && cargo tauri build --runner cargo-xwin --target x86_64-pc-windows-msvc --bundles nsis
cd apps/student && cargo tauri build --runner cargo-xwin --target x86_64-pc-windows-msvc --bundles nsis
```

Ayrıntı: [`docs/GELISTIRME.md`](docs/GELISTIRME.md)

## Güvenlik

Açık bildirimi için [`SECURITY.md`](SECURITY.md). Özet:

- Bağlantılar TLS ile şifrelenir; sertifika eşleştirme anında sabitlenir.
- Cihazlar hostname ile değil, cihaza özgü Ed25519 anahtarıyla doğrulanır.
- Ekran görüntüleri kendiliğinden diske **kaydedilmez**. Yalnızca öğretmen
  "Ekran görüntüsü al" dediğinde o anki kare `Resimler/OpenClass` altına
  yazılır ve denetim günlüğüne işlenir.
- Tepsi simgesi her zaman görünür ve paylaşım sırasında değişir; simgeyi
  gizleyen bir seçenek yoktur.
- Ders bittiğinde tüm yayınlar, kilitler ve uzaktan kontrol durur.
- Uzaktan kontrol ve ekran kilidi sırasında öğrencide kapatılamayan bir
  gösterge durur; kilidin zaman aşımı vardır ve Ctrl+Alt+Del engellenmez.
- Uzak dosya gezgini **yalnızca listeler ve indirir** — yazma ve silme
  protokolde tanımlı değildir; kapsam öğrenci tarafında kısıtlanır.
- Uygulama çökerse nedeni kaydedilir ve öğretmene **sorulur**; yalnızca onay
  verirse geliştiricilere gönderilir. Rapor hata mesajı, koddaki yeri, sürüm
  ve işletim sistemini taşır; bilgisayar adı, IP adresi ve ekran içeriği
  içermez.
- Gizli erişim, keylogger, parola toplama veya güvenlik yazılımı atlatma
  mekanizması **yoktur ve eklenmeyecektir**.

## Katkı

[`CONTRIBUTING.md`](CONTRIBUTING.md)

## Lisans

[GPL-3.0-or-later](LICENSE). Tüm bağımlılıklar izin verici lisanslıdır
(MIT / Apache-2.0 / ISC / kamu malı) ve GPL-3.0 ile uyumludur.
