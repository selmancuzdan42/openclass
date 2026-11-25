/** Rust tarafındaki `state::DeviceView` karşılığı. */
export interface DeviceView {
  deviceId: string;
  hostname: string;
  displayName: string | null;
  /** `authorized` | `pending` | `rejected` | `unknown` */
  authorization: Authorization;
  online: boolean;
  discovered: boolean;
  address: string | null;
  lastSeen: number;
  streaming: boolean;
  helpRequested: boolean;
  frameSeq: number;
  frameWidth: number;
  frameHeight: number;
  frameAgeMs: number;
  connectedAt: number;
  screenError: string | null;
  /** Öğrencinin bildirdiği laboratuvar/oda adı; bilinmiyorsa `null`. */
  room: string | null;
  /** USB depolama kısıtlı mı? (öğrencinin bildirdiği durum) */
  usbBlocked: boolean;
}

export type Authorization = "authorized" | "pending" | "rejected" | "unknown";

/**
 * Dosya gönderiminde hedef kök. Rust tarafındaki
 * `control::FileDestination` karşılığı (`rename_all = "snake_case"`).
 *
 * Serbest yol taşınmaz: öğrenci tarafında her değer kullanıcının kendi
 * klasörüne çözülür ve altına `OpenClass` eklenir.
 */
export type FileDestination = "documents" | "desktop" | "downloads";

export interface PairingInfo {
  code: string;
  remainingSecs: number;
  maxAttempts: number;
  failedAttempts: number;
}

export interface ServerInfo {
  teacherId: string;
  teacherName: string;
  className: string;
  controlPort: number;
  discoveryPort: number;
  certFingerprint: string;
  localAddresses: string[];
  onlineCount: number;
  extraDiscoveryHosts: string[];
  setupCompleted: boolean;
  /** Konsolun yönettiği laboratuvar; `null` ise bütün cihazlar listelenir. */
  activeRoom: string | null;
  /** Konsolun süzdüğü bilgisayar adı ön eki; `null` ise süzme yok. */
  activePrefix: string | null;
  /** Açılışta sınıf seçim penceresi gösterilsin mi? */
  askAtStartup: boolean;
  /** Bu (öğretmen) bilgisayarının adı. */
  hostname: string;
  /** Bilgisayar adından çıkarılan ön ek önerisi (`LAB1-OGR` → `LAB1`). */
  suggestedPrefix: string | null;
  /** Ağ servisleri başlatılamadıysa sebebi. */
  networkError: string | null;
}

export interface AuditEntry {
  at: number;
  kind: string;
  deviceId: string | null;
  detail: string | null;
}

export interface FrameEvent {
  deviceId: string;
  seq: number;
  width: number;
  height: number;
}

export interface HelpEvent {
  deviceId: string;
  hostname: string;
  note: string | null;
}

/** Öğrenci klavyesinden yakalanan bir tuş olayı (klavye izleme). */
export interface KeyEvent {
  deviceId: string;
  /** Windows sanal tuş kodu (VK_*). */
  code: number;
  /** Basılma mı bırakma mı. */
  pressed: boolean;
  /** Olay zaman damgası (ms). */
  ts: number;
}

export type Quality = "preview" | "focused";

/** Öğrenci makinesine iletilecek girdi olayı. */
export type InputEvent =
  | { t: "mouse_move"; x: number; y: number }
  | { t: "mouse_button"; button: "left" | "right" | "middle"; pressed: boolean }
  | { t: "mouse_wheel"; dx: number; dy: number }
  | { t: "key"; code: number; pressed: boolean };

/** Çalışan öğretmen ekranı yayını. */
export interface BroadcastInfo {
  broadcastId: string;
  startedAt: number;
  /** Penceresiz (tam ekran) mod mu? */
  fullscreen: boolean;
  /** Kareler UDP multicast ile mi gidiyor? */
  multicast: boolean;
  /** Multicast ulaşmadığı için TLS yedeğine düşen cihaz sayısı. */
  fallbackCount: number;
  /** Yayın bir öğrenci ekranının yansıtmasıysa o cihaz; değilse `null`. */
  sourceDeviceId: string | null;
  /** Yansıtılan öğrencinin bilgisayar adı. */
  sourceName: string | null;
}

/** Öğretmenin tanımladığı kayıtlı sınıf. */
export interface DeviceGroup {
  name: string;
  /**
   * Eşleşme için kullanılan bilgisayar adı ön eki (büyük harf). Boşsa grup
   * yalnızca oda adına göre eşleşir (grup adı = oda adı).
   */
  prefix: string;
}

/** Bir dosya gönderiminde tek alıcının durumu. */
export type TargetState =
  | { state: "offered" }
  | { state: "sending"; sent: number }
  | { state: "rejected"; reason: string }
  | { state: "done"; path: string }
  | { state: "failed"; reason: string };

/** Yürüyen veya bitmiş bir dosya gönderimi. */
export interface TransferView {
  transferId: string;
  name: string;
  size: number;
  /** Cihaz kimliği → durum. */
  targets: Record<string, TargetState>;
  finished: boolean;
}

/** Uzak dosya gezgininde tek bir girdi. */
export interface BrowseEntry {
  name: string;
  isDir: boolean;
  size: number;
  modified: number;
}

/** Öğrencide gezilmesine izin verilen bir kök. */
export interface BrowseRoot {
  label: string;
  path: string;
}

/** Yürüyen veya bitmiş bir indirme. */
export interface BrowseDownload {
  name: string;
  size: number;
  received: number;
  finished: boolean;
  ok: boolean;
  detail: string;
}

/** Açık gezgin oturumunun durumu. */
export interface BrowseView {
  deviceId: string;
  hostname: string;
  path: string;
  roots: BrowseRoot[];
  entries: BrowseEntry[];
  loading: boolean;
  error: string | null;
  download: BrowseDownload | null;
}
