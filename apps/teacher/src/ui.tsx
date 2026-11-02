/**
 * Arayüzün temel yapı taşları.
 *
 * Masaüstü yoğunluğu hedeflenir: küçük tipografi, dar iç boşluk, küçük köşe
 * yarıçapı. Bileşenler Windows'un Fluent diline yakın durur ama birebir
 * kopya değildir.
 */
import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState } from "react";
import type { ReactNode } from "react";
import { AlertTriangle, CheckCircle2, Info, X } from "lucide-react";

// ------------------------------------------------------------------ butonlar

type ButtonTone = "default" | "accent" | "danger" | "subtle";

const TONE: Record<ButtonTone, string> = {
  default:
    "bg-[var(--color-surface)] border-[var(--color-stroke-strong)] text-[var(--color-text)] hover:bg-[var(--color-surface-3)]",
  accent:
    "bg-[var(--color-accent)] border-[var(--color-accent)] text-white hover:bg-[var(--color-accent-hover)] hover:border-[var(--color-accent-hover)]",
  danger:
    "bg-[var(--color-danger)] border-[var(--color-danger)] text-white hover:brightness-110",
  subtle:
    "bg-transparent border-transparent text-[var(--color-text)] hover:bg-[var(--color-surface-3)]",
};

export function Button({
  tone = "default",
  className = "",
  children,
  ...rest
}: { tone?: ButtonTone } & React.ButtonHTMLAttributes<HTMLButtonElement>) {
  return (
    <button
      type="button"
      {...rest}
      className={`inline-flex h-[26px] items-center justify-center gap-1.5 rounded border px-2.5
        text-[12px] font-medium transition-colors
        disabled:cursor-not-allowed disabled:opacity-45
        ${TONE[tone]} ${className}`}
    >
      {children}
    </button>
  );
}

/** Yalnızca ikon taşıyan kare düğme (durum çubuğu, başlık çubuğu). */
export function IconButton({
  label,
  active = false,
  className = "",
  children,
  ...rest
}: { label: string; active?: boolean } & React.ButtonHTMLAttributes<HTMLButtonElement>) {
  return (
    <button
      type="button"
      title={label}
      aria-label={label}
      aria-pressed={active}
      {...rest}
      className={`inline-flex size-[22px] shrink-0 items-center justify-center rounded
        transition-colors disabled:cursor-not-allowed disabled:opacity-40
        ${
          active
            ? "bg-[var(--color-accent-soft)] text-[var(--color-accent)]"
            : "text-[var(--color-text-2)] hover:bg-[var(--color-surface-3)] hover:text-[var(--color-text)]"
        } ${className}`}
    >
      {children}
    </button>
  );
}

// ------------------------------------------------------------------- girdiler

export function TextInput({
  className = "",
  ...rest
}: React.InputHTMLAttributes<HTMLInputElement>) {
  return (
    <input
      {...rest}
      className={`h-[26px] rounded border border-[var(--color-stroke-strong)]
        bg-[var(--color-surface)] px-2 text-[12px] text-[var(--color-text)]
        placeholder:text-[var(--color-text-3)]
        focus:border-[var(--color-accent)] focus:outline-none ${className}`}
    />
  );
}

export function Field({
  label,
  hint,
  children,
}: {
  label: string;
  hint?: string;
  children: ReactNode;
}) {
  return (
    <label className="block">
      <span className="mb-1 block text-[11px] font-medium text-[var(--color-text-2)]">{label}</span>
      {children}
      {hint && <span className="mt-1 block text-[11px] text-[var(--color-text-3)]">{hint}</span>}
    </label>
  );
}

// ------------------------------------------------------------------- yüzeyler

export function Panel({
  title,
  actions,
  className = "",
  children,
}: {
  title?: string;
  actions?: ReactNode;
  className?: string;
  children: ReactNode;
}) {
  return (
    <section
      className={`hairline flex min-h-0 flex-col rounded-md border border-[var(--color-stroke)]
        bg-[var(--color-surface)] ${className}`}
    >
      {title && (
        <header className="flex h-[32px] shrink-0 items-center gap-2 border-b
          border-[var(--color-stroke)] px-3">
          <h2 className="text-[12px] font-semibold">{title}</h2>
          <div className="ml-auto flex items-center gap-1">{actions}</div>
        </header>
      )}
      <div className="min-h-0 flex-1">{children}</div>
    </section>
  );
}

export function Empty({ title, detail }: { title: string; detail?: string }) {
  return (
    <div className="flex h-full flex-col items-center justify-center gap-1 px-6 py-10 text-center">
      <p className="text-[12px] font-medium text-[var(--color-text-2)]">{title}</p>
      {detail && <p className="max-w-sm text-[11px] text-[var(--color-text-3)]">{detail}</p>}
    </div>
  );
}

// --------------------------------------------------------------------- modal

export function Modal({
  open,
  title,
  onClose,
  footer,
  width = "sm",
  children,
}: {
  open: boolean;
  title: string;
  onClose: () => void;
  footer?: ReactNode;
  width?: "sm" | "md" | "xl";
  children: ReactNode;
}) {
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, onClose]);

  if (!open) return null;

  const widths = { sm: "max-w-md", md: "max-w-2xl", xl: "max-w-[90vw]" };

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-6"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-label={title}
        className={`flex max-h-[88vh] w-full flex-col overflow-hidden rounded-md border
          border-[var(--color-stroke-strong)] bg-[var(--color-surface)] shadow-2xl ${widths[width]}`}
      >
        <header className="flex h-[36px] shrink-0 items-center justify-between border-b
          border-[var(--color-stroke)] px-3">
          <h3 className="text-[12px] font-semibold">{title}</h3>
          <IconButton label="Kapat" onClick={onClose}>
            <X size={14} />
          </IconButton>
        </header>
        <div className="min-h-0 flex-1 overflow-auto p-3">{children}</div>
        {footer && (
          <footer className="flex h-[44px] shrink-0 items-center justify-end gap-2 border-t
            border-[var(--color-stroke)] bg-[var(--color-surface-2)] px-3">
            {footer}
          </footer>
        )}
      </div>
    </div>
  );
}

// ------------------------------------------------------------- bağlam menüsü

export interface MenuItem {
  label: string;
  icon?: ReactNode;
  danger?: boolean;
  disabled?: boolean;
  onSelect: () => void;
}

/** Sağ tık menüsü. `items` içinde `null` ayırıcı çizgi üretir. */
export function ContextMenu({
  x,
  y,
  items,
  onClose,
}: {
  x: number;
  y: number;
  items: (MenuItem | null)[];
  onClose: () => void;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState({ x, y });

  // Menü ekrandan taşmasın.
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const r = el.getBoundingClientRect();
    setPos({
      x: Math.min(x, window.innerWidth - r.width - 8),
      y: Math.min(y, window.innerHeight - r.height - 8),
    });
  }, [x, y]);

  useEffect(() => {
    const close = () => onClose();
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("mousedown", close);
    window.addEventListener("resize", close);
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("mousedown", close);
      window.removeEventListener("resize", close);
      window.removeEventListener("keydown", onKey);
    };
  }, [onClose]);

  return (
    <div
      ref={ref}
      role="menu"
      style={{ left: pos.x, top: pos.y }}
      onMouseDown={(e) => e.stopPropagation()}
      className="fixed z-[60] min-w-[200px] rounded-md border border-[var(--color-stroke-strong)]
        bg-[var(--color-surface)] py-1 shadow-xl"
    >
      {items.map((item, i) =>
        item === null ? (
          <div key={`sep-${i}`} className="my-1 h-px bg-[var(--color-stroke)]" />
        ) : (
          <button
            key={item.label}
            role="menuitem"
            disabled={item.disabled}
            onClick={() => {
              item.onSelect();
              onClose();
            }}
            className={`flex w-full items-center gap-2.5 px-3 py-[5px] text-left text-[12px]
              disabled:cursor-not-allowed disabled:opacity-40
              ${
                item.danger
                  ? "text-[var(--color-danger)] hover:bg-[var(--color-danger)]/10"
                  : "text-[var(--color-text)] hover:bg-[var(--color-surface-3)]"
              }`}
          >
            <span className="flex w-4 shrink-0 justify-center">{item.icon}</span>
            {item.label}
          </button>
        ),
      )}
    </div>
  );
}

// ------------------------------------------------------------------ bildirim

type ToastKind = "info" | "success" | "error";
interface Toast {
  id: number;
  kind: ToastKind;
  text: string;
}

const ToastContext = createContext<{
  show: (kind: ToastKind, text: string) => void;
  /** Bir söz başarısız olursa hatasını bildirim olarak gösterir. */
  guard: <T>(p: Promise<T>, success?: string) => Promise<T | undefined>;
} | null>(null);

export function ToastProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<Toast[]>([]);

  const show = useCallback((kind: ToastKind, text: string) => {
    const id = Date.now() + Math.random();
    setToasts((list) => [...list, { id, kind, text }]);
    window.setTimeout(
      () => setToasts((list) => list.filter((t) => t.id !== id)),
      kind === "error" ? 8000 : 3200,
    );
  }, []);

  const guard = useCallback(
    async <T,>(p: Promise<T>, success?: string): Promise<T | undefined> => {
      try {
        const value = await p;
        if (success) show("success", success);
        return value;
      } catch (err) {
        show("error", err instanceof Error ? err.message : String(err));
        return undefined;
      }
    },
    [show],
  );

  const value = useMemo(() => ({ show, guard }), [show, guard]);

  return (
    <ToastContext.Provider value={value}>
      {children}
      <div className="pointer-events-none fixed bottom-9 right-3 z-[70] flex w-[320px] flex-col gap-1.5">
        {toasts.map((t) => (
          <div
            key={t.id}
            role="status"
            className={`pointer-events-auto flex items-start gap-2 rounded border px-2.5 py-2
              text-[12px] shadow-lg
              ${
                t.kind === "error"
                  ? "border-[var(--color-danger)] bg-[var(--color-danger)] text-white"
                  : t.kind === "success"
                    ? "border-[var(--color-ok)] bg-[var(--color-ok)] text-white"
                    : "border-[var(--color-stroke-strong)] bg-[var(--color-surface)] text-[var(--color-text)]"
              }`}
          >
            {t.kind === "error" ? (
              <AlertTriangle size={14} className="mt-px shrink-0" />
            ) : t.kind === "success" ? (
              <CheckCircle2 size={14} className="mt-px shrink-0" />
            ) : (
              <Info size={14} className="mt-px shrink-0" />
            )}
            <span className="min-w-0 break-words">{t.text}</span>
          </div>
        ))}
      </div>
    </ToastContext.Provider>
  );
}

export function useToast() {
  const ctx = useContext(ToastContext);
  if (!ctx) throw new Error("useToast, ToastProvider içinde kullanılmalı");
  return ctx;
}
