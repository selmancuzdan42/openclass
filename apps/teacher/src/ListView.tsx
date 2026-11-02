/**
 * Cihaz listesi: küçük resim ızgarası ve ayrıntılı tablo.
 *
 * Küçük resim yerleşimi sınıf yönetimi yazılımlarının alışılmış düzenini izler:
 * ekran görüntüsü üstte, altında bilgisayar adını taşıyan bir şerit, seçim
 * şeridin renklenmesiyle belli olur. Çift tıklama büyütülmüş görünümü açar,
 * sağ tık cihaza özgü menüyü getirir.
 *
 * Gerçek kare gelmediği sürece **sahte görüntü gösterilmez**; bunun yerine
 * nedeni anlatan bir yer tutucu çizilir.
 */
import { Hand, Monitor, MonitorOff, ShieldAlert, WifiOff } from "lucide-react";

import { formatTime } from "./api";
import { ScreenCanvas } from "@openclass/screen-view";
import { Empty } from "./ui";
import type { Authorization, DeviceView } from "./types";

const AUTH_LABEL: Record<Authorization, string> = {
  authorized: "Yetkili",
  pending: "Yetki bekliyor",
  rejected: "Yetkisi kaldırıldı",
  unknown: "Kayıtsız",
};

const AUTH_CLASS: Record<Authorization, string> = {
  authorized: "text-[var(--color-ok)]",
  pending: "text-[var(--color-warn)]",
  rejected: "text-[var(--color-danger)]",
  unknown: "text-[var(--color-text-3)]",
};

interface CommonProps {
  devices: DeviceView[];
  selected: Set<string>;
  frameBaseUrl: string;
  onSelect: (deviceId: string, additive: boolean) => void;
  onOpen: (deviceId: string) => void;
  onContextMenu: (deviceId: string, x: number, y: number) => void;
}

// ------------------------------------------------------------ küçük resimler

export function ThumbnailGrid({
  size,
  emptyTitle,
  emptyDetail,
  ...props
}: CommonProps & { size: number; emptyTitle: string; emptyDetail?: string }) {
  if (props.devices.length === 0) {
    return <Empty title={emptyTitle} detail={emptyDetail} />;
  }

  return (
    <div
      className="grid h-full content-start gap-2 overflow-auto p-2"
      style={{ gridTemplateColumns: `repeat(auto-fill, minmax(${size}px, 1fr))` }}
    >
      {props.devices.map((device) => (
        <Thumbnail key={device.deviceId} device={device} {...props} />
      ))}
    </div>
  );
}

function Thumbnail({
  device,
  selected,
  frameBaseUrl,
  onSelect,
  onOpen,
  onContextMenu,
}: CommonProps & { device: DeviceView }) {
  const isSelected = selected.has(device.deviceId);
  const hasFrame = device.online && device.frameSeq > 0;
  const stale = hasFrame && device.frameAgeMs > 10_000;

  return (
    <div
      role="button"
      tabIndex={0}
      aria-pressed={isSelected}
      onClick={(e) => onSelect(device.deviceId, e.ctrlKey || e.metaKey || e.shiftKey)}
      onDoubleClick={() => onOpen(device.deviceId)}
      onContextMenu={(e) => {
        e.preventDefault();
        onContextMenu(device.deviceId, e.clientX, e.clientY);
      }}
      onKeyDown={(e) => {
        if (e.key === "Enter") onOpen(device.deviceId);
        if (e.key === " ") {
          e.preventDefault();
          onSelect(device.deviceId, true);
        }
      }}
      className={`hairline flex cursor-default flex-col overflow-hidden rounded border
        transition-colors
        ${
          isSelected
            ? "border-[var(--color-accent)] ring-1 ring-[var(--color-accent)]"
            : "border-[var(--color-stroke-strong)] hover:border-[var(--color-text-3)]"
        }`}
    >
      {/* Ekran alanı */}
      <div className="relative aspect-video w-full bg-[#101418]">
        {hasFrame ? (
          <ScreenCanvas
            deviceId={device.deviceId}
            frameBaseUrl={frameBaseUrl}
            seq={device.frameSeq}
            alt={`${device.hostname} ekranı`}
            className="size-full object-contain"
          />
        ) : (
          <Placeholder device={device} />
        )}

        {device.helpRequested && (
          <span
            title="Yardım istiyor"
            className="absolute left-1 top-1 flex items-center gap-1 rounded
              bg-[var(--color-warn)] px-1.5 py-0.5 text-[10px] font-semibold text-white"
          >
            <Hand size={10} /> Yardım
          </span>
        )}
        {stale && (
          <span className="absolute bottom-1 left-1 rounded bg-black/70 px-1 py-px text-[10px] text-amber-300">
            {Math.round(device.frameAgeMs / 1000)} sn
          </span>
        )}
        {hasFrame && (
          <span className="absolute bottom-1 right-1 rounded bg-black/55 px-1 py-px text-[10px] text-white/75">
            {device.frameWidth}×{device.frameHeight}
          </span>
        )}
      </div>

      {/* Ad şeridi */}
      <div
        className={`flex h-[26px] shrink-0 items-center gap-1.5 px-1.5
          ${
            isSelected
              ? "bg-[var(--color-accent)] text-white"
              : "bg-[var(--color-surface-2)] text-[var(--color-text)]"
          }`}
      >
        <StateDot device={device} inverted={isSelected} />
        <span className="truncate text-[11.5px] font-medium" title={device.hostname}>
          {device.hostname}
        </span>
        {device.authorization !== "authorized" ? (
          <span
            title={AUTH_LABEL[device.authorization]}
            className={`ml-auto shrink-0 rounded px-1 text-[10px] font-medium ${
              isSelected
                ? "bg-white/20 text-white"
                : device.authorization === "pending"
                  ? "bg-[var(--color-warn)]/15 text-[var(--color-warn)]"
                  : "bg-[var(--color-text-3)]/15 text-[var(--color-text-3)]"
            }`}
          >
            {device.authorization === "pending" ? "Bekliyor" : AUTH_LABEL[device.authorization]}
          </span>
        ) : (
          device.displayName && (
            <span
              className={`ml-auto truncate text-[10.5px] ${
                isSelected ? "text-white/75" : "text-[var(--color-text-3)]"
              }`}
              title={device.displayName}
            >
              {device.displayName}
            </span>
          )
        )}
      </div>
    </div>
  );
}

/** Canlı görüntü yokken nedenini anlatan alan. */
function Placeholder({ device }: { device: DeviceView }) {
  let Icon = MonitorOff;
  let text = "Ekran yayını kapalı";

  if (!device.online) {
    Icon = WifiOff;
    text = device.discovered ? "Ağda görünüyor, bağlı değil" : "Çevrimdışı";
  } else if (device.authorization !== "authorized") {
    Icon = ShieldAlert;
    text = AUTH_LABEL[device.authorization];
  } else if (device.screenError) {
    text = device.screenError;
  } else if (device.streaming) {
    Icon = Monitor;
    text = "İlk kare bekleniyor…";
  }

  return (
    <div className="flex size-full flex-col items-center justify-center gap-1 px-2 text-center
      text-[#6b7580]">
      <Icon size={20} />
      <span className="line-clamp-3 text-[10.5px] leading-snug">{text}</span>
    </div>
  );
}

function StateDot({ device, inverted }: { device: DeviceView; inverted: boolean }) {
  const color = device.online
    ? "bg-[var(--color-ok)]"
    : device.discovered
      ? "bg-[var(--color-warn)]"
      : "bg-[var(--color-text-3)]";
  const label = device.online ? "Bağlı" : device.discovered ? "Ağda görünüyor" : "Çevrimdışı";
  return (
    <span
      title={label}
      className={`size-[7px] shrink-0 rounded-full ${inverted ? "bg-white" : color}`}
    />
  );
}

// ---------------------------------------------------------------- tablo

export function DetailsTable({
  devices,
  selected,
  onSelect,
  onOpen,
  onContextMenu,
  emptyTitle,
  emptyDetail,
}: Omit<CommonProps, "frameBaseUrl"> & { emptyTitle: string; emptyDetail?: string }) {
  if (devices.length === 0) {
    return <Empty title={emptyTitle} detail={emptyDetail} />;
  }

  return (
    <div className="h-full overflow-auto">
      <table className="w-full border-collapse text-[12px]">
        <thead className="sticky top-0 z-10 bg-[var(--color-surface-2)] text-left
          text-[11px] text-[var(--color-text-2)]">
          <tr className="border-b border-[var(--color-stroke)]">
            <Th className="w-[180px]">Bilgisayar</Th>
            <Th className="w-[130px]">Oda</Th>
            <Th className="w-[150px]">Görünen ad</Th>
            <Th className="w-[110px]">Bağlantı</Th>
            <Th className="w-[130px]">Yetki</Th>
            <Th className="w-[150px]">Adres</Th>
            <Th className="w-[130px]">Yayın</Th>
            <Th>Son görülme</Th>
          </tr>
        </thead>
        <tbody>
          {devices.map((d) => {
            const isSelected = selected.has(d.deviceId);
            return (
              <tr
                key={d.deviceId}
                onClick={(e) => onSelect(d.deviceId, e.ctrlKey || e.metaKey || e.shiftKey)}
                onDoubleClick={() => onOpen(d.deviceId)}
                onContextMenu={(e) => {
                  e.preventDefault();
                  onContextMenu(d.deviceId, e.clientX, e.clientY);
                }}
                className={`cursor-default border-b border-[var(--color-stroke)]
                  ${
                    isSelected
                      ? "bg-[var(--color-accent-soft)]"
                      : "hover:bg-[var(--color-surface-2)]"
                  }`}
              >
                <Td>
                  <span className="flex items-center gap-2">
                    <StateDot device={d} inverted={false} />
                    <span className="font-medium">{d.hostname}</span>
                    {d.helpRequested && (
                      <Hand size={12} className="text-[var(--color-warn)]" aria-label="Yardım istiyor" />
                    )}
                  </span>
                </Td>
                <Td className="text-[var(--color-text-2)]">{d.room ?? "—"}</Td>
                <Td className="text-[var(--color-text-2)]">{d.displayName ?? "—"}</Td>
                <Td className="text-[var(--color-text-2)]">
                  {d.online ? "Bağlı" : d.discovered ? "Ağda" : "Çevrimdışı"}
                </Td>
                <Td className={AUTH_CLASS[d.authorization]}>{AUTH_LABEL[d.authorization]}</Td>
                <Td className="tabular-nums text-[var(--color-text-2)]">{d.address ?? "—"}</Td>
                <Td className="text-[var(--color-text-2)]">
                  {d.screenError
                    ? "Hata"
                    : d.streaming
                      ? d.frameSeq > 0
                        ? `${d.frameWidth}×${d.frameHeight}`
                        : "Başlatılıyor…"
                      : "Kapalı"}
                </Td>
                <Td className="tabular-nums text-[var(--color-text-3)]">{formatTime(d.lastSeen)}</Td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

function Th({ children, className = "" }: { children: React.ReactNode; className?: string }) {
  return <th className={`px-2.5 py-1.5 font-medium ${className}`}>{children}</th>;
}

function Td({ children, className = "" }: { children: React.ReactNode; className?: string }) {
  return <td className={`truncate px-2.5 py-[5px] ${className}`}>{children}</td>;
}
