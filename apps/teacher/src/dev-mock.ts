/**
 * Geliştirme amaçlı sahte arka uç.
 *
 * Yalnızca uygulama Tauri dışında, düz bir tarayıcıda açıldığında devreye
 * girer (`npm run dev` → http://127.0.0.1:1420). Amacı, Rust tarafını yeniden
 * derlemeden arayüz üzerinde çalışabilmektir.
 *
 * Üretim derlemesine **dahil edilmez**: `main.tsx` bu modülü yalnızca
 * `import.meta.env.DEV` iken ve Tauri köprüsü yokken yükler.
 */
import type { AuditEntry, DeviceView, ServerInfo } from "./types";

const HOSTS = [
  "LAB101", "LAB102", "LAB103", "LAB104", "LAB105", "LAB106",
  "LAB107", "LAB108", "LAB109", "LAB110", "LAB111", "LAB112",
  "LAB201", "LAB202", "LAB203", "LAB204",
];

function makeDevice(hostname: string, i: number): DeviceView {
  const online = i % 5 !== 3;
  const streaming = online && i % 4 !== 2;
  return {
    deviceId: `00000000-0000-4000-8000-${String(i).padStart(12, "0")}`,
    hostname,
    displayName: i === 2 ? "1. sıra sol" : null,
    authorization: i === 11 ? "pending" : i === 14 ? "unknown" : "authorized",
    online,
    discovered: true,
    address: `192.168.1.${20 + i}:7643`,
    lastSeen: Math.floor(Date.now() / 1000) - i * 7,
    streaming,
    helpRequested: i === 4 || i === 9,
    frameSeq: streaming ? 100 + i : 0,
    frameWidth: 480,
    frameHeight: 270,
    frameAgeMs: i === 7 ? 14_000 : 400,
    connectedAt: Math.floor(Date.now() / 1000) - 900,
    screenError: i === 6 ? "Ekran yakalama başlatılamadı: sanal ekran sürücüsü desteklemiyor." : null,
  };
}

const devices = HOSTS.map(makeDevice);

let session: { sessionId: string; className: string; startedAt: number } | null = {
  sessionId: "11111111-1111-4111-8111-111111111111",
  className: "Lab 1 — 11-A Bilişim",
  startedAt: Math.floor(Date.now() / 1000) - 1_450,
};

let groups = [
  { name: "Lab 1", prefix: "LAB1" },
  { name: "Lab 2", prefix: "LAB2" },
];

const serverInfo: ServerInfo = {
  teacherId: "22222222-2222-4222-8222-222222222222",
  teacherName: "Cihan Güngör",
  className: "Lab 1 — 11-A Bilişim",
  controlPort: 7643,
  discoveryPort: 7644,
  certFingerprint: "11:05:F0:6B:9F:B2:A9:46:96:F3:CB:BF:11:32:AE:22:1A:AE:10:93:45:AF:65:F3:67:B2:F8:BC:CB:11:C9:56",
  localAddresses: ["192.168.1.18"],
  onlineCount: devices.filter((d) => d.online).length,
  extraDiscoveryHosts: [],
  setupCompleted: true,
  networkError: null,
};

const audit: AuditEntry[] = devices.slice(0, 12).map((d, i) => ({
  at: Math.floor(Date.now() / 1000) - i * 95,
  kind: ["device_connected", "screen_stream_started", "help_requested", "notice_sent"][i % 4],
  deviceId: d.deviceId,
  detail: d.hostname,
}));

const HANDLERS: Record<string, (args: Record<string, unknown>) => unknown> = {
  // Tauri olay eklentisi: sahte arka uç olay yaymaz, yalnızca kayıt numarası
  // döndürür ki `listen()` çağrıları sessizce başarılı olsun.
  "plugin:event|listen": () => Math.floor(Math.random() * 1e9),
  "plugin:event|unlisten": () => null,
  "plugin:event|emit": () => null,
  "plugin:event|emit_to": () => null,

  search_devices: () => devices,
  get_server_info: () => serverInfo,
  get_session: () => session,
  frame_base_url: () => "/mock-frame",
  list_groups: () => groups,
  get_pairing: () => null,
  get_broadcast: () => null,
  start_broadcast: () => ({ broadcastId: "x", startedAt: 0 }),
  stop_broadcast: () => null,
  open_browser: () => null,
  close_browser: () => null,
  browse_list: () => null,
  browse_fetch: () => null,
  get_browse: () => null,
  open_downloads_dir: () => null,
  send_file: () => "00000000-0000-0000-0000-000000000000",
  get_transfers: () => [],
  clear_finished_transfers: () => null,
  lock_devices: () => 1,
  unlock_devices: () => 1,
  start_remote_control: () => null,
  stop_remote_control: () => null,
  send_remote_input: () => null,
  get_audit_log: () => audit,
  suggest_groups: () => [
    { prefix: "LAB1", deviceCount: 12 },
    { prefix: "LAB2", deviceCount: 4 },
  ],
  start_pairing: () => ({
    code: "428913",
    remainingSecs: 300,
    maxAttempts: 5,
    failedAttempts: 0,
  }),
  cancel_pairing: () => null,
  start_session: () => {
    session = {
      sessionId: "33333333-3333-4333-8333-333333333333",
      className: serverInfo.className,
      startedAt: Math.floor(Date.now() / 1000),
    };
    return session;
  },
  end_session: () => {
    session = null;
    return null;
  },
  start_screen_streams: (args) => (args.deviceIds as string[]).length,
  stop_screen_streams: () => null,
  focus_device: () => null,
  unfocus_devices: () => null,
  send_notice: (args) => (args.deviceIds as string[]).length,
  clear_help_request: () => null,
  revoke_device: () => null,
  set_display_name: () => null,
  update_settings: () => null,
  save_group: (args) => {
    groups = [...groups, { name: args.name as string, prefix: args.prefix as string }];
    return null;
  },
  delete_group: (args) => {
    groups = groups.filter((g) => g.name !== args.name);
    return null;
  },
};

/** Tauri köprüsünün yerine geçen en küçük sahte uygulama. */
export function installDevMock() {
  const w = window as unknown as Record<string, unknown>;
  // `listen()` dönüşündeki unlisten işlevi bu nesneyi kullanır.
  w.__TAURI_EVENT_PLUGIN_INTERNALS__ = {
    unregisterListener: () => {},
  };
  w.__TAURI_INTERNALS__ = {
    invoke: async (cmd: string, args: Record<string, unknown> = {}) => {
      const handler = HANDLERS[cmd];
      if (!handler) throw `Sahte arka uçta tanımsız komut: ${cmd}`;
      return handler(args);
    },
    // Olay köprüsü: sahte arka uç olay yaymaz, dinleyiciler boşa çalışır.
    transformCallback: (cb: unknown) => cb,
  };
  // eslint-disable-next-line no-console
  console.info("[OpenClass] Sahte geliştirme arka ucu etkin — Tauri köprüsü yok.");
}
