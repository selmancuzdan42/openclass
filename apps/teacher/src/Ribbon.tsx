/**
 * Komut şeridi.
 *
 * NetSupport'un öğretmen konsolu komutları bir şeritte sekmelere ve adlandırılmış
 * gruplara ayırır; öğretmen ders sırasında aradığı komutu ezberden bulur. Aynı
 * bilgi mimarisi burada da kullanılır, görsel dil ise moderndir: büyük ikon +
 * küçük etiket, gruplar arasında ince ayırıcı, grup başlığı altta.
 */
import type { ReactNode } from "react";

export interface RibbonCommand {
  label: string;
  icon: ReactNode;
  onClick: () => void;
  disabled?: boolean;
  /** Devrede olduğunu göstermek için (ör. yayın açık). */
  active?: boolean;
  danger?: boolean;
  title?: string;
}

export interface RibbonGroup {
  caption: string;
  commands: RibbonCommand[];
}

export interface RibbonTab {
  key: string;
  label: string;
  groups: RibbonGroup[];
}

export function Ribbon({
  tabs,
  activeTab,
  onTabChange,
}: {
  tabs: RibbonTab[];
  activeTab: string;
  onTabChange: (key: string) => void;
}) {
  const current = tabs.find((t) => t.key === activeTab) ?? tabs[0];

  return (
    <div className="shrink-0 border-b border-[var(--color-stroke)] bg-[var(--color-surface)]">
      {/* Sekme şeridi */}
      <div role="tablist" className="flex h-[30px] items-end gap-0.5 px-2">
        {tabs.map((tab) => {
          const active = tab.key === current.key;
          return (
            <button
              key={tab.key}
              role="tab"
              aria-selected={active}
              onClick={() => onTabChange(tab.key)}
              className={`relative h-[26px] rounded-t px-3 text-[12px] transition-colors
                ${
                  active
                    ? "font-semibold text-[var(--color-accent)]"
                    : "text-[var(--color-text-2)] hover:bg-[var(--color-surface-3)]"
                }`}
            >
              {tab.label}
              {active && (
                <span className="absolute inset-x-2 -bottom-px h-[2px] rounded-full bg-[var(--color-accent)]" />
              )}
            </button>
          );
        })}
      </div>

      {/* Komut alanı */}
      <div className="flex h-[74px] items-stretch gap-0 overflow-x-auto border-t
        border-[var(--color-stroke)] bg-[var(--color-surface-2)] px-1">
        {current.groups.map((group, i) => (
          <div key={group.caption} className="flex items-stretch">
            {i > 0 && <div className="my-2 w-px shrink-0 bg-[var(--color-stroke)]" />}
            <div className="flex flex-col px-1.5 pb-0.5 pt-1.5">
              <div className="flex flex-1 items-start gap-0.5">
                {group.commands.map((cmd) => (
                  <CommandButton key={cmd.label} {...cmd} />
                ))}
              </div>
              <span className="pt-0.5 text-center text-[10px] text-[var(--color-text-3)]">
                {group.caption}
              </span>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

function CommandButton({ label, icon, onClick, disabled, active, danger, title }: RibbonCommand) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      title={title ?? label}
      className={`flex h-[50px] w-[66px] flex-col items-center justify-center gap-1 rounded
        px-1 text-[10.5px] leading-tight transition-colors
        disabled:cursor-not-allowed disabled:opacity-35
        ${
          active
            ? "bg-[var(--color-accent-soft)] text-[var(--color-accent)]"
            : danger
              ? "text-[var(--color-danger)] hover:bg-[var(--color-danger)]/10"
              : "text-[var(--color-text)] hover:bg-[var(--color-surface-3)]"
        }`}
    >
      <span className="flex h-5 items-center">{icon}</span>
      <span className="line-clamp-2 text-center">{label}</span>
    </button>
  );
}
