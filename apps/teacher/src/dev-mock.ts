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
import type {
  AuditEntry,
  CollectionView,
  CollectTarget,
  DeviceView,
  ServerInfo,
} from "./types";

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
    // Oda adını bildirmeyen cihazlar da olmalı: 0.1.0'dan yükselen ajanlar
    // bu alanı göndermez ve arayüz onları "—" ile göstermek zorunda.
    room: i === 13 || i === 14 ? null : hostname.startsWith("LAB1") ? "Lab 1" : "Lab 2",
    usbBlocked: online && (i === 0 || i === 5),
  };
}

const devices = HOSTS.map(makeDevice);

let groups = [
  { name: "Lab 1", prefix: "LAB1" },
  { name: "Lab 2", prefix: "LAB2" },
];

const serverInfo: ServerInfo = {
  // `?surum=0.5.0` ile eski bir sürüm taklit edilir; güncelleme penceresi açılır.
  appVersion: new URLSearchParams(location.search).get("surum") ?? "0.6.1",
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
  activeRoom: null,
  activePrefix: null,
  askAtStartup: true,
  hostname: "LAB1-OGR",
  suggestedPrefix: "LAB1",
  networkError: null,
};

const audit: AuditEntry[] = devices.slice(0, 12).map((d, i) => ({
  at: Math.floor(Date.now() / 1000) - i * 95,
  kind: ["device_connected", "screen_stream_started", "help_requested", "notice_sent"][i % 4],
  deviceId: d.deviceId,
  detail: d.hostname,
}));

const HANDLERS: Record<string, (args: Record<string, unknown>) => unknown> = {
  // Tauri olay eklentisi: kayıt numarası döndürür ve handler'ı saklar ki
  // klavye izleme simülasyonu `key-event` yayabilsin.
  "plugin:event|listen": (args) => {
    const event = args.event as string;
    const handler = args.handler as (e: EventBridge) => void;
    if (handler) {
      if (!listeners.has(event)) listeners.set(event, new Set());
      listeners.get(event)!.add(handler);
    }
    return Math.floor(Math.random() * 1e9);
  },
  "plugin:event|unlisten": () => null,
  "plugin:event|emit": () => null,
  "plugin:event|emit_to": () => null,

  search_devices: () => devices,
  get_server_info: () => serverInfo,
  frame_base_url: () => "/mock-frame",
  list_groups: () => groups,
  get_pairing: () => null,
  get_broadcast: () => null,
  start_broadcast: () => ({ broadcastId: "x", startedAt: 0 }),
  stop_broadcast: () => null,
  show_student_screen: () => ({ broadcastId: "x", startedAt: 0 }),
  restart_device: () => null,
  block_usb: (args) => (args.deviceIds as string[]).length,
  save_screenshot: (args) => `/home/ogretmen/Resimler/OpenClass/${args.hostname}_${args.stamp}.png`,
  open_screenshots_dir: () => null,
  get_crash_reports: () =>
    new URLSearchParams(location.search).has("cokme")
      ? [
          {
            app: "teacher",
            version: "0.5.1",
            os: "windows x86_64",
            message: "called `Option::unwrap()` on a `None` value",
            location: "apps/teacher/src-tauri/src/net/server.rs:812",
            at: Math.floor(Date.now() / 1000) - 3600,
          },
        ]
      : [],
  clear_crash_reports: () => null,
  open_browser: () => {
    browseOpen = true;
    browsePath = "";
    dispatch("browse-changed", mockBrowseView());
    return null;
  },
  close_browser: () => {
    browseOpen = false;
    browsePath = "";
    dispatch("browse-changed", null);
    return null;
  },
  browse_list: (args) => {
    const path = (args.path as string) ?? "";
    browsePath = path;
    // Geçersiz bir yol seçmeceği varsayılıp köke düşülür.
    if (path !== "" && !(path in MOCK_TREE)) browsePath = "";
    dispatch("browse-changed", mockBrowseView());
    return null;
  },
  browse_fetch: () => {
    // Gerçek indirme yapılmaz; sadece durum bozulmasın diye döner.
    return null;
  },
  get_browse: () => mockBrowseView(),
  open_downloads_dir: () => null,
  // Native diyalog yok: çoklu dosya seçiminde örnek dosyalar döner, diğer
  // her şeyde (klasör seçimi) iptal edilmiş gibi davranır.
  "plugin:dialog|open": (args) =>
    (args.options as { multiple?: boolean } | undefined)?.multiple
      ? [
          "C:\\Users\\Hoca\\Belgeler\\sınav.zip",
          "C:\\Users\\Hoca\\Belgeler\\Yönerge.pdf",
          "C:\\Users\\Hoca\\Belgeler\\şablon.docx",
        ]
      : null,
  send_files: (args) => (args.paths as string[]).length,
  collect_files: (args) => (args.deviceIds as string[]).length,
  get_collection: () => mockCollection(),
  open_release_page: () => null,
  shutdown_device: () => null,
  get_download_root: () => "C:\\Users\\Hoca\\Belgeler\\OpenClass\\Toplanan",
  set_download_root: (args: Record<string, unknown>) =>
    (args.path as string | null) ?? "C:\\Users\\Hoca\\Belgeler\\OpenClass\\Toplanan",
  get_transfers: () => [],
  clear_finished_transfers: () => null,
  lock_devices: () => 1,
  unlock_devices: () => 1,
  start_remote_control: () => null,
  stop_remote_control: () => null,
  send_remote_input: () => null,
  start_key_monitor: () => {
    startKeymock();
    return null;
  },
  stop_key_monitor: () => {
    stopKeymock();
    return null;
  },
  get_audit_log: () => audit,
  suggest_groups: () => [
    { prefix: "", deviceCount: 12, room: "Lab 1" },
    { prefix: "", deviceCount: 4, room: "Lab 2" },
    { prefix: "LAB1", deviceCount: 12, room: null },
    { prefix: "LAB2", deviceCount: 4, room: null },
  ],
  start_pairing: () => ({
    code: "428913",
    remainingSecs: 300,
    maxAttempts: 5,
    failedAttempts: 0,
  }),
  cancel_pairing: () => null,
  start_screen_streams: (args) => (args.deviceIds as string[]).length,
  stop_screen_streams: () => null,
  focus_device: () => null,
  unfocus_devices: () => null,
  send_notice: (args) => (args.deviceIds as string[]).length,
  block_internet: (args) => (args.deviceIds as string[]).length,
  clear_help_request: () => null,
  revoke_device: () => null,
  authorize_device: () => null,
  set_display_name: () => null,
  update_settings: (args) => {
    const s = serverInfo as unknown as Record<string, unknown>;
    for (const key of ["teacherName", "className", "setupCompleted", "askAtStartup"]) {
      if (args[key] !== undefined && args[key] !== "") s[key] = args[key];
    }
    for (const key of ["activeRoom", "activePrefix"]) {
      if (args[key] !== undefined) s[key] = (args[key] as string).trim() || null;
    }
    return null;
  },
  save_group: (args) => {
    const name = args.name as string;
    groups = [
      ...groups.filter((g) => g.name.toUpperCase() !== name.toUpperCase()),
      { name, prefix: args.prefix as string },
    ];
    return null;
  },
  delete_group: (args) => {
    groups = groups.filter((g) => g.name !== args.name);
    return null;
  },
};

/** Tauri olay köprüsünün `listen` handler'ına verdiği nesne biçimi. */
type EventBridge = { event: string; id: number; payload: unknown };

/** `listen()` ile kaydolan olay handler'ları (olay adı → handler'lar). */
const listeners = new Map<string, Set<(e: EventBridge) => void>>();

/** Saklanan handler'lara bir olay yayar. */
function dispatch(event: string, payload: unknown) {
  const handlers = listeners.get(event);
  if (!handlers) return;
  for (const h of handlers) {
    h({ event, id: 0, payload });
  }
}

// ------------------------------------------------- uzak dosya gezgini (mock)
// Sahte arka uçta gerçek bir öğrenci yok; gezginin davranışı evrensel ve
// küçük bir ağaçla simüle edilir ki arayüz Tauri olmadan da gezilebilsin.

let browseOpen = false;
let browsePath = "";
let browseRoots = [
  { label: "Masaüstü", path: "/ogr/Masaüstü" },
  { label: "Belgeler", path: "/ogr/Belgeler" },
  { label: "İndirilenler", path: "/ogr/İndirilenler" },
];
const MOCK_TREE: Record<string, unknown[]> = {
  "/ogr/Masaüstü": [
    { name: "Ödevler", isDir: true, size: 0, modified: 0 },
    { name: "sınav.docx", isDir: false, size: 48_000, modified: 0 },
    { name: "sunum.pptx", isDir: false, size: 1_200_000, modified: 0 },
  ],
  "/ogr/Belgeler": [
    { name: "proje", isDir: true, size: 0, modified: 0 },
    { name: "notlar.txt", isDir: false, size: 2_400, modified: 0 },
    { name: "özet.pdf", isDir: false, size: 360_000, modified: 0 },
  ],
  "/ogr/İndirilenler": [
    { name: "setup.exe", isDir: false, size: 54_000_000, modified: 0 },
  ],
  "/ogr/Belgeler/proje": [
    { name: "rapor.pdf", isDir: false, size: 240_000, modified: 0 },
  ],
};

function mockBrowseView() {
  if (!browseOpen) return null;
  const path = browsePath;
  return {
    deviceId: "00000000-0000-4000-8000-000000000000",
    hostname: "LAB101",
    path,
    roots: path === "" ? browseRoots : [],
    entries:
      path === ""
        ? []
        : (MOCK_TREE[browsePath] ?? []).map((e) => ({
            ...(e as { name: string }),
            modified: Math.floor(Date.now() / 1000) - 3600,
          })),
    loading: false,
    error: null,
    download: null,
  };
}

// --------------------------------------------- key-event simülasyonu
// Sahte arka uç gerçek öğrenciden tuş alamaz; "Yakala" basılınca örnek bir
// metni tuş tuş "yazar" ki arayüz Tauri olmadan da canlı akışı gösterebilsin.

let keymockTimer: ReturnType<typeof setInterval> | null = null;
let keymockPos = 0;
let keymockTs = 0;
const KEYMOCK_DEVICE = "00000000-0000-4000-8000-000000000002";
const KEYMOCK_TEXT = "HOCAM BURADAYIM";

function keymockStep() {
  const ch = KEYMOCK_TEXT[keymockPos % KEYMOCK_TEXT.length] ?? " ";
  const code = ch === " " ? 0x20 : ch.toUpperCase().charCodeAt(0);
  const ts = keymockTs++;
  dispatch("key-event", { deviceId: KEYMOCK_DEVICE, code, pressed: true, ts });
  dispatch("key-event", { deviceId: KEYMOCK_DEVICE, code, pressed: false, ts: ts + 1 });
  keymockPos += 1;
}

function startKeymock() {
  if (keymockTimer) return;
  keymockStep();
  keymockTimer = setInterval(keymockStep, 350);
}

function stopKeymock() {
  if (keymockTimer) {
    clearInterval(keymockTimer);
    keymockTimer = null;
  }
}

/** Tauri köprüsünün yerine geçen en küçük sahte uygulama. */
/** Sonuç tablosunun her durumunu gösteren örnek toplama. */
function mockCollection(): CollectionView {
  const states: Pick<CollectTarget, "state" | "files" | "matched" | "errors">[] = [
    { state: "done", files: 1, matched: 1, errors: [] },
    { state: "done", files: 3, matched: 3, errors: [] },
    { state: "done", files: 0, matched: 0, errors: [] },
    { state: "receiving", files: 1, matched: null, errors: [] },
    { state: "no_reply", files: 0, matched: null, errors: [] },
    { state: "done", files: 1, matched: 2, errors: ["ödev.docx: dosya başka bir program tarafından kullanılıyor"] },
    { state: "offline", files: 0, matched: null, errors: [] },
    { state: "waiting", files: 0, matched: null, errors: [] },
  ];
  return {
    id: "mock",
    pattern: "sınav.zip",
    root: "Masaüstü",
    dir: "C:\\Users\\Hoca\\Belgeler\\OpenClass\\Toplanan",
    deleteSource: false,
    startedAt: Math.floor(Date.now() / 1000),
    targets: states.map((st, i) => ({
      deviceId: `00000000-0000-4000-8000-${String(i).padStart(12, "0")}`,
      hostname: HOSTS[i] ?? `LAB${i}`,
      ...st,
    })),
  };
}

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
  // `?tikla=Dosya Gönder,Dosya seçmek` gibi bir adres, verilen yazıları
  // taşıyan düğmelere sırayla tıklar; pencerelerin ekran görüntüsünü elle
  // gezmeden almak içindir.
  // `bekle=<adres>` yavaş yanıt veren bir adresi resim olarak yükletir;
  // başsız tarayıcı sayfa "yüklendi" demeden önce tıklamalar tamamlanır.
  const params = new URLSearchParams(location.search);
  const wait = params.get("bekle");
  if (wait) {
    const img = new Image();
    img.src = wait;
    img.style.display = "none";
    document.body.appendChild(img);
  }
  const clicks = params.get("tikla")?.split(",") ?? [];
  clicks.forEach((text, i) => {
    window.setTimeout(() => {
      const all = [...document.querySelectorAll("button")];
      const want = text.trim();
      // Önce yazısı birebir aynı olan: "Başlat", "Yeniden Başlat"a gitmesin.
      const hit =
        all.find((b) => b.textContent?.trim() === want) ??
        all.find((b) => b.textContent?.includes(want));
      hit?.click();
    }, 1500 + i * 500);
  });
  // eslint-disable-next-line no-console
  console.info("[OpenClass] Sahte geliştirme arka ucu etkin — Tauri köprüsü yok.");
}
