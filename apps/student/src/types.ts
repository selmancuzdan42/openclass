export type ConnectionStatus =
  | { kind: "notPaired" }
  | { kind: "disconnected"; reason: string | null; retryInSecs: number }
  | { kind: "connecting" }
  | { kind: "pairing" }
  | { kind: "connected"; teacherName: string };

export interface SharingState {
  active: boolean;
  backend: string | null;
  fps: number | null;
  width: number | null;
  height: number | null;
  error: string | null;
}

export interface ReceivedNotice {
  messageId: string;
  title: string;
  body: string;
  receivedAt: number;
}

export interface DiscoveredTeacher {
  teacherId: string;
  teacherName: string;
  host: string;
  controlPort: number;
  fingerprintHex: string;
  lastSeen: number;
}

/** Öğretmen uzak dosya gezginini açtı. */
export interface Browsing {
  teacherName: string;
  startedAt: number;
}

/** Öğrenci klavye/faresi kilitli. */
export interface ScreenLock {
  message: string;
  startedAt: number;
}

/** Otomatik başlatmanın durumu. */
export interface AutostartState {
  supported: boolean;
  enabled: boolean;
}

/** Öğretmen uzaktan kontrolü devraldı. */
export interface RemoteControl {
  teacherName: string;
  startedAt: number;
}

/** Çalışan öğretmen ekranı yayını. */
export interface ActiveBroadcast {
  broadcastId: string;
  teacherName: string;
  startedAt: number;
}

export interface StatusSnapshot {
  hostname: string;
  deviceId: string;
  appVersion: string;
  status: ConnectionStatus;
  sharing: SharingState;
  helpRequested: boolean;
  pairedTeacherName: string | null;
  pairedTeacherAddress: string | null;
  manualAddress: string | null;
  notices: ReceivedNotice[];
  captureSupported: boolean;
  /** Bu bilgisayarın bulunduğu laboratuvar/oda adı; girilmemişse `null`. */
  room: string | null;
}
