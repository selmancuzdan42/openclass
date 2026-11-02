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
}

export type Authorization = "authorized" | "pending" | "rejected" | "unknown";

export interface PairingInfo {
  code: string;
  remainingSecs: number;
  maxAttempts: number;
  failedAttempts: number;
}

export interface ClassSession {
  sessionId: string;
  className: string;
  startedAt: number;
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
}

/** Öğretmenin tanımladığı laboratuvar grubu. */
export interface DeviceGroup {
  name: string;
  /** Eşleşme için kullanılan bilgisayar adı ön eki (büyük harf). */
  prefix: string;
}

/** Bilgisayar adlarından çıkarılan grup önerisi. */
export interface GroupSuggestion {
  prefix: string;
  deviceCount: number;
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
