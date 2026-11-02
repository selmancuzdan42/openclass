import { invoke } from "@tauri-apps/api/core";

import type {
  ActiveBroadcast,
  AutostartState,
  Browsing,
  DiscoveredTeacher,
  RemoteControl,
  ScreenLock,
  StatusSnapshot,
} from "./types";

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(cmd, args);
  } catch (err) {
    throw new Error(typeof err === "string" ? err : `Beklenmeyen hata: ${String(err)}`);
  }
}

export const api = {
  getStatus: () => call<StatusSnapshot>("get_status"),
  listTeachers: () => call<DiscoveredTeacher[]>("list_teachers"),
  pairWithTeacher: (args: { teacherId?: string; address?: string; code: string }) =>
    call<void>("pair_with_teacher", args),
  unpair: () => call<void>("unpair"),
  setManualAddress: (address: string | null) => call<void>("set_manual_address", { address }),
  /** Laboratuvar/oda adını kaydeder. Boş metin alanı temizler. */
  setRoom: (room: string) => call<void>("set_room", { room }),
  reconnectNow: () => call<void>("reconnect_now"),
  requestHelp: (note?: string) => call<void>("request_help", { note }),
  cancelHelp: () => call<void>("cancel_help"),
  clearNotices: () => call<void>("clear_notices"),
  hideConfigWindow: () => call<void>("hide_config_window"),
  quitAgent: () => call<void>("quit_agent"),
  getBroadcast: () => call<ActiveBroadcast | null>("get_broadcast"),
  broadcastBaseUrl: () => call<string>("broadcast_base_url"),
  getRemoteControl: () => call<RemoteControl | null>("get_remote_control"),
  getAutostart: () => call<AutostartState>("get_autostart"),
  getScreenLock: () => call<ScreenLock | null>("get_screen_lock"),
  getBrowsing: () => call<Browsing | null>("get_browsing"),
  setAutostart: (enabled: boolean) => call<void>("set_autostart", { enabled }),
};

export function formatTime(unixSeconds: number): string {
  if (!unixSeconds) return "—";
  return new Date(unixSeconds * 1000).toLocaleTimeString("tr-TR", {
    hour: "2-digit",
    minute: "2-digit",
  });
}
