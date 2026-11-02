import { ToastProvider } from "./components";
import { BroadcastWindow } from "./BroadcastWindow";
import { ActivityIndicator } from "./ActivityIndicator";
import { LockScreen } from "./LockScreen";
import { ConfigWindow } from "./ConfigWindow";
import { NoticeWindow } from "./NoticeWindow";

/**
 * Öğrenci ajanının iki penceresi vardır:
 *
 * * **Yapılandırma** (varsayılan) — öğretmen/yönetici içindir. Eşleştirme ve
 *   bağlantı ayarları buradan yapılır. Normal kullanımda kapalıdır.
 * * **Mesaj** (`?view=notice`) — öğretmen mesaj gönderdiğinde Rust tarafı
 *   açar, öğrenci okuyup kapatır.
 * * **Öğretmen ekranı** (`?view=broadcast`) — yayın başladığında tam ekran
 *   açılır, bittiğinde Rust tarafı kapatır.
 * * **Etkinlik göstergesi** (`?view=activity`) — uzaktan kontrol veya
 *   dosya gezgini açıkken üstte duran, kapatılamayan şerit.
 * * **Kilit ekranı** (`?view=lock`) — klavye/fare kilitliyken nedenini
 *   açıklayan tam ekran pencere.
 *
 * Öğrencinin sürekli gördüğü yüzey pencere değil, **sistem tepsisi
 * simgesidir**. Durum ve yardım talebi oradadır.
 */
export default function App() {
  const view = new URLSearchParams(window.location.search).get("view");

  if (view === "broadcast") {
    return <BroadcastWindow />;
  }

  if (view === "activity") {
    return <ActivityIndicator />;
  }

  if (view === "lock") {
    return <LockScreen />;
  }

  return (
    <ToastProvider>{view === "notice" ? <NoticeWindow /> : <ConfigWindow />}</ToastProvider>
  );
}
