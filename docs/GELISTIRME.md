# Geliştirme ve Derleme

## Gereksinimler

### Her platformda

- Rust (rustup ile), Node.js 20+, Git

### Windows'ta geliştirme (önerilen)

```powershell
# Visual Studio Build Tools 2022 → "Desktop development with C++"
rustup default stable-x86_64-pc-windows-msvc
cargo install tauri-cli --version "^2.0" --locked
npm install
```

WebView2 Runtime: Windows 11'de hazır, Windows 10'da gerekebilir.

### Linux'tan Windows'a cross-compile

```bash
sudo pacman -S rustup lld clang                  # Arch / CachyOS
rustup default stable
rustup target add x86_64-pc-windows-msvc
cargo install tauri-cli --version "^2.0" --locked
cargo install cargo-xwin --locked
npm install
```

Kurulum paketi (NSIS) üretmek için ayrıca `makensis` gerekir:

```bash
yay -S nsis        # Arch (AUR)
sudo apt install nsis   # Debian / Ubuntu
```

`makensis` yoksa `.exe` yine üretilir, yalnızca kurulum paketi atlanır.

#### Arch / CachyOS: AUR'daki `nsis` paketi çalışmaz

AUR paketi yalnızca amd64 kipinde derlenmiştir (`TARGET_ARCH=amd64`), bu
yüzden `/usr/share/nsis/Plugins` altında yalnızca `amd64-unicode` bulunur.
Tauri'nin kullandığı `nsis_tauri_utils.dll` ise 32 bit (PE32 i386) ve
`x86-unicode` klasörü bekler. `makensis` `!addplugindir` ile verilen yolu
yok sayıp yalnızca kendi mimari klasörünü taradığı için paketleme şu hatayla
düşer:

```
Plugin not found, cannot call nsis_tauri_utils::SemverCompare
```

Çözüm, paketi x86 kipinde yeniden derlemektir:

```bash
cp -r ~/.cache/yay/nsis /tmp/nsis-x86 && cd /tmp/nsis-x86
sed -i 's|TARGET_ARCH=amd64|TARGET_ARCH=x86|g; \
        s|ZLIB_W32=/usr/x86_64-w64-mingw32/|ZLIB_W32=/usr/i686-w64-mingw32/|g' PKGBUILD
sed -i 's|^pkgrel=2$|pkgrel=2.1|' PKGBUILD   # yerel derleme işareti
LC_ALL=C.UTF-8 makepkg -f --noconfirm
sudo pacman -U nsis-3.13-2.1-x86_64.pkg.tar.zst
```

Gereken araçlar zaten `mingw-w64-gcc` ve `mingw-w64-zlib` ile gelir
(`i686-w64-mingw32-gcc`, `/usr/i686-w64-mingw32/lib/libz.a`).

Doğrulama — `x86-unicode` görünmeli:

```bash
ls /usr/share/nsis/Plugins/     # x86-ansi  x86-unicode
```

Kurulum sonrası `/etc/pacman.conf` içindeki **`[options]` bölümüne**
şu satır eklenmelidir, yoksa ilk `nsis` güncellemesi bu derlemeyi ezer ve
paketleme sessizce kırılır:

```
IgnorePkg   = nsis
```

Satırı dosyanın sonuna eklemek işe yaramaz; `[multilib]` bölümünün içine
düşer ve pacman "geçersiz direktif" uyarısı verip yok sayar.

Üretilen `*-setup.exe` dosyası `PE32 ... Intel i386` olarak görünür; bu
beklenen davranıştır. NSIS kurulumları her zaman 32 bittir, kurdukları
uygulamanın 64 bit olması bunu değiştirmez.

### Linux'ta arayüz geliştirme

Arayüzü Linux'ta çalıştırıp iterasyon yapabilirsiniz (ekran yakalama
çalışmaz, bu beklenen davranıştır):

```bash
sudo pacman -S webkit2gtk-4.1 gtk3 libayatana-appindicator librsvg patchelf
npm run dev:teacher
```

## Komutlar

```bash
npm install                 # bağımlılıklar
cargo test --workspace      # Rust birim testleri
npm run typecheck           # TypeScript denetimi
npm run lint:rust           # clippy, uyarılar hata sayılır
cargo fmt --all             # biçimlendirme

npm run dev:teacher         # öğretmen konsolu, canlı yeniden yükleme
npm run dev:student         # öğrenci ajanı
```

### Windows çıktıları (Linux'tan)

```bash
cd apps/teacher
cargo tauri build --runner cargo-xwin --target x86_64-pc-windows-msvc --bundles nsis

cd ../student
cargo tauri build --runner cargo-xwin --target x86_64-pc-windows-msvc --bundles nsis
```

Çıktılar:

```
target/x86_64-pc-windows-msvc/release/OpenClass-Teacher.exe
target/x86_64-pc-windows-msvc/release/OpenClass-Student.exe
target/x86_64-pc-windows-msvc/release/bundle/nsis/*-setup.exe
```

Yalnızca `.exe` isterseniz `--no-bundle` ekleyin.

> Cross-compile sırasında `lld-link: Cannot use debug info ... LNK4099`
> uyarıları normaldir: Microsoft'un CRT kütüphanelerinin `.pdb` dosyaları
> xwin paketinde bulunmaz. Çalışmaya etkisi yoktur.

## Dizin yapısı

```
crates/protocol/    İki uygulamanın paylaştığı ağ sözleşmesi
crates/common/      Yollar, loglama, TLS kimliği, DPAPI gizli saklama
apps/teacher/
  src/              React arayüz
  src-tauri/src/
    net/server.rs   TLS + WebSocket sunucusu, yetkilendirme
    net/discovery.rs UDP probe gönderici
    db.rs           SQLite: cihazlar, oturumlar, denetim
    state.rs        Çalışma anı kayıt defteri
    commands.rs     Arayüzün çağırdığı komutlar
apps/student/
  src/              React arayüz
  src-tauri/src/
    net/client.rs   Bağlantı döngüsü, eşleştirme, ekran yayını
    net/announce.rs UDP keşif cevaplayıcısı
    net/pinned_tls.rs Parmak izi doğrulaması
    capture/        Ekran yakalama (WGC + DXGI)
    encode.rs       Ölçekleme + JPEG (platformdan bağımsız, testli)
```

## Hata ayıklama

Loglar:

```
Windows: %APPDATA%\OpenClass\OpenClass\teacher\logs\
         %APPDATA%\OpenClass\OpenClass\student\logs\
Linux:   ~/.local/share/openclass/teacher/logs/
```

Ayrıntılı log:

```powershell
$env:OPENCLASS_LOG="debug"; .\OpenClass-Teacher.exe
```

## Windows hedefini denetleme

Windows'a özgü kod (DPAPI, ekran yakalama, `SendInput`, `BlockInput`, kayıt
defteri) Linux'ta derlenmez; `cargo clippy` bu yolları hiç görmez.
`cargo check` ise derler ama **lint denetlemez**. CI'daki Windows işi clippy
koştuğu için yerelde temiz görünen bir değişiklik orada patlayabilir.

Push etmeden önce:

```bash
cargo xwin clippy --workspace --all-targets --target x86_64-pc-windows-msvc -- -D warnings
```

`./scripts/build-windows.sh` bunu kendiliğinden çalıştırır (`--hizli` ile
atlanabilir).

### Windows testlerini Linux'ta koşturma

Testlerin bir kısmı yalnızca Windows'ta farklı davranır (DPAPI, yol
biçimleri). Wine ile yerelde koşturulabilir:

```bash
WINEDEBUG=-all CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUNNER=wine \
  cargo xwin test --workspace --target x86_64-pc-windows-msvc
```

Arch'ta `sudo pacman -S wine`. Bu, CI'nın Windows işinde patlamayı
beklemekten çok daha hızlıdır.

## Sürüm paketlerini üretme

Kurulum dosyaları **yerelde** derlenir ve sürüm sayfasına elle yüklenir.
CI yalnızca doğrulama yapar (lint, test, derleme kontrolü); paket üretmez.
Böylece yayımlanan ikilinin tam olarak hangi ağaçtan çıktığı elde kalır.

NSIS gerekir (Arch/CachyOS):

```bash
yay -S nsis
```

Sonra:

```bash
./scripts/build-windows.sh
```

Çıktılar:

```
target/x86_64-pc-windows-msvc/release/OpenClass-Teacher.exe
target/x86_64-pc-windows-msvc/release/OpenClass-Student.exe
target/x86_64-pc-windows-msvc/release/bundle/nsis/*-setup.exe
```

Kurulum paketleri WebView2 çalışma zamanını içinde taşır; ilk derlemede
Microsoft'tan indirilir (~127 MB) ve önbelleğe alınır.
