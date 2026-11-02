import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { ScreenCanvas } from "@openclass/screen-view";
import { Monitor } from "lucide-react";

import { api } from "./api";
import type { ActiveBroadcast } from "./types";

/**
 * Öğretmen ekranı penceresi.
 *
 * Tam ekran ve üstte durur. Öğrencinin bunu kapatması beklenmez, ama
 * **gizli de değildir**: üstte ne olduğunu ve kimin yayınladığını yazan bir
 * şerit vardır. Yayın bittiğinde pencereyi Rust tarafı kapatır.
 */
export function BroadcastWindow() {
  const [broadcast, setBroadcast] = useState<ActiveBroadcast | null>(null);
  const [baseUrl, setBaseUrl] = useState("");
  const [seq, setSeq] = useState(0);

  const refresh = useCallback(async () => {
    try {
      setBroadcast(await api.getBroadcast());
    } catch {
      /* yok sayılır */
    }
  }, []);

  useEffect(() => {
    void (async () => {
      setBaseUrl(await api.broadcastBaseUrl().catch(() => ""));
      await refresh();
    })();
  }, [refresh]);

  useEffect(() => {
    const unlisten = [
      listen<number>("broadcast-frame", (e) => setSeq(e.payload)),
      listen("broadcast-changed", () => void refresh()),
    ];
    return () => {
      void Promise.all(unlisten).then((fns) => fns.forEach((f) => f()));
    };
  }, [refresh]);

  return (
    <div className="flex h-full flex-col bg-black">
      <header className="flex h-[30px] shrink-0 items-center justify-center gap-2
        bg-[var(--color-accent)] px-3 text-[12px] font-semibold text-white">
        <Monitor size={14} />
        {broadcast
          ? `${broadcast.teacherName} ekranını paylaşıyor`
          : "Öğretmen ekranı"}
      </header>

      <main className="relative min-h-0 flex-1">
        {seq > 0 ? (
          <ScreenCanvas
            deviceId=""
            frameBaseUrl={baseUrl}
            seq={seq}
            alt="Öğretmen ekranı"
            className="size-full object-contain"
          />
        ) : (
          <div className="flex size-full items-center justify-center text-[13px] text-white/60">
            Öğretmen ekranı bekleniyor…
          </div>
        )}
      </main>
    </div>
  );
}
