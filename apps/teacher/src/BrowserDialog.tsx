import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  ArrowUp,
  CheckCircle2,
  Download,
  Eye,
  FileIcon,
  FileUp,
  Folder,
  FolderCog,
  FolderOpen,
  HardDrive,
  XCircle,
} from "lucide-react";

import { open as openFolderDialog } from "@tauri-apps/plugin-dialog";

import { api, formatTime } from "./api";
import { Button, Empty, Modal, useToast } from "./ui";
import type { BrowseEntry, BrowseRoot, BrowseView, FileDestination } from "./types";

/**
 * Uzak dosya gezgini.
 *
 * **Yalnızca listeleme ve indirme.** Yazma, silme, yeniden adlandırma ve
 * çalıştırma yoktur; protokolde böyle bir mesaj da tanımlı değil.
 *
 * Kapsam kararını öğrenci tarafı verir: burada bir yol yazılamaz, yalnızca
 * öğrencinin açtığı köklerde gezilir.
 */
export function BrowserDialog({
  open,
  pickFolder,
  onPick,
  onClose,
}: {
  open: boolean;
  /**
   * Klasör seçme kipi: indirme yerine hedef klasör seçilir.
   *
   * Dosya gönderiminde kullanılır. Gönderim birden çok cihaza gidebildiği
   * için seçim tek bir öğrencinin gezgininde yapılır; taşınan değer köke
   * göre göreli olduğundan aynı yol herkeste kendi klasörüne denk gelir.
   */
  pickFolder?: { fileName: string; deviceCount: number };
  onPick?: (destination: FileDestination, subpath: string | null, shown: string) => void;
  onClose: () => void;
}) {
  const { show } = useToast();
  const [view, setView] = useState<BrowseView | null>(null);
  /** İndirmelerin yazıldığı klasör; oturum boyunca korunur. */
  const [downloadRoot, setDownloadRoot] = useState<string>("");

  useEffect(() => {
    if (!open) return;
    void api.getBrowse().then(setView).catch(() => {});
    void api.getDownloadRoot().then(setDownloadRoot).catch(() => {});
    const unlisten = listen<BrowseView | null>("browse-changed", (e) => setView(e.payload));
    return () => {
      void unlisten.then((f) => f());
    };
  }, [open]);

  /** İndirme klasörünü değiştirir. Seçim oturum boyunca hatırlanır. */
  async function pickDownloadRoot() {
    const picked = await openFolderDialog({
      directory: true,
      multiple: false,
      title: "İndirilen dosyaların toplanacağı klasörü seçin",
    });
    if (typeof picked !== "string") return;
    try {
      setDownloadRoot(await api.setDownloadRoot(picked));
    } catch (err) {
      show("error", err instanceof Error ? err.message : String(err));
    }
  }

  /**
   * Gezgindeki mutlak yolu kök + göreli yola ayırır.
   *
   * Öğrencinin kullanıcı klasörü her makinede başka yerdedir
   * (`C:\\Users\\ali\\Belgeler`), bu yüzden mutlak yol taşınamaz. Kök
   * etiketi kapalı bir kümeye (`FileDestination`) eşlenir, kalan kısım
   * göreli yol olarak gider.
   *
   * `OpenClass` kökü Belgeler'in altında olduğundan kendiliğinden
   * `documents` + `OpenClass` olarak çözülür.
   */
  function resolveTarget(
    path: string,
    roots: BrowseRoot[],
  ): { destination: FileDestination; subpath: string | null } | null {
    const eslesme: Record<string, FileDestination> = {
      "Masaüstü": "desktop",
      Belgeler: "documents",
      İndirilenler: "downloads",
    };

    let best: { root: BrowseRoot; destination: FileDestination } | null = null;
    for (const r of roots) {
      const destination = eslesme[r.label];
      if (!destination) continue;
      const altinda =
        path === r.path || path.startsWith(r.path + "\\") || path.startsWith(r.path + "/");
      if (!altinda) continue;
      // En uzun eşleşen kök kazanır: iç içe kökler olabilir.
      if (!best || r.path.length > best.root.path.length) best = { root: r, destination };
    }
    if (!best) return null;

    const rel = path.slice(best.root.path.length).replace(/^[\\/]+/, "");
    return { destination: best.destination, subpath: rel || null };
  }

  const atRoots = !view?.path;
  const parent = parentOf(view?.path ?? "");
  // Klasör seçme kipinde: şu an bulunulan klasör geçerli bir hedef mi?
  const hedef =
    pickFolder && view?.path ? resolveTarget(view.path, view.roots) : null;

  return (
    <Modal
      open={open}
      width="xl"
      title={
        pickFolder
          ? `“${pickFolder.fileName}” nereye gönderilsin?`
          : view
            ? `${view.hostname} — dosyalar`
            : "Dosya gezgini"
      }
      onClose={onClose}
      footer={
        pickFolder ? (
          <>
            <span className="mr-auto text-[11px] leading-snug text-[var(--color-text-3)]">
              {view?.hostname} üzerinde seçiyorsunuz; aynı klasör{" "}
              {pickFolder.deviceCount} cihazın her birinde kendi
              kullanıcı klasörü altında kullanılır.
            </span>
            <Button
              onClick={async () => {
                await api.closeBrowser().catch(() => {});
                onClose();
              }}
            >
              Vazgeç
            </Button>
            <Button
              tone="accent"
              disabled={!view || atRoots || !hedef}
              title={
                atRoots
                  ? "Bir klasöre girin"
                  : hedef
                    ? undefined
                    : "Bu konuma gönderilemez"
              }
              onClick={async () => {
                if (!hedef || !view) return;
                await api.closeBrowser().catch(() => {});
                onPick?.(hedef.destination, hedef.subpath, view.path);
              }}
            >
              <FileUp size={13} /> Buraya Gönder
            </Button>
          </>
        ) : (
          <>
            <span className="mr-auto flex items-center gap-1.5 text-[11px]
              text-[var(--color-text-3)]">
              <Eye size={12} />
              Öğrencide “dosyalarınıza bakılıyor” göstergesi duruyor
            </span>
            <Button
              title={downloadRoot ? `İndirmeler: ${downloadRoot}` : "İndirme klasörünü seç"}
              onClick={() => void pickDownloadRoot()}
            >
              <FolderCog size={13} /> Klasörü Değiştir
            </Button>
            <Button
              title={downloadRoot || undefined}
              onClick={() => void api.openDownloadsDir()}
            >
              <FolderOpen size={13} /> Toplanan Klasörü
            </Button>
            <Button
              onClick={async () => {
                await api.closeBrowser().catch(() => {});
                onClose();
              }}
            >
              Kapat
            </Button>
          </>
        )
      }
    >
      {!view ? (
        <Empty title="Gezgin kapalı" detail="Bir cihaz seçip gezgini açın." />
      ) : (
        <div className="flex flex-col">
          {/* Yol çubuğu */}
          <div className="flex items-center gap-2 border-b border-[var(--color-line)]
            bg-[var(--color-bg-1)] px-3 py-2">
            <Button
              disabled={atRoots}
              title={atRoots ? "En üst seviyedesiniz" : "Üst klasör"}
              onClick={() => void api.browseList(parent)}
            >
              <ArrowUp size={13} />
            </Button>
            <span className="min-w-0 flex-1 truncate font-mono text-[11.5px]
              text-[var(--color-text-2)]">
              {view.path || "Paylaşılan konumlar"}
            </span>
            {view.loading && (
              <span className="text-[11px] text-[var(--color-text-3)]">yükleniyor…</span>
            )}
          </div>

          {view.error && (
            <p className="border-b border-[var(--color-line)] bg-[var(--color-danger)]/10
              px-3 py-2 text-[11.5px] text-[var(--color-danger)]">
              {view.error}
            </p>
          )}

          {view.download && <DownloadBar download={view.download} />}

          {/* İçerik */}
          <div className="max-h-[52vh] overflow-y-auto">
            {atRoots ? (
              view.roots.length === 0 ? (
                <Empty
                  title="Paylaşılan konum yok"
                  detail="Öğrencide erişilebilir bir klasör bulunamadı."
                />
              ) : (
                view.roots.map((r) => (
                  <button
                    key={r.path}
                    onClick={() => void api.browseList(r.path)}
                    className="flex w-full items-center gap-2 border-b
                      border-[var(--color-line)] px-3 py-2 text-left text-[12.5px]
                      hover:bg-[var(--color-bg-2)]"
                  >
                    <HardDrive size={14} className="text-[var(--color-accent)]" />
                    <span className="font-medium">{r.label}</span>
                    <span className="ml-auto truncate font-mono text-[11px]
                      text-[var(--color-text-3)]">
                      {r.path}
                    </span>
                  </button>
                ))
              )
            ) : view.entries.length === 0 ? (
              <Empty title="Klasör boş" detail="Burada gösterilecek bir şey yok." />
            ) : (
              <table className="w-full">
                <tbody>
                  {view.entries.map((e) => (
                    <Row key={e.name} entry={e} dir={view.path} busy={!!view.download} />
                  ))}
                </tbody>
              </table>
            )}
          </div>
        </div>
      )}
    </Modal>
  );
}

function Row({
  entry,
  dir,
  busy,
}: {
  entry: BrowseEntry;
  dir: string;
  busy: boolean;
}) {
  const path = joinPath(dir, entry.name);

  return (
    <tr className="border-b border-[var(--color-line)] hover:bg-[var(--color-bg-2)]">
      <td className="w-6 pl-3">
        {entry.isDir ? (
          <Folder size={14} className="text-[var(--color-accent)]" />
        ) : (
          <FileIcon size={14} className="text-[var(--color-text-3)]" />
        )}
      </td>
      <td className="py-1.5 pl-2 text-[12.5px]">
        {entry.isDir ? (
          <button
            onClick={() => void api.browseList(path)}
            className="text-left hover:underline"
          >
            {entry.name}
          </button>
        ) : (
          entry.name
        )}
      </td>
      <td className="w-24 text-right text-[11px] text-[var(--color-text-3)]">
        {entry.isDir ? "—" : formatSize(entry.size)}
      </td>
      <td className="w-32 pr-2 text-right text-[11px] text-[var(--color-text-3)]">
        {entry.modified > 0 ? formatTime(entry.modified) : "—"}
      </td>
      <td className="w-10 pr-3 text-right">
        {!entry.isDir && (
          <button
            disabled={busy}
            title={busy ? "Başka bir indirme sürüyor" : "Bu dosyayı indir"}
            onClick={() => void api.browseFetch(path)}
            className="rounded p-1 hover:bg-[var(--color-bg-3)] disabled:opacity-40"
          >
            <Download size={13} />
          </button>
        )}
      </td>
    </tr>
  );
}

function DownloadBar({ download }: { download: BrowseView["download"] }) {
  if (!download) return null;

  const percent =
    download.size > 0
      ? Math.min(100, Math.round((download.received / download.size) * 100))
      : 0;

  return (
    <div className="flex items-center gap-2 border-b border-[var(--color-line)]
      bg-[var(--color-bg-1)] px-3 py-2 text-[11.5px]">
      {download.finished ? (
        download.ok ? (
          <CheckCircle2 size={13} className="text-[var(--color-ok)]" />
        ) : (
          <XCircle size={13} className="text-[var(--color-danger)]" />
        )
      ) : (
        <Download size={13} className="text-[var(--color-accent)]" />
      )}

      <span className="min-w-0 flex-1 truncate">
        {download.finished
          ? download.ok
            ? `${download.name} indirildi ve doğrulandı`
            : `İndirme başarısız: ${download.detail}`
          : `${download.name} indiriliyor %${percent}`}
      </span>

      {!download.finished && (
        <span className="h-1.5 w-32 overflow-hidden rounded bg-[var(--color-bg-2)]">
          <span
            className="block h-full bg-[var(--color-accent)] transition-[width]"
            style={{ width: `${percent}%` }}
          />
        </span>
      )}
    </div>
  );
}

/** Yolun üst klasörü; kök seviyesinde boş döner (kök listesine gider). */
function parentOf(path: string): string {
  if (!path) return "";
  const sep = path.includes("\\") ? "\\" : "/";
  const trimmed = path.replace(/[/\\]+$/, "");
  const cut = trimmed.lastIndexOf(sep);
  // Kökün kendisine ulaşıldıysa kök listesine dön.
  if (cut <= 0) return "";
  return trimmed.slice(0, cut);
}

/** İki parçayı hedefin ayırıcısıyla birleştirir. */
function joinPath(dir: string, name: string): string {
  const sep = dir.includes("\\") ? "\\" : "/";
  return `${dir.replace(/[/\\]+$/, "")}${sep}${name}`;
}

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1_048_576) return `${(bytes / 1024).toFixed(0)} KB`;
  return `${(bytes / 1_048_576).toFixed(1)} MB`;
}
