# Windows Kurulum Rehberi

## Desteklenen sistemler

- Windows 10 64-bit (sürüm 1803 veya üzeri)
- Windows 11 64-bit

32-bit Windows desteklenmez.

## Gereksinimler

### WebView2 Runtime

OpenClass arayüzü Microsoft Edge WebView2 kullanır.

- **Windows 11:** hazır gelir, bir şey yapmanıza gerek yok.
- **Windows 10:** güncel sistemlerde genellikle kuruludur. Değilse
  Microsoft'un "Evergreen Standalone Installer" paketini kurun.

Kurulum paketi WebView2'yi kontrol eder ve eksikse uyarır.

### Ağ portları

| Port | Protokol | Kim dinler | Ne için |
|---|---|---|---|
| 7643 | TCP | **Öğretmen** | TLS + WebSocket kontrol kanalı |
| 7645 | UDP | **Öğretmen** | Öğrencilerin keşif cevapları |
| 7644 | UDP | **Öğrenci** | Öğretmenin keşif sorgusu |

> Öğretmen konsolunun keşif portu (7645) bilinçli olarak sabittir. Gelişigüzel
> bir port kullanılsaydı güvenlik duvarına kural yazılamaz, keşif yalnızca
> güvenlik duvarı kapalıyken çalışırdı.

İlk açılışta Windows Güvenlik Duvarı izin isteyecektir. **Özel ağlar** için
izin verin. Okul ağı "Genel ağ" olarak tanımlıysa o kutuyu da işaretleyin.

Kural elle eklenecekse (yönetici PowerShell):

```powershell
# Öğretmen bilgisayarında
New-NetFirewallRule -DisplayName "OpenClass Teacher" -Direction Inbound `
  -Program "C:\Program Files\OpenClass Teacher\OpenClass-Teacher.exe" `
  -Action Allow -Profile Private,Domain

# Öğrenci bilgisayarlarında
New-NetFirewallRule -DisplayName "OpenClass Student" -Direction Inbound `
  -Program "C:\Program Files\OpenClass Student\OpenClass-Student.exe" `
  -Action Allow -Profile Private,Domain
```

### Öğretmen konsolu Linux'ta çalışıyorsa

Geliştirme sırasında öğretmen konsolu Linux'ta koşabilir. O makinede güvenlik
duvarı açıksa (CachyOS'ta `ufw` genelde etkindir) şu kurallar gerekir:

```bash
# Kontrol kanalı — bu olmadan hiçbir öğrenci bağlanamaz
sudo ufw allow from 192.168.1.0/24 to any port 7643 proto tcp

# Keşif cevapları — bu olmadan öğrenciler listede kendiliğinden görünmez
# (elle adres girerek yine bağlanılabilir)
sudo ufw allow from 192.168.1.0/24 to any port 7645 proto udp
```

Kuralları yerel ağa kısıtlamak, portları internete açmaktan iyidir.

> **En sık yapılan hata:** yalnızca SSH portunu (22) açıp 7643'ü unutmak.
> Bu durumda öğrenci "eşleştirilemedi" der, sebebi anlaşılmaz; paket sessizce
> düşmüştür.

## Kurulum

1. `OpenClass Teacher_0.1.0_x64-setup.exe` → öğretmen bilgisayarına
2. `OpenClass Student_0.1.0_x64-setup.exe` → her öğrenci bilgisayarına

Kurulum paketleri **makine geneline** (`per-machine`) kurar; yönetici yetkisi
gerekir. Böylece bilgisayarda oturum açan her öğrenci aynı kurulumu kullanır.

### İmzasız uygulama uyarısı

v0.1 kod imzalama sertifikası ile imzalanmamıştır. Windows SmartScreen
"Bilinmeyen yayımcı" uyarısı gösterebilir: **Ek bilgi → Yine de çalıştır**.

## Laboratuvara toplu dağıtım

Kurulum paketi NSIS'tir ve sessiz kurulumu destekler:

```powershell
Start-Process -Wait -FilePath ".\OpenClass Student_0.1.0_x64-setup.exe" `
  -ArgumentList "/S"
```

Grup İlkesi veya bir dağıtım aracıyla bu komut tüm laboratuvara gönderilebilir.

> **Önemli:** Her bilgisayarın ayrı ayrı eşleştirilmesi gerekir. Eşleştirme
> cihaza özgü bir anahtar üretir; disk görüntüsü (image) klonlanırsa tüm
> makineler aynı cihaz kimliğini taşır ve öğretmen konsolunda tek cihaz gibi
> görünür. Klonlama yapıyorsanız görüntüyü **OpenClass kurulu ama hiç
> çalıştırılmamış** hâlde alın.

## Bilgisayar adlandırma

OpenClass laboratuvarları IP ile değil **bilgisayar adıyla** ayırır.
Önerilen düzen:

```
LAB 1 → LAB101, LAB102, ... LAB120
LAB 2 → LAB201, LAB202, ... LAB220
```

Farklı bir düzen de çalışır (`BIL101`, `PC-101`, `ATOLYE01`, `SINIF-A-01`);
arama ön ek eşleşmesi yapar ve hiçbir desen koda gömülü değildir.

Bilgisayar adı değiştirme (yönetici PowerShell):

```powershell
Rename-Computer -NewName "LAB101" -Restart
```

## Kaldırma

Ayarlar → Uygulamalar → OpenClass Teacher / Student → Kaldır.

Yapılandırma ve cihaz kimliği `%APPDATA%\OpenClass\` altında kalır; tamamen
temizlemek için bu klasörü de silin.


## Multicast portu (öğrenci makinesi)

Ekran yayını UDP multicast ile gider; öğrenci makinesinde **UDP 7646**
gelen yönde açık olmalı:

```powershell
New-NetFirewallRule -DisplayName "OpenClass multicast" `
  -Direction Inbound -Protocol UDP -LocalPort 7646 -Action Allow
```

Port kapalıysa yayın çalışmaya devam eder: öğrenci paket gelmediğini fark
edip öğretmenden TLS yedeği ister. Öğretmen konsolunun durum çubuğunda kaç
cihazın yedeğe düştüğü görünür.

Laboratuvar anahtarı **IGMP snooping** yapıyorsa multicast trafiğinin
engellenmediğinden emin olun; aksi hâlde tüm cihazlar TLS yedeğine düşer ve
bant genişliği kazancı kaybedilir.
