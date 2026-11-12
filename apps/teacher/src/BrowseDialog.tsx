/**
 * Uzak dosya gezgini — öğretmenin öğrencinin dosya sisteminde dolaşıp
 * dosya indirdiği diyalog.
 *
 * Sol panelde öğrencinin açık kökleri (Masaüstü / Belgeler / İndirilenler)
 * durur; sağda o konumun klasör ve dosya listesi gösterilir. Klasöre
 * tıklanınca içeri girilir, "Üst" ile bir üst konuma dönülür. Bir dosyaya
 * tıklanınca veya satırdaki "İndir" düğmesine basılınca öğretmenin kendi
 * makinesinde bir hedef klasör seçmesi istenir ve dosya oraya indirilir.
 *
 * Yalnızca listeleme ve indirme vardır — yazma, silme, çalıştırma protokolde
 * tanımlı değildir ve bu diyalogdan da yapılamaz.
 */
import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { open as openFolderDialog } from "@tauri-apps/plugin-dialog";
import {
  ArrowUp,
  Download,
  Folder,
  File as FileIcon,
  HardDrive,
  Loader2,
  RefreshCw,
} from "lucide-react";

import { api } from "./api";
import { Button, Empty, Modal, useToast } from "./ui";
import type { BrowseEntry, BrowseView, DeviceView } from "./types";

interface BrowseDialogProps {
  open: boolean;
  device: DeviceView | null;
  onClose: () => void;
}

/** Gezinme yığını: en son eleman o an görüntülenen klasör, boşsa kök listesi. */
type NavItem = { path: string; name: string };

/** Bir dosya boyutunu okunur biçime çevirir. */
function formatSize(bytes: number): string {
  if (bytes <= 0) return "";
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
}

/** Unix saniyeyi tarih olarak yazar. */
function formatDate(unix: number): string {
  if (!unix) return "—";
  return new Date(unix * 1000).toLocaleDateString("tr-TR", {
    day: "2-digit",
    month: "2-digit",
    year: "numeric",
  });
}

/** İki yol parçasını tek tam yola çevirir (Linux / ve Windows \\ destekler). */
function joinPath(base: string, name: string): string {
  if (!base) return name;
  const sep = base.includes("\\") && !base.includes("/") ? "\\" : "/";
  return base.replace(/[\\/]+$/, "") + sep + name;
}

export function BrowseDialog({ open, device, onClose }: BrowseDialogProps) {
  const { show } = useToast();
  const [view, setView] = useState<BrowseView | null>(null);
  // Kullanıcının izlediği konum yığını; boşsa kök listesi gösterilir.
  const [nav, setNav] = useState<NavItem[]>([]);
  // Bir hedef klasör seçimi beklenirken tıklamalar kilitlenir.
  const [picking, setPicking] = useState(false);
  const deviceIdRef = useRef<string | null>(null);
  const mountedRef = useRef(false);

  // Diyalog açılınca öğrencideki gezgini başlat; kapanınca temizle.
  useEffect(() => {
    if (!open || !device) return;
    if (deviceIdRef.current === device.deviceId) return;
    deviceIdRef.current = device.deviceId;
    setView(null);
    setNav([]);
    setPicking(false);
    void api
      .openBrowser(device.deviceId)
      .catch((e) => show("error", e.message));
  }, [open, device, show]);

  useEffect(() => {
    mountedRef.current = open;
    return () => {
      mountedRef.current = false;
    };
  }, [open]);

  // Kapanırken öğrencideki gezgini kapat (yarım kalan indirmeyi de durdurur).
  useEffect(() => {
    if (open) return;
    if (!deviceIdRef.current) return;
    deviceIdRef.current = null;
    void api.closeBrowser().catch(() => {});
  }, [open]);

  // Durum akışı: `browse-changed` olayı + düzenli `get_browse` yoklaması.
  // İkisi birden tutulur ki olay gecikse bile arayüz güncel kalsın.
  const refresh = useCallback(async () => {
    try {
      const v = await api.getBrowse();
      if (mountedRef.current) setView(v);
    } catch {
      /* yok sayılır — bir sonraki yoklamada denenir */
    }
  }, []);

  useEffect(() => {
    if (!open) return;
    const unlisten = listen<BrowseView | null>("browse-changed", (e) => {
      if (mountedRef.current) setView(e.payload ?? null);
    });
    // İndirme parçaları olay olarak gelebilir ama yedek yoklama hızlı kalır.
    const id = window.setInterval(refresh, 1200);
    void refresh();
    return () => {
      void unlisten.then((f) => f());
      window.clearInterval(id);
    };
  }, [open, refresh]);

  const browsing = device !== null;

  function enterRoot(path: string, name: string) {
    setNav([{ path, name }]);
    void api.browseList(path).catch((e) => show("error", e.message));
  }

  function enterFolder(entry: BrowseEntry, currentPath: string) {
    const path = joinPath(currentPath, entry.name);
    setNav((stack) => [...stack, { path, name: entry.name }]);
    void api.browseList(path).catch((e) => show("error", e.message));
  }

  function goUp() {
    if (nav.length === 0) return;
    // Kök yığının dibinde: "Üst" kök listesine döner.
    const next = nav.slice(0, -1);
    setNav(next);
    const target = next.length ? next[next.length - 1].path : "";
    void api.browseList(target).catch((e) => show("error", e.message));
  }

  const currentFolder: NavItem | null = nav.length
    ? nav[nav.length - 1]
    : null;

  function renderNav() {
    if (nav.length === 0) return null;
    return (
      <div className="mb-2 flex items-center gap-2 truncate">
        <Button
          onClick={goUp}
          className="shrink-0"
        >
          <ArrowUp size={13} />
          Üst
        </Button>
        <span className="min-w-0 truncate text-[12px] text-[var(--color-text-2)]">
          {currentFolder?.name}
        </span>
      </div>
    );
  }

  async function startDownload(entry: BrowseEntry, currentPath: string) {
    if (!browsing || picking || busy) return;
    const path = joinPath(currentPath, entry.name);
    setPicking(true);
    let target: string | null = null;
    try {
      // "kaydedilecek yeri seç": native klasör seçimi her dosya indirilmeden
      // önce açılır; seçilen klasör o indirme için hedef olur.
      const picked = await openFolderDialog({
        directory: true,
        multiple: false,
        title: "Kaydedilecek yeri seç",
      });
      if (typeof picked === "string") target = picked;
    } catch (e) {
      show("error", e instanceof Error ? e.message : String(e));
      return;
    } finally {
      setPicking(false);
    }

    // İptal edilirse bu dosya atlanır.
    if (target === null) return;

    const ok = await api
      .browseFetch(path, target)
      .then(() => true)
      .catch((e) => {
        show("error", e instanceof Error ? e.message : String(e));
        return false;
      });
    if (ok) show("info", `"${entry.name}" indiriliyor…`);
  }

  const download = view?.download ?? null;
  const busy = download !== null && !download.finished;
  const lastDownload =
    download !== null && download.finished
      ? { ok: download.ok, text: download.detail || download.name }
      : null;

  return (
    <Modal
      open={open}
      title={
        device
          ? `${device.hostname} — Dosya Al`
          : "Dosya Al"
      }
      onClose={onClose}
      width="xl"
      footer={
        <div className="flex w-full items-center justify-between">
          {/* İndirme durumu / son sonuç */}
          <div className="min-w-0 flex-1 pr-4">
            {download && !download.finished && (
              <div className="flex items-center gap-2 text-[11px] text-[var(--color-text-2)]">
                <Loader2 size={13} className="animate-spin" />
                <span className="max-w-[260px] truncate">{download.name}</span>
                <span className="ml-auto shrink-0 tabular-nums">
                  {formatSize(download.received)} / {formatSize(download.size)}
                </span>
              </div>
            )}
            {lastDownload && (
              <div
                className={`flex items-center gap-1.5 text-[11px] ${
                  lastDownload.ok
                    ? "text-[var(--color-ok)]"
                    : "text-[var(--color-danger)]"
                }`}
              >
                {lastDownload.ok ? "✓" : "✕"} İndirme tamamlandı:{" "}
                <span className="min-w-0 flex-1 truncate">{lastDownload.text}</span>
              </div>
            )}
          </div>
          <div className="flex gap-2">
            <Button tone="subtle" onClick={() => void refresh()}>
              <RefreshCw size={13} />
              Yenile
            </Button>
            <Button onClick={onClose}>Kapat</Button>
          </div>
        </div>
      }
    >
      {/* Sol: konumlar */}
      <div className="flex h-full gap-2">
        <aside className="w-[200px] shrink-0 overflow-auto rounded border border-[var(--color-stroke)]">
          <div className="sticky top-0 flex h-[28px] items-center gap-1.5 border-b border-[var(--color-stroke)] bg-[var(--color-surface-2)] px-2 text-[11px] font-semibold text-[var(--color-text-2)]">
            <HardDrive size={12} />
            Konumlar
          </div>
          {(view?.roots ?? []).length === 0 && !view?.loading && (
            <div className="p-3 text-[11px] text-[var(--color-text-3)]">
              Konum yüklenemedi.
            </div>
          )}
          {(view?.roots ?? []).map((root) => {
            const active = currentFolder?.path === root.path;
            return (
              <button
                key={root.path}
                onClick={() => enterRoot(root.path, root.label)}
                className={`flex w-full items-center gap-2 px-2 py-[6px] text-left text-[12px]
                  transition-colors
                  ${
                    active
                      ? "bg-[var(--color-accent-soft)] text-[var(--color-accent)]"
                      : "text-[var(--color-text)] hover:bg-[var(--color-surface-3)]"
                  }`}
              >
                <Folder size={14} className="shrink-0" />
                <span className="min-w-0 flex-1 truncate">{root.label}</span>
              </button>
            );
          })}
        </aside>

        {/* Sağ: içerik listesi */}
        <section className="flex min-w-0 flex-1 flex-col">
          {renderNav()}
          <div className="min-h-0 flex-1 overflow-auto rounded border border-[var(--color-stroke)]">
            {view?.loading ? (
              <div className="flex h-full items-center justify-center gap-2 py-10 text-[11px] text-[var(--color-text-3)]">
                <Loader2 size={14} className="animate-spin" />
                Yükleniyor…
              </div>
            ) : view?.error ? (
              <div className="flex h-full items-center justify-center px-6 py-10 text-[11px] text-[var(--color-danger)]">
                {view.error}
              </div>
            ) : view?.path === "" &&
              currentFolder === null &&
              (view?.roots?.length ?? 0) === 0 ? (
              <Empty
                title="Gezgi boş"
                detail="Soldaki bir konuma tıklayarak öğrencinin dosyalarını gezebilirsiniz."
              />
            ) : (view?.entries?.length ?? 0) === 0 ? (
              <Empty title="Bu klasör boş" />
            ) : (
              <table className="w-full border-collapse text-[12px]">
                <thead>
                  <tr className="sticky top-0 bg-[var(--color-surface-2)] text-left text-[10.5px] uppercase tracking-wide text-[var(--color-text-3)]">
                    <th className="px-2 py-1 font-medium">Ad</th>
                    <th className="w-[90px] px-2 py-1 font-medium">Boyut</th>
                    <th className="w-[100px] px-2 py-1 font-medium">Değişiklik</th>
                    <th className="w-[74px] px-2 py-1 font-medium" />
                  </tr>
                </thead>
                <tbody>
                  {(view?.entries ?? []).map((entry) => (
                    <tr
                      key={entry.name}
                      className={`group border-t border-[var(--color-stroke)] ${
                        entry.isDir
                          ? "cursor-pointer hover:bg-[var(--color-surface-3)]"
                          : "cursor-default hover:bg-[var(--color-surface-3)]"
                      }`}
                      onClick={() => {
                        if (entry.isDir) {
                          enterFolder(entry, view?.path ?? "");
                        } else {
                          void startDownload(entry, view?.path ?? "");
                        }
                      }}
                    >
                      <td className="px-2 py-[5px]">
                        <span className="flex items-center gap-2">
                          {entry.isDir ? (
                            <Folder size={14} className="shrink-0 text-[var(--color-accent)]" />
                          ) : (
                            <FileIcon size={14} className="shrink-0 text-[var(--color-text-3)]" />
                          )}
                          <span className="min-w-0 truncate">{entry.name}</span>
                        </span>
                      </td>
                      <td className="px-2 py-[5px] tabular-nums text-[var(--color-text-2)]">
                        {entry.isDir ? "" : formatSize(entry.size)}
                      </td>
                      <td className="px-2 py-[5px] text-[var(--color-text-2)]">
                        {formatDate(entry.modified)}
                      </td>
                      <td className="px-2 py-[5px] text-right">
                        {!entry.isDir && (
                          <Button
                            tone="default"
                            className="opacity-0 transition-opacity group-hover:opacity-100"
                            disabled={picking || busy}
                            onClick={(e) => {
                              e.stopPropagation();
                              void startDownload(entry, view?.path ?? "");
                            }}
                          >
                            <Download size={13} />
                            İndir
                          </Button>
                        )}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </div>
        </section>
      </div>
    </Modal>
  );
}
