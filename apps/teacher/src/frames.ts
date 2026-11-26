/**
 * Canlı kare bilgisi deposu.
 *
 * Kare olayları saniyede onlarca gelir. Bunlar cihaz listesine yazılırsa her
 * olay bütün konsolu yeniden çizdirir; zayıf öğretmen bilgisayarında arayüz
 * takılır. Bu yüzden kare sırası listeden ayrı tutulur ve yalnızca ilgili
 * ekranı gösteren bileşen yeniden çizilir.
 */
import { useCallback, useSyncExternalStore } from "react";

import type { DeviceView } from "./types";

export interface LiveFrame {
  seq: number;
  width: number;
  height: number;
}

const frames = new Map<string, LiveFrame>();
const listeners = new Map<string, Set<() => void>>();

function notify(deviceId: string) {
  listeners.get(deviceId)?.forEach((fn) => fn());
}

/** Yeni kareyi kaydeder; cihazın ilk karesiyse `true` döner. */
export function pushFrame(deviceId: string, seq: number, width: number, height: number): boolean {
  const first = !frames.has(deviceId);
  frames.set(deviceId, { seq, width, height });
  notify(deviceId);
  return first;
}

/**
 * Depoyu tazelenen cihaz listesiyle eşitler: görüntüsü kalmayan cihazın
 * kaydı silinir, olay gelmeden önce listede görülen kare eklenir.
 */
export function syncFrames(devices: DeviceView[]) {
  const live = new Set<string>();
  for (const d of devices) {
    if (!d.online || d.frameSeq === 0) continue;
    live.add(d.deviceId);
    if (!frames.has(d.deviceId)) {
      frames.set(d.deviceId, { seq: d.frameSeq, width: d.frameWidth, height: d.frameHeight });
      notify(d.deviceId);
    }
  }
  for (const id of [...frames.keys()]) {
    if (!live.has(id)) {
      frames.delete(id);
      notify(id);
    }
  }
}

/** Cihazın en güncel kare bilgisi; görüntü yoksa `null`. */
export function useLiveFrame(device: DeviceView | null): LiveFrame | null {
  const id = device?.deviceId ?? "";
  const subscribe = useCallback(
    (onChange: () => void) => {
      let set = listeners.get(id);
      if (!set) listeners.set(id, (set = new Set()));
      set.add(onChange);
      return () => {
        set.delete(onChange);
        if (set.size === 0) listeners.delete(id);
      };
    },
    [id],
  );
  const frame = useSyncExternalStore(subscribe, () => frames.get(id) ?? null);
  return device?.online ? frame : null;
}
