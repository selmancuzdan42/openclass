import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { FolderOpen, MousePointerClick } from "lucide-react";

import { api } from "./api";
import type { Browsing, RemoteControl } from "./types";

/**
 * Etkinlik göstergesi.
 *
 * Ekranın üstünde, her zaman üstte, kapatılamaz küçük bir şerit. Öğrencinin
 * makinesinde onun adına bir şey yapıldığını bilmemesi kabul edilemez.
 *
 * Uzaktan kontrol ve dosya gezgini **aynı** pencereyi paylaşır: her biri
 * için ayrı WebView açmak 4 GB'lık laboratuvar makinesinde boşa giden
 * bellektir. Rust tarafı ikisi de kapandığında pencereyi yok eder.
 */
export function ActivityIndicator() {
  const [control, setControl] = useState<RemoteControl | null>(null);
  const [browsing, setBrowsing] = useState<Browsing | null>(null);

  useEffect(() => {
    void api.getRemoteControl().then(setControl).catch(() => {});
    void api.getBrowsing().then(setBrowsing).catch(() => {});

    const unlisteners = [
      listen<RemoteControl | null>("remote-control-changed", (e) => setControl(e.payload)),
      listen<Browsing | null>("browsing-changed", (e) => setBrowsing(e.payload)),
    ];
    return () => {
      for (const u of unlisteners) void u.then((f) => f());
    };
  }, []);

  const teacher = control?.teacherName ?? browsing?.teacherName ?? "Öğretmeniniz";

  return (
    <div className="flex h-full items-center justify-center gap-4 bg-[var(--color-warn)]
      px-3 text-[12px] font-semibold text-white select-none">
      {control && (
        <span className="flex items-center gap-1.5">
          <MousePointerClick size={14} />
          {teacher} bilgisayarınızı kullanıyor
        </span>
      )}
      {browsing && (
        <span className="flex items-center gap-1.5">
          <FolderOpen size={14} />
          {teacher} dosyalarınıza bakıyor
        </span>
      )}
      {!control && !browsing && (
        <span className="flex items-center gap-1.5">
          <MousePointerClick size={14} />
          OpenClass etkin
        </span>
      )}
    </div>
  );
}
