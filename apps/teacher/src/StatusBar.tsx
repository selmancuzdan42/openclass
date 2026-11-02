/**
 * Durum çubuğu.
 *
 * NetSupport'un durum çubuğundaki bilgi kümesini izler: içinde bulunulan sınıf,
 * bağlı cihaz sayısı, seçili cihaz sayısı ve gruptaki cihaz sayısı. Buna ek
 * olarak seçim kısayolları, görünüm anahtarları ve küçük resim boyutu
 * kaydırıcısı burada toplanır — öğretmen ders sırasında bunları en çok kullanır.
 */
import { Hand, LayoutGrid, Radio, Rows3, Wifi, WifiOff } from "lucide-react";

import { IconButton } from "./ui";
import type { BroadcastInfo } from "./types";

export type ListMode = "thumbnails" | "details";

export function StatusBar({
  className,
  connected,
  selected,
  inGroup,
  helpCount,
  broadcast,
  mode,
  onModeChange,
  thumbSize,
  onThumbSizeChange,
  onSelectOnline,
  onSelectOffline,
  onSelectHelp,
}: {
  className: string;
  connected: number;
  selected: number;
  inGroup: number;
  helpCount: number;
  /** Açık ekran yayını; taşıma yolunu göstermek için. */
  broadcast: BroadcastInfo | null;
  mode: ListMode;
  onModeChange: (m: ListMode) => void;
  thumbSize: number;
  onThumbSizeChange: (v: number) => void;
  onSelectOnline: () => void;
  onSelectOffline: () => void;
  onSelectHelp: () => void;
}) {
  return (
    <footer className="flex h-[26px] shrink-0 items-center gap-3 border-t
      border-[var(--color-stroke)] bg-[var(--color-surface)] px-2 text-[11px]
      text-[var(--color-text-2)]">
      <span className="flex items-center gap-1.5">
        <strong className="font-semibold text-[var(--color-text)]">{className}</strong>
      </span>

      {broadcast && <BroadcastBadge broadcast={broadcast} />}

      <Divider />
      <Metric label="Bağlı" value={connected} />
      <Metric label="Seçili" value={selected} />
      <Metric label="Grupta" value={inGroup} />

      <Divider />
      <span className="text-[var(--color-text-3)]">Seç:</span>
      <IconButton label="Bağlı olanları seç" onClick={onSelectOnline}>
        <Wifi size={13} />
      </IconButton>
      <IconButton label="Çevrimdışı olanları seç" onClick={onSelectOffline}>
        <WifiOff size={13} />
      </IconButton>
      <IconButton
        label="Yardım isteyenleri seç"
        onClick={onSelectHelp}
        disabled={helpCount === 0}
        className={helpCount > 0 ? "!text-[var(--color-warn)]" : ""}
      >
        <Hand size={13} />
      </IconButton>

      <div className="ml-auto flex items-center gap-3">
        {mode === "thumbnails" && (
          <label className="flex items-center gap-1.5" title="Küçük resim boyutu">
            <span className="text-[var(--color-text-3)]">Boyut</span>
            <input
              type="range"
              className="size-slider w-[90px]"
              min={150}
              max={420}
              step={10}
              value={thumbSize}
              onChange={(e) => onThumbSizeChange(Number(e.target.value))}
              aria-label="Küçük resim boyutu"
            />
          </label>
        )}

        <div className="flex items-center gap-0.5 rounded border border-[var(--color-stroke)] p-0.5">
          <IconButton
            label="Küçük resimler"
            active={mode === "thumbnails"}
            onClick={() => onModeChange("thumbnails")}
          >
            <LayoutGrid size={13} />
          </IconButton>
          <IconButton
            label="Ayrıntılı liste"
            active={mode === "details"}
            onClick={() => onModeChange("details")}
          >
            <Rows3 size={13} />
          </IconButton>
        </div>
      </div>
    </footer>
  );
}

function Metric({ label, value }: { label: string; value: number }) {
  return (
    <span>
      {label}{" "}
      <strong className="tabular-nums font-semibold text-[var(--color-text)]">{value}</strong>
    </span>
  );
}

function Divider() {
  return <span className="h-3 w-px bg-[var(--color-stroke)]" />;
}

/**
 * Yayının taşıma yolunu gösteren işaret.
 *
 * Multicast açıkken kare telden bir kez geçer. TLS yedeğine düşen cihaz
 * varsa sayısı burada görünür: laboratuvar anahtarı IGMP'yi süzüyorsa
 * öğretmen sorunu buradan fark eder.
 */
function BroadcastBadge({ broadcast }: { broadcast: BroadcastInfo }) {
  const fallback = broadcast.fallbackCount;

  return (
    <>
      <Divider />
      <span
        className="flex items-center gap-1.5"
        title={
          broadcast.multicast
            ? fallback > 0
              ? `${fallback} cihaza multicast ulaşmıyor, onlara TLS ile gönderiliyor`
              : "Kareler UDP multicast ile tek seferde gönderiliyor"
            : "Multicast kapalı; kareler her cihaza TLS ile tek tek gönderiliyor"
        }
      >
        <Radio
          size={12}
          className={
            broadcast.multicast && fallback === 0
              ? "text-[var(--color-ok)]"
              : "text-[var(--color-warn)]"
          }
        />
        <span>
          Yayın · {broadcast.multicast ? "multicast" : "TLS"}
          {broadcast.multicast && fallback > 0 && ` (${fallback} yedek)`}
        </span>
      </span>
    </>
  );
}
