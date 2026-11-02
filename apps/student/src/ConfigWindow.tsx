import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  Cast,
  Link2,
  Monitor,
  Moon,
  Power,
  RefreshCw,
  ShieldCheck,
  Sun,
  Unlink,
} from "lucide-react";

import { api } from "./api";
import { Button, Card, Field, IconButton, TextInput, useToast } from "./components";
import { useTheme } from "./useTheme";
import type {
  AutostartState,
  ConnectionStatus,
  DiscoveredTeacher,
  StatusSnapshot,
} from "./types";

/**
 * Yapılandırma penceresi — **öğretmen/yönetici içindir**.
 *
 * Öğrenci bu pencereyi normal kullanımda görmez: ajan tepside çalışır, pencere
 * gizlidir. Burada yalnızca kurulum ayarları bulunur; durum göstergeleri de
 * ayarın işe yarayıp yaramadığını doğrulamak için vardır.
 *
 * Ekran paylaşımı göstergesi ve yardım talebi bilinçli olarak burada **değil**,
 * sistem tepsisindedir (bkz. `src-tauri/src/tray.rs`).
 */
export function ConfigWindow() {
  const { theme, toggle } = useTheme();
  const { guard, show } = useToast();
  const [snapshot, setSnapshot] = useState<StatusSnapshot | null>(null);

  const refresh = useCallback(async () => {
    try {
      setSnapshot(await api.getStatus());
    } catch {
      /* açılışta kısa süre başarısız olabilir */
    }
  }, []);

  useEffect(() => {
    void refresh();
    const id = window.setInterval(() => void refresh(), 1000);
    return () => window.clearInterval(id);
  }, [refresh]);

  useEffect(() => {
    const unlisten = [
      listen("status-changed", () => void refresh()),
      listen("sharing-changed", () => void refresh()),
      listen("paired", () => {
        show("success", "Öğretmenle eşleşildi.");
        void refresh();
      }),
      listen<string>("network-warning", (e) => show("error", e.payload)),
    ];
    return () => {
      void Promise.all(unlisten).then((fns) => fns.forEach((f) => f()));
    };
  }, [refresh, show]);

  if (!snapshot) {
    return (
      <div className="flex h-full items-center justify-center text-[12px] text-[var(--color-text-3)]">
        Başlatılıyor…
      </div>
    );
  }

  const paired = snapshot.status.kind !== "notPaired";

  return (
    <div className="flex h-full flex-col bg-[var(--color-canvas)]">
      <header className="flex h-[40px] shrink-0 items-center gap-2 border-b
        border-[var(--color-stroke)] bg-[var(--color-chrome)] px-3">
        <Cast size={16} className="text-[var(--color-accent)]" />
        <div className="min-w-0">
          <p className="truncate text-[12.5px] font-semibold leading-tight">
            OpenClass Öğrenci — Yapılandırma
          </p>
          <p className="truncate text-[11px] leading-tight text-[var(--color-text-2)]">
            Bu pencere öğretmen/yönetici içindir.
          </p>
        </div>
        <IconButton label="Temayı değiştir" onClick={toggle} className="ml-auto">
          {theme === "dark" ? <Sun size={15} /> : <Moon size={15} />}
        </IconButton>
      </header>

      <main className="min-h-0 flex-1 space-y-2 overflow-auto p-2.5">
        <DeviceCard snapshot={snapshot} />
        <AutostartCard />
        <StatusCard snapshot={snapshot} onRefresh={refresh} />
        {paired ? (
          <PairedCard snapshot={snapshot} onChanged={refresh} />
        ) : (
          <PairingCard onPaired={refresh} />
        )}
      </main>

      <footer className="flex shrink-0 flex-col gap-2 border-t border-[var(--color-stroke)]
        bg-[var(--color-surface)] px-3 py-2">
        <p className="text-[10.5px] leading-snug text-[var(--color-text-3)]">
          Bu pencere kapatıldığında ajan sistem tepsisinde çalışmaya devam eder.
          Öğrenci durumu ve yardım talebini oradan görür.
        </p>
        <div className="flex items-center gap-2">
          <Button
            tone="subtle"
            className="!text-[var(--color-danger)]"
            onClick={async () => {
              await guard(api.quitAgent());
            }}
          >
            <Power size={13} /> Ajanı kapat
          </Button>
          <Button
            tone="accent"
            className="ml-auto"
            onClick={() => void guard(api.hideConfigWindow())}
          >
            Kapat
          </Button>
        </div>
      </footer>
    </div>
  );
}

// ------------------------------------------------------------ otomatik başlatma

function AutostartCard() {
  const { guard } = useToast();
  const [autostart, setAutostart] = useState<AutostartState | null>(null);

  const load = useCallback(async () => {
    try {
      setAutostart(await api.getAutostart());
    } catch {
      /* yok sayılır */
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  if (!autostart?.supported) return null;

  return (
    <Card title="Otomatik başlatma">
      <label className="flex cursor-pointer items-start gap-2.5 px-3 py-2.5">
        <input
          type="checkbox"
          checked={autostart.enabled}
          onChange={async (e) => {
            const ok = await guard(
              api.setAutostart(e.target.checked),
              e.target.checked
                ? "Windows açılışında başlayacak."
                : "Otomatik başlatma kapatıldı.",
            );
            if (ok !== undefined) await load();
          }}
          className="mt-0.5 accent-[var(--color-accent)]"
        />
        <span className="min-w-0">
          <span className="block text-[12px] font-medium">
            Windows açılışında ajanı başlat
          </span>
          <span className="block text-[11px] text-[var(--color-text-3)]">
            Girdi, kayıt defterinin standart başlangıç anahtarına uygulamanın
            kendi adıyla yazılır; gizlenmez. Yalnızca bu Windows kullanıcısı
            için geçerlidir — makine geneli ayar kurulum paketiyle yapılır.
          </span>
        </span>
      </label>
    </Card>
  );
}

// -------------------------------------------------------------------- cihaz

function DeviceCard({ snapshot }: { snapshot: StatusSnapshot }) {
  return (
    <Card title="Bu bilgisayar">
      <dl className="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 px-3 py-2.5 text-[11.5px]">
        <dt className="text-[var(--color-text-3)]">Bilgisayar adı</dt>
        <dd data-selectable className="font-medium">
          {snapshot.hostname}
        </dd>
        <dt className="text-[var(--color-text-3)]">Cihaz kimliği</dt>
        <dd data-selectable className="truncate font-mono text-[10.5px]">
          {snapshot.deviceId}
        </dd>
        <dt className="text-[var(--color-text-3)]">Sürüm</dt>
        <dd className="tabular-nums">{snapshot.appVersion}</dd>
        <dt className="text-[var(--color-text-3)]">Ekran yakalama</dt>
        <dd className={snapshot.captureSupported ? "" : "text-[var(--color-warn)]"}>
          {snapshot.captureSupported ? "Destekleniyor" : "Bu sistemde desteklenmiyor"}
        </dd>
      </dl>
    </Card>
  );
}

// -------------------------------------------------------------------- durum

function StatusCard({
  snapshot,
  onRefresh,
}: {
  snapshot: StatusSnapshot;
  onRefresh: () => void;
}) {
  const { guard } = useToast();
  const { text, dot } = describeStatus(snapshot.status);

  return (
    <Card title="Bağlantı durumu">
      <div className="flex items-start gap-2.5 px-3 py-2.5">
        <span className={`mt-1 size-[9px] shrink-0 rounded-full ${dot}`} />
        <div className="min-w-0 flex-1">
          <p className="text-[12.5px] font-medium leading-tight">{text}</p>
          {snapshot.session && (
            <p className="mt-0.5 truncate text-[11px] text-[var(--color-text-2)]">
              Ders açık: {snapshot.session.className}
            </p>
          )}
          <p className="mt-0.5 flex items-center gap-1.5 text-[11px] text-[var(--color-text-3)]">
            <Monitor size={11} />
            {snapshot.sharing.active
              ? `Ekran paylaşılıyor · ${snapshot.sharing.backend ?? ""}`
              : snapshot.sharing.error
                ? snapshot.sharing.error
                : "Ekran paylaşımı kapalı"}
          </p>
        </div>
        <IconButton
          label="Yeniden bağlan"
          onClick={async () => {
            await guard(api.reconnectNow());
            onRefresh();
          }}
        >
          <RefreshCw size={14} />
        </IconButton>
      </div>
    </Card>
  );
}

function describeStatus(status: ConnectionStatus): { text: string; dot: string } {
  switch (status.kind) {
    case "notPaired":
      return { text: "Eşleştirilmedi", dot: "bg-[var(--color-text-3)]" };
    case "connecting":
      return { text: "Bağlanıyor…", dot: "bg-[var(--color-warn)]" };
    case "pairing":
      return { text: "Eşleştiriliyor…", dot: "bg-[var(--color-warn)]" };
    case "connected":
      return { text: `Bağlı — ${status.teacherName}`, dot: "bg-[var(--color-ok)]" };
    case "disconnected":
      return {
        text: status.reason ?? `Bağlantı yok · ${status.retryInSecs} sn sonra denenecek`,
        dot: "bg-[var(--color-danger)]",
      };
  }
}

// --------------------------------------------------------------- eşleşmişse

function PairedCard({
  snapshot,
  onChanged,
}: {
  snapshot: StatusSnapshot;
  onChanged: () => void;
}) {
  const { guard } = useToast();
  const [manual, setManual] = useState(snapshot.manualAddress ?? "");

  return (
    <Card title="Eşleşilen öğretmen">
      <div className="space-y-3 px-3 py-2.5">
        <dl className="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 text-[11.5px]">
          <dt className="text-[var(--color-text-3)]">Öğretmen</dt>
          <dd className="font-medium">{snapshot.pairedTeacherName ?? "—"}</dd>
          <dt className="text-[var(--color-text-3)]">Adres</dt>
          <dd data-selectable className="font-mono text-[10.5px]">
            {snapshot.pairedTeacherAddress ?? "—"}
          </dd>
        </dl>

        <Field
          label="Sabit öğretmen adresi"
          hint="Ağ keşfi çalışmıyorsa öğretmenin IP adresini buraya yazın."
        >
          <TextInput
            className="w-full"
            value={manual}
            onChange={(e) => setManual(e.target.value)}
            placeholder="192.168.1.10:7643"
          />
        </Field>

        <div className="flex gap-2">
          <Button
            onClick={async () => {
              await guard(api.setManualAddress(manual.trim() || null), "Adres kaydedildi.");
              onChanged();
            }}
          >
            Kaydet
          </Button>
          <Button
            tone="danger"
            className="ml-auto"
            onClick={async () => {
              await guard(api.unpair(), "Eşleştirme kaldırıldı.");
              onChanged();
            }}
          >
            <Unlink size={13} /> Eşleştirmeyi kaldır
          </Button>
        </div>
      </div>
    </Card>
  );
}

// ------------------------------------------------------------- eşleştirme

function PairingCard({ onPaired }: { onPaired: () => void }) {
  const { guard } = useToast();
  const [teachers, setTeachers] = useState<DiscoveredTeacher[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [manual, setManual] = useState("");
  const [code, setCode] = useState("");
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    try {
      const list = await api.listTeachers();
      setTeachers(list);
      setSelectedId((current) => current ?? list[0]?.teacherId ?? null);
    } catch {
      /* yok sayılır */
    }
  }, []);

  useEffect(() => {
    void load();
    const id = window.setInterval(() => void load(), 2000);
    return () => window.clearInterval(id);
  }, [load]);

  async function submit() {
    setBusy(true);
    const ok = await guard(
      api.pairWithTeacher({
        teacherId: manual.trim() ? undefined : (selectedId ?? undefined),
        address: manual.trim() || undefined,
        code,
      }),
    );
    setBusy(false);
    if (ok !== undefined) {
      setCode("");
      onPaired();
    }
  }

  return (
    <Card title="Öğretmen konsoluyla eşleştir">
      <div className="space-y-3 px-3 py-2.5">
        <p className="text-[11.5px] text-[var(--color-text-2)]">
          Öğretmen konsolunda <strong>Cihaz Eşleştir</strong>'e basın ve ekranda beliren
          6 haneli kodu buraya girin. Bu işlem bilgisayar başına bir kez yapılır.
        </p>

        <div>
          <p className="mb-1.5 text-[11px] font-medium text-[var(--color-text-2)]">
            Ağda bulunan öğretmenler {teachers.length === 0 && "(aranıyor…)"}
          </p>
          {teachers.length === 0 ? (
            <p className="rounded border border-dashed border-[var(--color-stroke-strong)] px-3 py-3
              text-center text-[11px] text-[var(--color-text-3)]">
              Henüz öğretmen konsolu görünmüyor. Aşağıdan adresi elle girebilirsiniz.
            </p>
          ) : (
            <div className="space-y-1">
              {teachers.map((t) => {
                const active = selectedId === t.teacherId && !manual.trim();
                return (
                  <label
                    key={t.teacherId}
                    className={`flex cursor-pointer items-start gap-2 rounded border px-2.5 py-2
                      ${
                        active
                          ? "border-[var(--color-accent-border)] bg-[var(--color-accent-soft)]"
                          : "border-[var(--color-stroke)] hover:bg-[var(--color-surface-2)]"
                      }`}
                  >
                    <input
                      type="radio"
                      name="teacher"
                      checked={active}
                      onChange={() => {
                        setSelectedId(t.teacherId);
                        setManual("");
                      }}
                      className="mt-0.5 accent-[var(--color-accent)]"
                    />
                    <span className="min-w-0 flex-1">
                      <span className="block truncate text-[12px] font-medium">{t.teacherName}</span>
                      <span className="block truncate font-mono text-[10.5px] text-[var(--color-text-3)]">
                        {t.host}:{t.controlPort}
                      </span>
                      <span className="mt-0.5 flex items-center gap-1 truncate font-mono text-[10px]
                        text-[var(--color-text-3)]">
                        <ShieldCheck size={10} />
                        {t.fingerprintHex.slice(0, 23)}…
                      </span>
                    </span>
                  </label>
                );
              })}
            </div>
          )}
        </div>

        <Field label="Veya öğretmen adresini elle girin">
          <TextInput
            className="w-full"
            value={manual}
            onChange={(e) => setManual(e.target.value)}
            placeholder="192.168.1.10"
          />
        </Field>

        <Field label="Eşleştirme kodu">
          <TextInput
            className="!h-[44px] w-full text-center font-mono !text-[24px] tracking-[0.3em]"
            value={code}
            onChange={(e) => setCode(e.target.value.replace(/\D/g, "").slice(0, 6))}
            inputMode="numeric"
            placeholder="123456"
          />
        </Field>

        <Button
          tone="accent"
          className="w-full"
          disabled={busy || code.length < 6 || (!selectedId && !manual.trim())}
          onClick={submit}
        >
          <Link2 size={14} /> {busy ? "Eşleştiriliyor…" : "Eşleştir ve Bağlan"}
        </Button>
      </div>
    </Card>
  );
}
