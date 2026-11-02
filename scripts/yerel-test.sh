#!/usr/bin/env bash
# OpenClass — tek makinede yerel test ortamı.
#
# Bir öğretmen konsolu ve istenen sayıda sahte öğrenci ajanını aynı Linux
# makinesinde çalıştırır. Öğrenciler TEST DESENİ yayınlar (gerçek ekran
# yakalama yalnızca Windows'ta vardır), böylece kare hattının tamamı burada
# sınanabilir: keşif, eşleştirme, TLS, kimlik doğrulama, arama/filtreleme,
# ölçekleme + JPEG kodlama, ağ aktarımı, konsolda görüntüleme, kalite
# değişimi, mesajlaşma, yardım talebi, ders oturumu, yeniden bağlanma.
#
# Kullanım:
#   ./scripts/yerel-test.sh                      # LAB101 + LAB102 (hızlı/debug)
#   ./scripts/yerel-test.sh LAB101 LAB102 LAB201
#   ./scripts/yerel-test.sh --ogretmen           # YALNIZCA öğretmen konsolu
#                                                # (öğrenciler gerçek Windows
#                                                #  makinelerde çalışacaksa)
#   ./scripts/yerel-test.sh --perf LAB101 ...    # optimize derleme (başarım ölçümü)
#   ./scripts/yerel-test.sh --release LAB101     # sürüm derlemesi (test modu kapalı)
#   ./scripts/yerel-test.sh --manual LAB101      # elle eşleştirme (kod girerek)
#   ./scripts/yerel-test.sh --sifirla ...        # öğretmen kimliğini de sıfırla
#
# Varsayılanda her şey kendiliğinden kurulur: öğrenciler sabit kodla eşleşir,
# ders başlar ve ekran yayınları açılır. Yayınlanan görüntü **test desenidir**,
# gerçek ekran değildir (Linux'ta gerçek yakalama yoktur).
#
# Durdurmak için Ctrl+C.

set -euo pipefail

cd "$(dirname "$0")/.."
ROOT="$PWD"
# Test verisi **kalıcı** bir dizinde durur.
#
# Önceden `/tmp` altındaydı; çoğu dağıtımda (bu makinede de) `/tmp` bir
# tmpfs, yani RAM diski. Her yeniden başlatmada öğretmenin TLS sertifikası
# ve cihaz veritabanı siliniyordu; öğrenci sabitlediği parmak izini
# bulamadığı ve öğretmen cihazı tanımadığı için **her açılışta yeniden
# eşleştirme** gerekiyordu. Kasıtlı sıfırlama için `--sifirla` var.
RUNTIME="${OPENCLASS_TEST_DIR:-${XDG_DATA_HOME:-$HOME/.local/share}/openclass-yerel-test}"

MODE=debug
AUTO=1
TEACHER_ONLY=0
RESET=0
ARGS=()
for arg in "$@"; do
  case "$arg" in
    --release) MODE=release ;;
    --debug)   MODE=debug ;;
    --perf)    MODE=testlab ;;
    --manual)  AUTO=0 ;;
    --ogretmen|--teacher) TEACHER_ONLY=1; AUTO=0 ;;
    --sifirla|--reset) RESET=1 ;;
    -*) echo "Bilinmeyen seçenek: $arg" >&2; exit 1 ;;
    *) ARGS+=("$arg") ;;
  esac
done

HOSTS=("${ARGS[@]}")
if [ "$TEACHER_ONLY" = 1 ]; then
  HOSTS=()
elif [ ${#HOSTS[@]} -eq 0 ]; then
  HOSTS=(LAB101 LAB102)
fi

# ---------------------------------------------------------------------------
# Bellek koruması
#
# Her uygulama örneği kendi WebView sürecini taşır; ölçülen ortalama yerleşik
# bellek ~250 MB'dir. Gerçek laboratuvarda her öğrenci ayrı makinededir, ama
# burada hepsi aynı makinede koşar. Yetersiz bellekle başlatmak makineyi
# kilitler (OOM), bu yüzden önceden kontrol edilir.
# ---------------------------------------------------------------------------
# Ölçülen yerleşik bellek örnek başına ~250 MB; ancak WebView'ın geçici
# ayırmaları ve GPU tamponlarıyla tepe kullanım bunun epey üstüne çıkar.
# 10 örnek, 9 GB boş bellekli bir makinede sistemi kilitledi; bu yüzden
# hem cömert bir pay bırakılır hem de sert bir üst sınır uygulanır.
PER_INSTANCE_MB=450
MAX_INSTANCES=${OPENCLASS_MAX_INSTANCES:-6}
NEEDED_MB=$(( (${#HOSTS[@]} + 1) * PER_INSTANCE_MB ))
AVAILABLE_MB=$(awk '/MemAvailable/{print int($2/1024)}' /proc/meminfo)

refuse() {
  echo "HATA: $1" >&2
  echo "      Her uygulama örneği kendi WebView sürecini taşır; bu makinede" >&2
  echo "      hepsi aynı anda koşuyor. Gerçek laboratuvarda her öğrenci ayrı" >&2
  echo "      makinededir, bu sınır orada geçerli değildir." >&2
  echo "      20 makinelik yük ölçümü laboratuvarda yapılmalıdır" >&2
  echo "      (bkz. docs/TEST-PLANI.md, Ortam B)." >&2
  echo "      Sınırı bilerek aşmak için: OPENCLASS_MAX_INSTANCES=N" >&2
  exit 1
}

if [ "${#HOSTS[@]}" -gt "$MAX_INSTANCES" ]; then
  refuse "${#HOSTS[@]} öğrenci istendi, yerel sınır $MAX_INSTANCES."
fi
if [ "$NEEDED_MB" -gt "$AVAILABLE_MB" ]; then
  refuse "~${NEEDED_MB} MB gerekiyor, kullanılabilir bellek ${AVAILABLE_MB} MB."
fi

# ---------------------------------------------------------------------------
# Zaten çalışan bir örnek var mı?
#
# İkinci bir öğretmen konsolu 7643'ü bağlayamaz ama çalışmaya devam eder:
# UDP ile kendi sertifikasını duyurur, TCP'ye ise eski süreç cevap verir.
# Sonuç, öğrencide anlaşılmaz bir "sertifika farklı" hatasıdır. Bu yüzden
# baştan engellenir.
# ---------------------------------------------------------------------------
if pgrep -f "target/(debug|release|testlab)/openclass-teacher" >/dev/null 2>&1; then
  echo "HATA: Zaten çalışan bir OpenClass Teacher var." >&2
  echo "      Önce onu kapatın (başlattığınız terminalde Ctrl+C)." >&2
  echo >&2
  ps -eo pid,etime,args | grep -E "openclass-(teacher|student)" | grep -v grep >&2
  exit 1
fi

PIDS=()
cleanup() {
  echo
  echo "==> Kapatılıyor"
  for pid in "${PIDS[@]}"; do
    kill "$pid" 2>/dev/null || true
  done
  wait 2>/dev/null || true
}
trap cleanup EXIT INT TERM

# Bir TCP portu dinlemeye başlayana kadar bekler.
wait_for_port() {
  local port="$1" label="$2" tries=0
  while true; do
    if (exec 3<>"/dev/tcp/127.0.0.1/$port") 2>/dev/null; then
      exec 3<&- 3>&-; return 0
    fi
    if (exec 3<>"/dev/tcp/::1/$port") 2>/dev/null; then
      exec 3<&- 3>&-; return 0
    fi
    tries=$((tries + 1))
    if [ "$tries" -gt 120 ]; then
      echo "HATA: $label ($port) 60 saniyede açılmadı." >&2
      echo "      Log: $RUNTIME/ogretmen.log" >&2
      exit 1
    fi
    sleep 0.5
  done
}

# ---------------------------------------------------------------------------
# Derleme
#
# Arayüz ikiliye GÖMÜLÜ derlenir (`--features custom-protocol`). Böylece test
# bir geliştirme sunucusunun ayakta kalmasına bağlı olmaz; Vite kapanırsa
# pencerelerin "Connection refused" göstermesi diye bir durum oluşmaz.
#
# Hata ayıklama derlemesi seçilir çünkü test kolaylıkları (sabit eşleştirme
# kodu, kendiliğinden başlatma) yalnızca `debug_assertions` ile çalışır.
# ---------------------------------------------------------------------------

echo "==> Arayüzler derleniyor"
npm run build --workspace @openclass/teacher --silent
if [ "$TEACHER_ONLY" != 1 ]; then
  npm run build --workspace @openclass/student --silent
fi

case "$MODE" in
  release)
    echo "==> Rust ikilileri derleniyor (release — birkaç dakika sürer)"
    cargo build --release --features custom-protocol -p openclass-teacher -p openclass-student
    BIN_DIR="$ROOT/target/release"
    ;;
  testlab)
    echo "==> Rust ikilileri derleniyor (testlab: optimize + test kolaylıkları)"
    cargo build --profile testlab --features custom-protocol -p openclass-teacher -p openclass-student
    BIN_DIR="$ROOT/target/testlab"
    ;;
  *)
    echo "==> Rust ikilileri derleniyor"
    if [ "$TEACHER_ONLY" = 1 ]; then
      cargo build --features custom-protocol -p openclass-teacher
    else
      cargo build --features custom-protocol -p openclass-teacher -p openclass-student
    fi
    BIN_DIR="$ROOT/target/debug"
    ;;
esac

TEACHER_BIN="$BIN_DIR/openclass-teacher"
STUDENT_BIN="$BIN_DIR/openclass-student"

# ---------------------------------------------------------------------------
# Çalışma dizinleri
# ---------------------------------------------------------------------------

# ---------------------------------------------------------------------------
# Çalışma dizini
#
# Öğretmenin TLS sertifikası ve kimliği bu dizinde durur. Öğrenciler
# eşleştirme anında sertifikanın parmak izini **sabitler**; dizin silinirse
# öğretmen yeni bir sertifika üretir ve daha önce eşleşmiş her öğrenci
# bağlantıyı güvenlik gerekçesiyle reddeder.
#
# Bu yüzden öğretmen verisi varsayılan olarak KORUNUR. Sahte öğrenciler ise
# her koşuda sıfırdan başlar (tekrarlanabilir eşleştirme testi için).
# Her şeyi sıfırlamak için: --sifirla
# ---------------------------------------------------------------------------
if [ "$RESET" = 1 ]; then
  echo "==> Öğretmen kimliği sıfırlanıyor (eşleşmiş tüm cihazlar yeniden eşleşmeli)"
  rm -rf "$RUNTIME"
else
  # Sahte öğrencilerin verisi her koşuda tazelenir; öğretmeninki kalır.
  for host in "${HOSTS[@]}"; do
    rm -rf "${RUNTIME:?}/$host"
  done
fi
mkdir -p "$RUNTIME/ogretmen/teacher"

# Öğrencilerin keşif portlarını önceden hesapla ve öğretmen yapılandırmasına
# yaz. Aynı makinede 255.255.255.255 yayını geri dönmediği için öğretmenin
# öğrencileri 127.0.0.1 üzerinden unicast probe ile bulması gerekir.
# (Gerçek laboratuvarda bu gerekmez; yayın yeterlidir.)
PORT=7644
TARGETS=""
for _ in "${HOSTS[@]}"; do
  [ -n "$TARGETS" ] && TARGETS="$TARGETS, "
  TARGETS="$TARGETS\"127.0.0.1:$PORT\""
  PORT=$((PORT + 1))
done

# Yapılandırma yoksa oluştur. Varsa dokunma: içinde öğretmen kimliği
# (teacherId) durur ve onu değiştirmek eşleşmeleri bozar.
if [ ! -f "$RUNTIME/ogretmen/teacher/config.json" ]; then
  cat > "$RUNTIME/ogretmen/teacher/config.json" <<JSON
{
  "teacherName": "Test Öğretmen",
  "className": "Lab 1",
  "extraDiscoveryHosts": [$TARGETS]
}
JSON
fi

# ---------------------------------------------------------------------------
# Uygulamalar
# ---------------------------------------------------------------------------

# Test kolaylıkları yalnızca hata ayıklama derlemesinde etkindir; sürüm
# derlemesi bu değişkenleri tamamen yok sayar.
TEST_CODE=424242
if [ "$AUTO" = 1 ]; then
  if [ "$MODE" = release ]; then
    echo "NOT: --release ile otomatik eşleştirme/başlatma çalışmaz (sürüm"
    echo "     derlemesi test değişkenlerini yok sayar). Elle eşleştirin."
    AUTO=0
  fi
fi

echo "==> Öğretmen konsolu başlatılıyor"
TEACHER_ENV=(
  "OPENCLASS_DATA_DIR=$RUNTIME/ogretmen"
  "OPENCLASS_LOG=${OPENCLASS_LOG:-info}"
)
# Öğretmenin "Ekranımı Yayınla" işlevi gerçek ekran yakalaması ister; bu
# yalnızca Windows'ta vardır. Bu betik Linux'ta koştuğu için test desenine
# geçilir, yoksa yayın "öğretmen ekranı yakalanamadı" diye başarısız olur.
# Arayüz bunu açıkça "test deseni" olarak etiketler; gerçek ekran gibi
# sunulmaz. OPENCLASS_CAPTURE önceden verilmişse ona dokunulmaz.
TEACHER_ENV+=("OPENCLASS_CAPTURE=${OPENCLASS_CAPTURE:-test}")
if [ "$AUTO" = 1 ]; then
  TEACHER_ENV+=("OPENCLASS_TEST_PAIRING_CODE=$TEST_CODE" "OPENCLASS_TEST_AUTOSTART=1")
fi
env "${TEACHER_ENV[@]}" "$TEACHER_BIN" >"$RUNTIME/ogretmen.log" 2>&1 &
PIDS+=($!)

wait_for_port 7643 "öğretmen bağlantı sunucusu"

if [ "$TEACHER_ONLY" = 1 ]; then
  IPS=$(ip -4 -br addr 2>/dev/null | awk '$1!="lo" && $2=="UP" {sub(/\/.*/,"",$3); print $3}')
  cat <<TEACHERINFO

════════════════════════════════════════════════════════════════
  Öğretmen konsolu çalışıyor
════════════════════════════════════════════════════════════════

  Bu makinenin adresi:
$(printf '    %s\n' $IPS)

  Şimdi Windows tarafında:

   1. Şu dosyayı Windows bilgisayara kopyala:
      $ROOT/target/x86_64-pc-windows-msvc/release/OpenClass-Student.exe

   2. Windows'ta çift tıkla. Güvenlik duvarı sorarsa
      "Özel ağlarda izin ver" de.

   3. Açılan yapılandırma penceresinde öğretmeni listeden seç.
      Listede yoksa "adres" kutusuna yaz:
$(printf '         %s:7643\n' $IPS)

   4. Buradaki konsolda: Cihaz Eşleştir → çıkan 6 haneli kodu
      Windows'taki pencereye gir → Eşleştir ve Bağlan

   5. Buradaki konsolda: Dersi Başlat → Ekranları Göster

  Windows makinesinin GERÇEK ekranı burada görünmeli.

  Log: $RUNTIME/ogretmen.log
  Durdurmak için: Ctrl+C
════════════════════════════════════════════════════════════════

TEACHERINFO
  wait
fi

PORT=7644
for host in "${HOSTS[@]}"; do
  echo "==> Öğrenci başlatılıyor: $host (keşif portu $PORT)"
  STUDENT_ENV=(
    "OPENCLASS_DATA_DIR=$RUNTIME/$host"
    "OPENCLASS_HOSTNAME=$host"
    "OPENCLASS_DISCOVERY_PORT=$PORT"
    "OPENCLASS_LOG=${OPENCLASS_LOG:-info}"
    "OPENCLASS_CAPTURE=test"
  )
  if [ "$AUTO" = 1 ]; then
    STUDENT_ENV+=("OPENCLASS_TEST_PAIRING_CODE=$TEST_CODE")
  fi
  env "${STUDENT_ENV[@]}" "$STUDENT_BIN" >"$RUNTIME/$host.log" 2>&1 &
  PIDS+=($!)
  PORT=$((PORT + 1))
  sleep 1
done

cat <<INFO

════════════════════════════════════════════════════════════════
  OpenClass yerel test ortamı çalışıyor  ($MODE)
════════════════════════════════════════════════════════════════

  Öğrenciler : ${HOSTS[*]}
  Loglar     : $RUNTIME/

$(if [ "$AUTO" = 1 ]; then cat <<AUTOINFO
  OTOMATİK MOD: öğrenciler sabit kodla ($TEST_CODE) kendiliğinden
  eşleşti, ders açıldı ve ekran yayınları başladı. Birkaç saniye
  içinde konsolda canlı kareler görmelisiniz.

  Görüntüler TEST DESENİDİR, gerçek ekran değildir — Linux'ta
  gerçek yakalama yoktur. Her kartta bilgisayar adı ve kare
  numarası yazar; kartı büyütünce metin okunur hâle gelmeli.

  Deneyin:
   1. Bilgisayarları Ara → "LAB1" yazın (LAB2 dışarıda kalmalı)
   2. Bir karta çift tıklayın (kalite artmalı, metin okunmalı)
   3. Mesaj gönderin → öğrencide açılır pencere gelmeli
   4. Öğrenci tepsi simgesinden Yardım İste'yi seçsin
   5. Dersi Bitir → tüm yayınlar durmalı
AUTOINFO
else cat <<MANUALINFO
  ELLE MOD: eşleştirmeyi kendiniz yapacaksınız.
   1. Cihaz Eşleştir → kodu öğrenci yapılandırma penceresine girin
   2. Her öğrenci için ayrı kod üretin (kod tek kullanımlık)
   3. Dersi Başlat
   4. Bilgisayarları Ara → "LAB1" yazıp arayın
MANUALINFO
fi)

  Canlı log izlemek için başka bir terminalde:
    tail -f $RUNTIME/ogretmen.log

  Durdurmak için: Ctrl+C
════════════════════════════════════════════════════════════════

INFO

wait
