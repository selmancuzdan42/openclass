#!/usr/bin/env bash
# Linux'tan Windows x64 çıktılarını üretir.
#
# Kullanım:
#   ./scripts/build-windows.sh              # .exe + NSIS kurulum paketi
#   ./scripts/build-windows.sh --no-bundle  # yalnızca .exe
#   ./scripts/build-windows.sh --hizli      # lint denetimini atla
#
# Derlemeden önce Windows hedefinde clippy koşar. `cargo check` Windows'a
# özgü kod yollarını derler ama **lint denetlemez**; CI'daki Windows işi
# clippy koştuğu için yerelde temiz görünen bir değişiklik orada patlayabilir.

set -euo pipefail
cd "$(dirname "$0")/.."

BUNDLE_ARGS=("--bundles" "nsis")
if [ "${1:-}" = "--no-bundle" ]; then
  BUNDLE_ARGS=("--no-bundle")
  echo "==> Yalnızca .exe üretilecek"
elif ! command -v makensis >/dev/null 2>&1; then
  echo "UYARI: makensis bulunamadı — kurulum paketi üretilemeyecek."
  echo "       Arch: yay -S nsis     Debian/Ubuntu: sudo apt install nsis"
  echo "       Yalnızca .exe üretiliyor."
  BUNDLE_ARGS=("--no-bundle")
fi

TARGET=x86_64-pc-windows-msvc

# NSIS'i Türkçe olmayan bir UTF-8 yereliyle çalıştır.
#
# İki ayrı tuzak var:
#
# 1. makensis komut anahtarlarını (`/IfNDef` gibi) harf duyarsız
#    karşılaştırır ve bunu yerele bağlı küçültmeyle yapar. Türkçe yerelde
#    `I` noktasız `ı`ya dönüşür, `IfNDef` → `ıfndef` olur ve hiçbir
#    anahtarla eşleşmez; NSIS'in kendi `MUI2.nsh` dosyası ilk satırlarında
#    sözdizimi hatası verir.
#
# 2. Düz `LC_ALL=C` bunu çözer ama bu kez ASCII dışı yolları (`Masaüstü`
#    gibi) çeviremez: "main argv conversion failed".
#
# `C.UTF-8` ikisini birden karşılar: İngilizce harf kuralları + UTF-8 yol
# desteği. İngilizce yerelde çalışan makinelerde (CI) sorun hiç görünmez,
# bu yüzden teşhisi zordur.
export LC_ALL=C.UTF-8

if [ "${1:-}" != "--hizli" ]; then
  echo "==> Windows hedefinde clippy"
  cargo xwin clippy --workspace --all-targets --target "$TARGET" -- -D warnings
fi

for app in teacher student; do
  echo "==> OpenClass ${app} derleniyor"
  ( cd "apps/$app" && cargo tauri build --runner cargo-xwin --target "$TARGET" "${BUNDLE_ARGS[@]}" )
done

echo
echo "==> Çıktılar"
ls -la "target/$TARGET/release/"*.exe 2>/dev/null || true
ls -la "target/$TARGET/release/bundle/nsis/"*.exe 2>/dev/null || true
