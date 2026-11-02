/**
 * Grup çubuğu.
 *
 * Laboratuvarlar bilgisayar adı ön ekiyle tanımlanır (`LAB1` → `LAB101`…).
 * Sekmeler, o gruptaki **bağlı** cihaz sayısını ve toplam cihaz sayısını
 * gösterir; öğretmen hangi laboratuvarın ne durumda olduğunu tek bakışta görür.
 */
import { Plus, X } from "lucide-react";

import type { DeviceGroup } from "./types";

export interface GroupTab {
  /** `null` → "Tümü" sekmesi. */
  group: DeviceGroup | null;
  total: number;
  online: number;
}

export function GroupBar({
  tabs,
  activeGroup,
  onSelect,
  onAdd,
  onRemove,
}: {
  tabs: GroupTab[];
  /** Etkin grubun adı; `null` ise "Tümü". */
  activeGroup: string | null;
  onSelect: (name: string | null) => void;
  onAdd: () => void;
  onRemove: (name: string) => void;
}) {
  return (
    <div className="flex h-[32px] shrink-0 items-center gap-0.5 overflow-x-auto border-b
      border-[var(--color-stroke)] bg-[var(--color-surface)] px-2">
      {tabs.map(({ group, total, online }) => {
        const name = group?.name ?? null;
        const active = activeGroup === name;
        return (
          <div key={name ?? "__all__"} className="group/tab relative shrink-0">
            <button
              onClick={() => onSelect(name)}
              title={group ? `${group.name} — ön ek: ${group.prefix}` : "Tüm cihazlar"}
              className={`flex h-[24px] items-center gap-1.5 rounded border px-2.5 text-[12px]
                transition-colors
                ${
                  active
                    ? "border-[var(--color-accent-border)] bg-[var(--color-accent-soft)] font-semibold text-[var(--color-accent)]"
                    : "border-transparent text-[var(--color-text-2)] hover:bg-[var(--color-surface-3)]"
                } ${group ? "pr-6" : ""}`}
            >
              <span
                className={`size-[6px] rounded-full ${
                  online > 0 ? "bg-[var(--color-ok)]" : "bg-[var(--color-text-3)]"
                }`}
              />
              {group?.name ?? "Tümü"}
              <span className="tabular-nums opacity-70">
                {online}/{total}
              </span>
            </button>
            {group && (
              <button
                onClick={() => onRemove(group.name)}
                title={`${group.name} grubunu kaldır`}
                aria-label={`${group.name} grubunu kaldır`}
                className="absolute right-1 top-1/2 hidden size-[14px] -translate-y-1/2
                  items-center justify-center rounded text-[var(--color-text-3)]
                  hover:bg-[var(--color-stroke)] hover:text-[var(--color-danger)]
                  group-hover/tab:flex"
              >
                <X size={10} />
              </button>
            )}
          </div>
        );
      })}

      <button
        onClick={onAdd}
        title="Laboratuvar grubu ekle"
        className="ml-0.5 flex h-[24px] shrink-0 items-center gap-1 rounded px-2 text-[12px]
          text-[var(--color-text-2)] transition-colors hover:bg-[var(--color-surface-3)]"
      >
        <Plus size={13} /> Grup
      </button>
    </div>
  );
}
