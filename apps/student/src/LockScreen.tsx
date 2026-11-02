import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { Lock } from "lucide-react";

import { api } from "./api";
import type { ScreenLock } from "./types";

/**
 * Ekran kilidi penceresi.
 *
 * Tam ekran kaplar ve kilidin **neden** olduğunu söyler. Sessizce donmuş bir
 * makine bırakmak kabul edilemez: öğrenci ne olduğunu ve ne yapması
 * gerektiğini okumalı.
 *
 * Klavye ve fare işletim sistemi düzeyinde engellenir (bkz. `lock.rs`);
 * burada yapılacak bir şey yok. Pencereyi Rust tarafı kapatır.
 */
export function LockScreen() {
  const [lock, setLock] = useState<ScreenLock | null>(null);

  useEffect(() => {
    void api.getScreenLock().then(setLock).catch(() => {});
    const unlisten = listen<ScreenLock | null>("lock-changed", (e) => setLock(e.payload));
    return () => {
      void unlisten.then((f) => f());
    };
  }, []);

  return (
    <div className="flex size-full flex-col items-center justify-center gap-6 bg-[#0d1117]
      px-10 text-center text-white select-none">
      <div className="flex size-20 items-center justify-center rounded-full
        bg-[var(--color-accent)]/15">
        <Lock size={38} className="text-[var(--color-accent)]" />
      </div>

      <div className="space-y-2">
        <h1 className="text-[26px] font-semibold">Ekran geçici olarak kilitli</h1>
        <p className="max-w-xl text-[15px] leading-relaxed text-white/70">
          {lock?.message?.trim()
            ? lock.message
            : "Öğretmeniniz dersin bu bölümünde bilgisayarları kilitledi."}
        </p>
      </div>

      <p className="max-w-lg text-[12.5px] leading-relaxed text-white/45">
        Klavye ve fareniz kısa süreliğine devre dışı. Ders bölümü bittiğinde
        kilit kendiliğinden kalkar. Bir sorun olursa{" "}
        <kbd className="rounded border border-white/20 px-1.5 py-0.5 text-[11px]">
          Ctrl
        </kbd>
        {" + "}
        <kbd className="rounded border border-white/20 px-1.5 py-0.5 text-[11px]">
          Alt
        </kbd>
        {" + "}
        <kbd className="rounded border border-white/20 px-1.5 py-0.5 text-[11px]">
          Del
        </kbd>{" "}
        her zaman çalışır.
      </p>
    </div>
  );
}
