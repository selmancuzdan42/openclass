import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";
import { Mail } from "lucide-react";

import { api, formatTime } from "./api";
import { Button } from "./components";
import type { ReceivedNotice } from "./types";

/**
 * Öğretmen mesajı penceresi.
 *
 * Mesajlar kalıcı bir panelde birikmez; bu pencere mesaj geldiğinde açılır ve
 * öğrenci okuduktan sonra kapatılır. Öğrencinin ekranında sürekli duran bir
 * OpenClass penceresi yoktur.
 */
export function NoticeWindow() {
  const [notices, setNotices] = useState<ReceivedNotice[]>([]);

  const load = async () => {
    try {
      const snapshot = await api.getStatus();
      setNotices([...snapshot.notices].reverse());
    } catch {
      /* yok sayılır */
    }
  };

  useEffect(() => {
    void load();
    const unlisten = listen("notice", () => void load());
    return () => {
      void unlisten.then((f) => f());
    };
  }, []);

  const latest = notices[0];

  return (
    <div className="flex h-full flex-col bg-[var(--color-surface)]">
      <header className="flex h-[40px] shrink-0 items-center gap-2 border-b
        border-[var(--color-stroke)] bg-[var(--color-chrome)] px-3">
        <Mail size={16} className="text-[var(--color-accent)]" />
        <span className="text-[12.5px] font-semibold">Öğretmeninizden mesaj</span>
        {latest && (
          <span className="ml-auto tabular-nums text-[11px] text-[var(--color-text-3)]">
            {formatTime(latest.receivedAt)}
          </span>
        )}
      </header>

      <main className="min-h-0 flex-1 overflow-auto p-4">
        {!latest ? (
          <p className="text-[12px] text-[var(--color-text-3)]">Mesaj yok.</p>
        ) : (
          <>
            <p className="mb-2 text-[12px] font-semibold text-[var(--color-text-2)]">
              {latest.title}
            </p>
            <p
              data-selectable
              className="whitespace-pre-wrap break-words text-[16px] leading-relaxed"
            >
              {latest.body}
            </p>

            {notices.length > 1 && (
              <div className="mt-5 border-t border-[var(--color-stroke)] pt-3">
                <p className="mb-2 text-[11px] font-medium uppercase tracking-wide
                  text-[var(--color-text-3)]">
                  Önceki mesajlar
                </p>
                <ul className="space-y-2">
                  {notices.slice(1, 6).map((n) => (
                    <li key={n.messageId} className="text-[12px]">
                      <span className="tabular-nums text-[var(--color-text-3)]">
                        {formatTime(n.receivedAt)}
                      </span>{" "}
                      <span data-selectable>{n.body}</span>
                    </li>
                  ))}
                </ul>
              </div>
            )}
          </>
        )}
      </main>

      <footer className="flex h-[52px] shrink-0 items-center justify-end gap-2 border-t
        border-[var(--color-stroke)] bg-[var(--color-surface-2)] px-3">
        <Button tone="accent" className="!h-[32px] !px-5" onClick={() => void getCurrentWindow().close()}>
          Tamam
        </Button>
      </footer>
    </div>
  );
}
