/** Öğrenci arayüzünün yapı taşları — öğretmen konsoluyla aynı tasarım dili. */
import { createContext, useCallback, useContext, useMemo, useState } from "react";
import type { ReactNode } from "react";
import { AlertTriangle, CheckCircle2, Info } from "lucide-react";

type ButtonTone = "default" | "accent" | "danger" | "subtle";

const TONE: Record<ButtonTone, string> = {
  default:
    "bg-[var(--color-surface)] border-[var(--color-stroke-strong)] text-[var(--color-text)] hover:bg-[var(--color-surface-3)]",
  accent:
    "bg-[var(--color-accent)] border-[var(--color-accent)] text-white hover:bg-[var(--color-accent-hover)] hover:border-[var(--color-accent-hover)]",
  danger: "bg-[var(--color-danger)] border-[var(--color-danger)] text-white hover:brightness-110",
  subtle: "bg-transparent border-transparent text-[var(--color-text-2)] hover:bg-[var(--color-surface-3)]",
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
      className={`inline-flex h-[30px] items-center justify-center gap-1.5 rounded border px-3
        text-[12px] font-medium transition-colors
        disabled:cursor-not-allowed disabled:opacity-45 ${TONE[tone]} ${className}`}
    >
      {children}
    </button>
  );
}

export function IconButton({
  label,
  className = "",
  children,
  ...rest
}: { label: string } & React.ButtonHTMLAttributes<HTMLButtonElement>) {
  return (
    <button
      type="button"
      title={label}
      aria-label={label}
      {...rest}
      className={`inline-flex size-[26px] shrink-0 items-center justify-center rounded
        text-[var(--color-text-2)] transition-colors
        hover:bg-[var(--color-surface-3)] hover:text-[var(--color-text)] ${className}`}
    >
      {children}
    </button>
  );
}

export function Card({
  title,
  className = "",
  children,
}: {
  title?: string;
  className?: string;
  children: ReactNode;
}) {
  return (
    <section
      className={`hairline rounded-md border border-[var(--color-stroke)]
        bg-[var(--color-surface)] ${className}`}
    >
      {title && (
        <header className="border-b border-[var(--color-stroke)] px-3 py-1.5">
          <h2 className="text-[11px] font-semibold uppercase tracking-wide text-[var(--color-text-3)]">
            {title}
          </h2>
        </header>
      )}
      {children}
    </section>
  );
}

export function TextInput({
  className = "",
  ...rest
}: React.InputHTMLAttributes<HTMLInputElement>) {
  return (
    <input
      {...rest}
      className={`h-[30px] rounded border border-[var(--color-stroke-strong)]
        bg-[var(--color-surface)] px-2.5 text-[12px] text-[var(--color-text)]
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

type ToastKind = "info" | "success" | "error";
interface Toast {
  id: number;
  kind: ToastKind;
  text: string;
}

const ToastContext = createContext<{
  show: (kind: ToastKind, text: string) => void;
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
      <div className="pointer-events-none fixed inset-x-3 bottom-3 z-50 flex flex-col gap-1.5">
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
                    : "border-[var(--color-stroke-strong)] bg-[var(--color-surface)]"
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
