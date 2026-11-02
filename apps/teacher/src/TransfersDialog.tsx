import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { CheckCircle2, FileUp, Trash2, XCircle } from "lucide-react";

import { api } from "./api";
import { Button, Empty, Modal } from "./ui";
import type { DeviceView, TargetState, TransferView } from "./types";

/**
 * Dosya gönderimlerinin durum penceresi.
 *
 * Her alıcı ayrı satırda görünür: biri reddettiyse veya özet doğrulanmadıysa
 * öğretmen bunu görmeli. "Gönderildi" demek yeterli değil — dosyanın karşıya
 * sağlam ulaştığı alıcının onayıyla bilinir.
 */
export function TransfersDialog({
  open,
  devices,
  onClose,
}: {
  open: boolean;
  devices: DeviceView[];
  onClose: () => void;
}) {
  const [transfers, setTransfers] = useState<TransferView[]>([]);

  useEffect(() => {
    if (!open) return;
    void api.getTransfers().then(setTransfers).catch(() => {});
    const unlisten = listen<TransferView[]>("transfers-changed", (e) =>
      setTransfers(e.payload),
    );
    return () => {
      void unlisten.then((f) => f());
    };
  }, [open]);

  const name = (id: string) =>
    devices.find((d) => d.deviceId === id)?.hostname ?? id.slice(0, 8);

  const anyFinished = transfers.some((t) => t.finished);

  return (
    <Modal
      open={open}
      width="xl"
      title="Dosya gönderimleri"
      onClose={onClose}
      footer={
        <>
          <Button
            disabled={!anyFinished}
            onClick={() => void api.clearFinishedTransfers()}
          >
            <Trash2 size={13} /> Bitenleri Temizle
          </Button>
          <Button onClick={onClose}>Kapat</Button>
        </>
      }
    >
      {transfers.length === 0 ? (
        <Empty
          title="Gönderim yok"
          detail="Sınıf şeridindeki “Dosya Gönder” ile başlayın."
        />
      ) : (
        <div className="divide-y divide-[var(--color-line)]">
          {transfers.map((t) => (
            <div key={t.transferId} className="p-3">
              <div className="mb-2 flex items-center gap-2">
                <FileUp size={14} className="text-[var(--color-text-3)]" />
                <span className="text-[12.5px] font-medium">{t.name}</span>
                <span className="text-[11px] text-[var(--color-text-3)]">
                  {(t.size / 1_048_576).toFixed(1)} MB
                </span>
                {t.finished && (
                  <span className="ml-auto text-[11px] text-[var(--color-text-3)]">
                    bitti
                  </span>
                )}
              </div>

              <div className="space-y-1">
                {Object.entries(t.targets).map(([deviceId, st]) => (
                  <TargetRow
                    key={deviceId}
                    hostname={name(deviceId)}
                    state={st}
                    size={t.size}
                  />
                ))}
              </div>
            </div>
          ))}
        </div>
      )}
    </Modal>
  );
}

function TargetRow({
  hostname,
  state,
  size,
}: {
  hostname: string;
  state: TargetState;
  size: number;
}) {
  const percent =
    state.state === "sending" && size > 0
      ? Math.min(100, Math.round((state.sent / size) * 100))
      : state.state === "done"
        ? 100
        : 0;

  return (
    <div className="flex items-center gap-2 text-[11.5px]">
      <span className="w-28 shrink-0 truncate font-mono">{hostname}</span>

      <span className="h-1.5 min-w-0 flex-1 overflow-hidden rounded bg-[var(--color-bg-2)]">
        <span
          className={`block h-full transition-[width] ${
            state.state === "failed" || state.state === "rejected"
              ? "bg-[var(--color-danger)]"
              : state.state === "done"
                ? "bg-[var(--color-ok)]"
                : "bg-[var(--color-accent)]"
          }`}
          style={{
            width: `${state.state === "failed" || state.state === "rejected" ? 100 : percent}%`,
          }}
        />
      </span>

      <span className="w-52 shrink-0 truncate text-right text-[var(--color-text-3)]">
        {label(state, percent)}
      </span>

      <span className="w-4 shrink-0">
        {state.state === "done" && (
          <CheckCircle2 size={13} className="text-[var(--color-ok)]" />
        )}
        {(state.state === "failed" || state.state === "rejected") && (
          <XCircle size={13} className="text-[var(--color-danger)]" />
        )}
      </span>
    </div>
  );
}

function label(state: TargetState, percent: number): string {
  switch (state.state) {
    case "offered":
      return "cevap bekleniyor…";
    case "sending":
      return `gönderiliyor %${percent}`;
    case "rejected":
      return `reddetti: ${state.reason}`;
    case "done":
      return "alındı ve doğrulandı";
    case "failed":
      return state.reason;
  }
}
