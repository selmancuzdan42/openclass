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

import { open as openFolderDialog, open as openFileDialog } from "@tauri-apps/plugin-dialog";

import { api, formatTime } from "./api";
import { Button, Empty, Modal, useToast } from "./ui";
import type { BrowseEntry, BrowseRoot, BrowseView, FileDestination } from "./types";

/**
 * Uzak dosya gezgini — NetSupport tarzı **tek pencere**.
 *
 * Solda kök konumlar (Masaüstü/Belgeler/İndirilenler) kalıcı kısayollar
 * olarak durur; tek tıkla o köke gidilir. Sağda gezinti alanı vardır:
 * klasörler tıklanınca açılır, dosyalar listelenir.
 *
 * Klasörlerin satırında **Buraya Gönder**, dosyaların satırında **İndir**
 * butonu bulunur. Gönderim iki ayrı adımda gezdirmez: gezilen klasör hedeftir,
 * yerel dosya seçimi (işletim sistemi penceresi) gezgin açıkken yapılır,
 * seçilince gönderim başlar.
 *
 * Burada bir yol yazılamaz, yalnızca öğrencinin açtığı köklerde gezilir;
 * kapsam kararını öğrenci tarafı verir. Yazma, silme ve yeniden adlandırma
 * protokolde tanımlı değildir.
 */
export function BrowserDialog({
  open,
  targetIds,
  onSendToFolder,
  onClose,
}: {
  open: boolean;
  /** "Buraya Gönder"de dosyanın gönderileceği cihaz kimlikleri. Boşsa gönderim gizlenir. */
  targetIds: string[];
  /** Bir gönderim başarıyla başladığında üst kata haber verilir (pencere kapatılır, durum gösterilir). */
  onSendToFolder: (destination: FileDestination, subpath: string | null, shown: string) => void;
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
   * (`C:\Users\ali\Belgeler`), bu yüzden mutlak yol taşınamaz. Kök etiketi
   * kapalı bir kümeye (`FileDestination`) eşlenir, kalan kısım göreli yol
   * olarak gider. `OpenClass` kökü Belgeler'in altında olduğundan
   * kendiliğinden `documents` + `OpenClass` olarak çözülür.
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

  /** Şu an gezilen klasörü hedef yapıp yerel dosya seçtirir ve gönderir. */
  async function sendHere() {
    if (!view) return;
    const hedef = resolveTarget(view.path, view.roots);
    if (!hedef) {
      show("error", "Bu konuma gönderilemez.");
      return;
    }
    if (targetIds.length === 0) {
      show("error", "Dosya gönderilecek bağlı cihaz yok.");
      return;
    }
    // Gezgin (modal) açık kalır; yerel seçim tek sistem penceresiyle üstte yapılır.
    const picked = await openFileDialog({
      multiple: false,
      directory: false,
      title: "Öğrencilere gönderilecek dosyayı seçin",
    });
    if (typeof picked !== "string") return; // vazgeçildi

    try {
      await api.sendFile(targetIds, picked, hedef.destination, hedef.subpath);
      onSendToFolder(hedef.destination, hedef.subpath, view.path);
    } catch (err) {
      show("error", err instanceof Error ? err.message : String(err));
    }
  }

  const atRoots = !view?.path;
  const parent = parentOf(view?.path ?? "");
  const activeRoot = view?.path
    ? view.roots.find(
        (r) =>
          view.path === r.path ||
          view.path.startsWith(r.path + "\\") ||
          view.path.startsWith(r.path + "/"),
      )?.path ?? ""
    : "";
  const canSend = targetIds.length > 0 && !!view?.path;

  return (
    <Modal
      open={open}
      width="xl"
      title={view ? `${view.hostname} — dosyalar` : "Dosya gezgini"}
      onClose={onClose}
      footer={
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
      }
    >
      {!view ? (
        <Empty title="Gezgin kapalı" detail="Bir cihaz seçip gezgini açın." />
      ) : (
        <div className="flex flex-col">
          {/* Üst araç çubuğu: yol + Buraya Gönder */}
          <div className="flex items-center gap-2 border-b border-[var(--color-line)]
            bg-[var(--color-bg-1)] px-3 py-2">
            <span className="min-w-0 flex-1 truncate font-mono text-[11.5px]
              text-[var(--color-text-2)]">
              {view.path || "Paylaşılan konumlar"}
            </span>
            {view.loading && (
              <span className="text-[11px] text-[var(--color-text-3)]">yükleniyor…</span>
            )}
            <Button
              tone="accent"
              disabled={!canSend}
              title={
                atRoots
                  ? "Bir klasöre gidin"
                  : targetIds.length === 0
                    ? "Gönderilecek cihaz yok"
                    : "Şu an gezilen klasöre dosya gönder"
              }
              onClick={() => void sendHere()}
            >
              <FileUp size={13} /> Buraya Gönder
            </Button>
          </div>

          {view.error && (
            <p className="border-b border-[var(--color-line)] bg-[var(--color-danger)]/10
              px-3 py-2 text-[11.5px] text-[var(--color-danger)]">
              {view.error}
            </p>
          )}

          {view.download && <DownloadBar download={view.download} />}

          <div className="flex min-h-0">
            {/* Sol: kök kısayolları + yukarı çık */}
            <div className="flex w-[180px] shrink-0 flex-col border-r border-[var(--color-line)]">
              <p className="px-3 pt-2 text-[10.5px] font-semibold uppercase
                tracking-wide text-[var(--color-text-3)]">
                Konumlar
              </p>
              <div className="scroll-thin min-h-0 flex-1 overflow-y-auto pb-1">
                {view.roots.length === 0 ? (
                  <p className="px-3 py-2 text-[11px] text-[var(--color-text-3)]">
                    Erişilebilir konum yok
                  </p>
                ) : (
                  view.roots.map((r) => {
                    const active = r.path === activeRoot;
                    return (
                      <button
                        key={r.path}
                        onClick={() => void api.browseList(r.path)}
                        title={r.path}
                        className={`flex w-full items-center gap-2 px-3 py-1.5 text-left
                          text-[12px] transition-colors ${
                            active
                              ? "bg-[var(--color-accent-soft)] font-medium text-[var(--color-accent)]"
                              : "text-[var(--color-text-2)] hover:bg-[var(--color-bg-2)]"
                          }`}
                      >
                        <HardDrive size={14} className="shrink-0" />
                        <span className="truncate">{r.label}</span>
                      </button>
                    );
                  })
                )}
              </div>

              <div className="border-t border-[var(--color-line)] p-2">
                <Button
                  disabled={atRoots}
                  title={atRoots ? "En üst seviyedesiniz" : "Üst klasör"}
                  className="w-full justify-start"
                  onClick={() => void api.browseList(parent)}
                >
                  <ArrowUp size={13} /> Yukarı Çık
                </Button>
              </div>
            </div>

            {/* Sağ: gezinti içeriği */}
            <div className="max-h-[56vh] min-w-0 flex-1 overflow-y-auto">
              {atRoots ? (
                view.roots.length === 0 ? (
                  <Empty
                    title="Paylaşılan konum yok"
                    detail="Öğrencide erişilebilir bir klasör bulunamadı."
                  />
                ) : (
                  <div className="p-3">
                    <span className="text-[11.5px] text-[var(--color-text-3)]">
                      İçeriye girmek için soldan bir konum seçin veya bir klasörü açın.
                    </span>
                  </div>
                )
              ) : view.entries.length === 0 ? (
                <Empty title="Klasör boş" detail="Burada gösterilecek bir şey yok." />
              ) : (
                <table className="w-full">
                  <tbody>
                    {view.entries.map((e) => (
                      <Row
                        key={e.name}
                        entry={e}
                        dir={view.path}
                        busy={!!view.download}
                        canSend={canSend}
                        onSendFolder={() => void sendHere()}
                      />
                    ))}
                  </tbody>
                </table>
              )}
            </div>
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
  canSend,
  onSendFolder,
}: {
  entry: BrowseEntry;
  dir: string;
  busy: boolean;
  canSend: boolean;
  onSendFolder: () => void;
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
      <td className="max-w-[36vw] py-1.5 pl-2 text-[12.5px]">
        {entry.isDir ? (
          <button
            onClick={() => void api.browseList(path)}
            className="max-w-full truncate text-left hover:underline"
          >
            {entry.name}
          </button>
        ) : (
          <span className="truncate">{entry.name}</span>
        )}
      </td>
      <td className="w-24 pr-2 text-right text-[11px] text-[var(--color-text-3)]">
        {entry.isDir ? "—" : formatSize(entry.size)}
      </td>
      <td className="w-32 pr-2 text-right text-[11px] text-[var(--color-text-3)]">
        {entry.modified > 0 ? formatTime(entry.modified) : "—"}
      </td>
      <td className="w-36 pr-3 text-right">
        {entry.isDir ? (
          canSend && (
            <Button
              tone="subtle"
              className="!h-[22px] !px-1.5 text-[11.5px]"
              title="Şu an gezilen bu klasöre dosya gönder"
              onClick={onSendFolder}
            >
              <FileUp size={12} /> Buraya Gönder
            </Button>
          )
        ) : (
          <button
            disabled={busy}
            title={busy ? "Başka bir indirme sürüyor" : "Bu dosyayı indir"}
            onClick={() => void api.browseFetch(path)}
            className="inline-flex items-center gap-1 rounded px-1.5 py-0.5 text-[11.5px]
              text-[var(--color-text-2)] hover:bg-[var(--color-bg-3)] hover:text-[var(--color-text)]
              disabled:opacity-40"
          >
            <Download size={12} /> İndir
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
