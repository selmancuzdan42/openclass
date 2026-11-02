/**
 * Geliştirme amaçlı sahte arka uç.
 *
 * Yalnızca uygulama Tauri dışında, düz bir tarayıcıda açıldığında devreye
 * girer. Rust tarafını derlemeden arayüz üzerinde çalışmayı sağlar ve
 * üretim derlemesine dahil edilmez.
 */
import type { StatusSnapshot } from "./types";

const snapshot: StatusSnapshot = {
  hostname: "LAB101",
  deviceId: "731583ec-5d80-47c9-819d-0b4e260e8e0b",
  appVersion: "0.1.0",
  status: { kind: "connected", teacherName: "Cihan Güngör" },
  sharing: {
    active: true,
    backend: "Windows Graphics Capture",
    fps: 2,
    width: 480,
    height: 270,
    error: null,
  },
  helpRequested: false,
  pairedTeacherName: "Cihan Güngör",
  pairedTeacherAddress: "192.168.1.18:7643",
  manualAddress: null,
  notices: [
    {
      messageId: "aaaa1111-1111-4111-8111-111111111111",
      title: "Cihan Güngör",
      body: "Sayfa 42'deki alıştırmaya geçin, 10 dakikanız var.",
      receivedAt: Math.floor(Date.now() / 1000) - 240,
    },
    {
      messageId: "bbbb2222-2222-4222-8222-222222222222",
      title: "Cihan Güngör",
      body: "Ders bitiminde dosyalarınızı kaydetmeyi unutmayın.",
      receivedAt: Math.floor(Date.now() / 1000) - 60,
    },
  ],
  captureSupported: true,
  // Sihirbazı denemek için `null` yapın: oda ve eşleştirme yoksa açılır.
  room: "Lab 1",
};

const HANDLERS: Record<string, () => unknown> = {
  "plugin:event|listen": () => Math.floor(Math.random() * 1e9),
  "plugin:event|unlisten": () => null,
  get_status: () => snapshot,
  list_teachers: () => [
    {
      teacherId: "22222222-2222-4222-8222-222222222222",
      teacherName: "Cihan Güngör",
      host: "192.168.1.18",
      controlPort: 7643,
      fingerprintHex: "11:05:F0:6B:9F:B2:A9:46:96:F3:CB:BF:11:32:AE:22",
      lastSeen: Math.floor(Date.now() / 1000),
    },
  ],
  reconnect_now: () => null,
  request_help: () => null,
  cancel_help: () => null,
  clear_notices: () => null,
  set_manual_address: () => null,
  set_room: () => null,
  unpair: () => null,
  pair_with_teacher: () => null,
  hide_config_window: () => null,
  quit_agent: () => null,
  get_broadcast: () => null,
  get_remote_control: () => null,
  get_autostart: () => ({ supported: true, enabled: false }),
  get_screen_lock: () => null,
  get_browsing: () => null,
  set_autostart: () => null,
  broadcast_base_url: () => "/mock-frame",
};

export function installDevMock() {
  const w = window as unknown as Record<string, unknown>;
  w.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
  w.__TAURI_INTERNALS__ = {
    invoke: async (cmd: string) => {
      const handler = HANDLERS[cmd];
      if (!handler) throw `Sahte arka uçta tanımsız komut: ${cmd}`;
      return handler();
    },
    transformCallback: (cb: unknown) => cb,
  };
  // eslint-disable-next-line no-console
  console.info("[OpenClass] Sahte geliştirme arka ucu etkin — Tauri köprüsü yok.");
}
