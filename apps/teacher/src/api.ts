/**
 * Rust komutlarının tip güvenli sarmalayıcıları.
 *
 * Her çağrı hata durumunda Rust tarafından gelen **Türkçe** metni fırlatır;
 * arayüz bu metni doğrudan kullanıcıya gösterebilir.
 */
import { invoke } from "@tauri-apps/api/core";

import type {
  AuditEntry,
  BroadcastInfo,
  DeviceGroup,
  DeviceView,
  FileDestination,
  GroupSuggestion,
  BrowseView,
  InputEvent,
  TransferView,
  PairingInfo,
  Quality,
  ServerInfo,
} from "./types";

/** `invoke` hatalarını her zaman `Error` nesnesine çevirir. */
async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(cmd, args);
  } catch (err) {
    throw new Error(typeof err === "string" ? err : `Beklenmeyen hata: ${String(err)}`);
  }
}

export const api = {
  searchDevices: (query: string) => call<DeviceView[]>("search_devices", { query }),
  setDisplayName: (deviceId: string, name: string | null) =>
    call<void>("set_display_name", { deviceId, name }),
  revokeDevice: (deviceId: string) => call<void>("revoke_device", { deviceId }),

  startPairing: () => call<PairingInfo>("start_pairing"),
  cancelPairing: () => call<void>("cancel_pairing"),
  getPairing: () => call<PairingInfo | null>("get_pairing"),


  startScreenStreams: (deviceIds: string[], quality: Quality) =>
    call<number>("start_screen_streams", { deviceIds, quality }),
  stopScreenStreams: (deviceIds: string[]) =>
    call<void>("stop_screen_streams", { deviceIds }),
  focusDevice: (deviceId: string) => call<void>("focus_device", { deviceId }),
  unfocusDevices: () => call<void>("unfocus_devices"),

  sendNotice: (deviceIds: string[], title: string, body: string) =>
    call<number>("send_notice", { deviceIds, title, body }),
  clearHelpRequest: (deviceId: string) => call<void>("clear_help_request", { deviceId }),

  getServerInfo: () => call<ServerInfo>("get_server_info"),
  updateSettings: (settings: {
    teacherName?: string;
    className?: string;
    extraDiscoveryHosts?: string[];
    setupCompleted?: boolean;
    /** Boş metin "bütün laboratuvarlar" demektir. */
    activeRoom?: string;
    /** Boş metin "ön ek süzmesi yok" demektir. */
    activePrefix?: string;
  }) => call<void>("update_settings", settings),
  getAuditLog: (limit?: number) => call<AuditEntry[]>("get_audit_log", { limit }),

  listGroups: () => call<DeviceGroup[]>("list_groups"),
  saveGroup: (name: string, prefix: string) => call<void>("save_group", { name, prefix }),
  deleteGroup: (name: string) => call<void>("delete_group", { name }),
  suggestGroups: () => call<GroupSuggestion[]>("suggest_groups"),

  frameBaseUrl: () => call<string>("frame_base_url"),

  startRemoteControl: (deviceId: string) =>
    call<void>("start_remote_control", { deviceId }),
  stopRemoteControl: () => call<void>("stop_remote_control"),
  sendRemoteInput: (events: InputEvent[]) => call<void>("send_remote_input", { events }),

  startBroadcast: (fullscreen: boolean) =>
    call<BroadcastInfo>("start_broadcast", { fullscreen }),
  openBrowser: (deviceId: string) => call<void>("open_browser", { deviceId }),
  closeBrowser: () => call<void>("close_browser"),
  browseList: (path: string) => call<void>("browse_list", { path }),
  browseFetch: (path: string) => call<void>("browse_fetch", { path }),
  getBrowse: () => call<BrowseView | null>("get_browse"),
  openDownloadsDir: () => call<void>("open_downloads_dir"),
  getDownloadRoot: () => call<string>("get_download_root"),
  /** `null` varsayılan klasöre döner. Dönen değer yürürlükteki klasördür. */
  setDownloadRoot: (path: string | null) => call<string>("set_download_root", { path }),

  sendFile: (
    deviceIds: string[],
    path: string,
    destination: FileDestination,
    subpath: string | null,
  ) => call<string>("send_file", { deviceIds, path, destination, subpath }),
  getTransfers: () => call<TransferView[]>("get_transfers"),
  clearFinishedTransfers: () => call<void>("clear_finished_transfers"),

  lockDevices: (deviceIds: string[], message: string) =>
    call<number>("lock_devices", { deviceIds, message }),
  unlockDevices: (deviceIds: string[]) => call<number>("unlock_devices", { deviceIds }),
  stopBroadcast: () => call<void>("stop_broadcast"),
  getBroadcast: () => call<BroadcastInfo | null>("get_broadcast"),
};

/** Denetim kaydı türlerinin Türkçe karşılıkları. */
export const AUDIT_LABELS: Record<string, string> = {
  device_paired: "Cihaz eşleştirildi",
  remote_control_started: "Uzaktan kontrol başladı",
  remote_control_stopped: "Uzaktan kontrol bitti",
  screen_locked: "Ekran kilitlendi",
  screen_unlocked: "Ekran kilidi kaldırıldı",
  file_sent: "Dosya gönderildi",
  browse_opened: "Dosya gezgini açıldı",
  browse_listed: "Klasör listelendi",
  file_downloaded: "Dosya indirildi",
  device_rejected: "Eşleştirme reddedildi",
  device_revoked: "Cihaz yetkisi kaldırıldı",
  device_connected: "Cihaz bağlandı",
  device_disconnected: "Cihaz ayrıldı",
  auth_failed: "Kimlik doğrulama başarısız",
  screen_stream_started: "Ekran yayını başladı",
  screen_stream_stopped: "Ekran yayını durdu",
  notice_sent: "Mesaj gönderildi",
  help_requested: "Yardım talebi",
};

export function formatTime(unixSeconds: number): string {
  if (!unixSeconds) return "—";
  return new Date(unixSeconds * 1000).toLocaleTimeString("tr-TR", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}

export function formatDuration(sinceUnixSeconds: number): string {
  if (!sinceUnixSeconds) return "—";
  const total = Math.max(0, Math.floor(Date.now() / 1000) - sinceUnixSeconds);
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  if (h > 0) return `${h} sa ${m} dk`;
  if (m > 0) return `${m} dk ${s} sn`;
  return `${s} sn`;
}
