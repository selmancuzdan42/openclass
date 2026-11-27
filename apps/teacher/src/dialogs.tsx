/** Komut şeridinden açılan iletişim kutuları. */
import { useEffect, useMemo, useState } from "react";

import { api, AUDIT_LABELS, formatTime, sendCrashReport } from "./api";
import { ScreenCanvas } from "@openclass/screen-view";
import { useLiveFrame } from "./frames";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import {
  Download,
  FileText,
  FileUp,
  Folder,
  FolderOpen,
  Lock,
  MousePointerBan,
  MousePointerClick,
  ScreenShare,
  FileArchive,
  Globe,
  Monitor,
  Pencil,
  Plus,
  Camera,
  Power,
  RefreshCw,
  RotateCw,
  Trash2,
  X,
} from "lucide-react";

import { RemoteControlSurface } from "./RemoteControlSurface";
import { Button, Empty, Field, Modal, TextInput, useToast } from "./ui";
import type {
  AuditEntry,
  CollectionView,
  CollectTarget,
  CrashReport,
  DeviceGroup,
  DeviceView,
  FileDestination,
  PairingInfo,
  ServerInfo,
  UpdateInfo,
} from "./types";

// ------------------------------------------------------------- eşleştirme

export function PairingDialog({
  open,
  pairing,
  info,
  onClose,
  onRegenerate,
}: {
  open: boolean;
  pairing: PairingInfo | null;
  info: ServerInfo | null;
  onClose: () => void;
  onRegenerate: () => void;
}) {
  const expired = pairing === null || pairing.remainingSecs === 0;

  return (
    <Modal
      open={open}
      title="Cihaz eşleştirme"
      onClose={onClose}
      footer={<Button onClick={onClose}>Kapat</Button>}
    >
      <ol className="mb-3 list-inside list-decimal space-y-1 text-[12px] text-[var(--color-text-2)]">
        <li>Öğrenci bilgisayarında OpenClass Student uygulamasını açın.</li>
        <li>Listeden bu öğretmen konsolunu seçin; görünmüyorsa adresi elle girin.</li>
        <li>Aşağıdaki kodu öğrenci ekranına yazın.</li>
      </ol>

      {expired ? (
        <div className="rounded border border-dashed border-[var(--color-stroke-strong)] p-6 text-center">
          <p className="text-[12px] text-[var(--color-text-2)]">Kodun süresi doldu.</p>
          <Button tone="accent" className="mt-3" onClick={onRegenerate}>
            Yeni kod üret
          </Button>
        </div>
      ) : (
        <div className="rounded border border-[var(--color-accent-border)]
          bg-[var(--color-accent-soft)] p-5 text-center">
          <p
            data-selectable
            className="font-mono text-[38px] font-bold leading-none tracking-[0.25em]
              text-[var(--color-accent)]"
          >
            {pairing.code}
          </p>
          <p className="mt-2.5 text-[11px] text-[var(--color-text-2)]">
            {pairing.remainingSecs} saniye geçerli ·{" "}
            {pairing.maxAttempts - pairing.failedAttempts} deneme hakkı kaldı
          </p>
        </div>
      )}

      {info && (
        <div className="mt-3 rounded border border-[var(--color-stroke)] p-2.5 text-[11px]">
          <p className="mb-1 font-medium">Keşif çalışmazsa öğrenciye şu adresi verin:</p>
          <p data-selectable className="font-mono text-[var(--color-accent)]">
            {info.localAddresses.length
              ? info.localAddresses.map((a) => `${a}:${info.controlPort}`).join("   ")
              : "Yerel adres bulunamadı"}
          </p>
        </div>
      )}

      <p className="mt-3 text-[11px] text-[var(--color-text-3)]">
        Kod tek kullanımlıktır. Eşleştirme tamamlandığında cihaz kalıcı olarak tanınır;
        IP adresi değişse bile yeniden eşleştirme gerekmez.
      </p>
    </Modal>
  );
}

// ------------------------------------------------------------------ mesaj

export function MessageDialog({
  open,
  targets,
  onClose,
  onSent,
}: {
  open: boolean;
  targets: DeviceView[];
  onClose: () => void;
  onSent: () => void;
}) {
  const { guard, show } = useToast();
  const [title, setTitle] = useState("");
  const [body, setBody] = useState("");
  const [busy, setBusy] = useState(false);

  async function send() {
    if (!body.trim()) {
      show("error", "Mesaj boş olamaz.");
      return;
    }
    setBusy(true);
    const sent = await guard(api.sendNotice(targets.map((d) => d.deviceId), title, body));
    setBusy(false);
    if (sent !== undefined) {
      show("success", `${sent} cihaza gönderildi.`);
      setBody("");
      onSent();
      onClose();
    }
  }

  return (
    <Modal
      open={open}
      title="Mesaj gönder"
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>Vazgeç</Button>
          <Button tone="accent" onClick={send} disabled={busy || targets.length === 0}>
            {busy ? "Gönderiliyor…" : `Gönder (${targets.length})`}
          </Button>
        </>
      }
    >
      <p className="mb-3 text-[11px] text-[var(--color-text-2)]">
        Alıcı: {targets.length === 0 ? "bağlı cihaz yok" : targets.map((d) => d.hostname).join(", ")}
      </p>
      <div className="space-y-3">
        <Field label="Başlık (isteğe bağlı)">
          <TextInput
            className="w-full"
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            placeholder="Öğretmen"
          />
        </Field>
        <Field label="Mesaj">
          <textarea
            value={body}
            onChange={(e) => setBody(e.target.value)}
            rows={4}
            placeholder="Sayfa 42'deki alıştırmaya geçin."
            className="w-full resize-y rounded border border-[var(--color-stroke-strong)]
              bg-[var(--color-surface)] px-2 py-1.5 text-[12px]
              focus:border-[var(--color-accent)] focus:outline-none"
          />
        </Field>
      </div>
    </Modal>
  );
}

// ---------------------------------------------------------------- büyütme

export function FocusDialog({
  device,
  frameBaseUrl,
  controlled,
  onToggleControl,
  onScreenshot,
  onClose,
}: {
  device: DeviceView | null;
  frameBaseUrl: string;
  /** Bu cihaz şu anda uzaktan kontrol ediliyor mu? */
  controlled: boolean;
  onToggleControl: () => void;
  /** Görüntülenen ekranı dosyaya kaydeder. */
  onScreenshot: () => void;
  onClose: () => void;
}) {
  const frame = useLiveFrame(device);
  const ratio = frame && frame.height > 0 ? frame.width / frame.height : 16 / 9;

  return (
    <Modal
      open={device !== null}
      width="xl"
      title={device ? `${device.hostname} — büyütülmüş görünüm` : ""}
      onClose={onClose}
      footer={
        device && (
          <>
            <span className="mr-auto text-[11px] text-[var(--color-text-3)]">
              {frame?.width ?? 0}×{frame?.height ?? 0} · {device.address ?? "—"} · kare #
              {frame?.seq ?? 0}
            </span>
            {controlled && (
              <span className="mr-2 flex items-center gap-1.5 rounded bg-[var(--color-warn)]
                px-2 py-1 text-[11px] font-semibold text-white">
                <MousePointerClick size={12} />
                Kontrol sizde — fare ve klavye öğrenciye gidiyor
              </span>
            )}
            <Button
              disabled={!frame}
              title="Görüntülenen ekranı Resimler/OpenClass klasörüne kaydeder"
              onClick={onScreenshot}
            >
              <Camera size={13} /> Ekran Görüntüsü
            </Button>
            <Button
              tone={controlled ? "danger" : "accent"}
              disabled={!device.online}
              onClick={onToggleControl}
            >
              {controlled ? (
                <>
                  <MousePointerBan size={13} /> Kontrolü Bırak
                </>
              ) : (
                <>
                  <MousePointerClick size={13} /> Kontrolü Al
                </>
              )}
            </Button>
          </>
        )
      }
    >
      {device &&
        (frame ? (
          <RemoteControlSurface active={controlled} aspectRatio={ratio}>
            <ScreenCanvas
              deviceId={device.deviceId}
              frameBaseUrl={frameBaseUrl}
              seq={frame.seq}
              alt={`${device.hostname} ekranı`}
              className="max-h-[70vh] w-full bg-[#101418] object-contain"
            />
          </RemoteControlSurface>
        ) : (
          <Empty
            title="Henüz görüntü yok"
            detail={
              device.screenError ?? "Ekran yayını başlatıldı, ilk kare bekleniyor."
            }
          />
        ))}
    </Modal>
  );
}

// ---------------------------------------------------------------- ayarlar

export function SettingsDialog({
  open,
  info,
  onClose,
  onSaved,
  onChangeClass,
  onCheckUpdate,
}: {
  open: boolean;
  info: ServerInfo | null;
  onClose: () => void;
  onSaved: () => void;
  /** Sınıf seçim penceresini açar. */
  onChangeClass: () => void;
  /** Yeni sürüm olup olmadığına bakar ve sonucu bildirir. */
  onCheckUpdate: () => void;
}) {
  const { guard } = useToast();
  const [teacherName, setTeacherName] = useState("");
  const [hosts, setHosts] = useState("");
  const [askAtStartup, setAskAtStartup] = useState(true);

  useEffect(() => {
    if (!info || !open) return;
    setAskAtStartup(info.askAtStartup);
    setTeacherName(info.teacherName);
    setHosts(info.extraDiscoveryHosts.join("\n"));
  }, [info, open]);

  return (
    <Modal
      open={open}
      title="Ayarlar"
      onClose={onClose}
      footer={
        <>
          <span className="mr-auto flex items-center gap-2 text-[11px] text-[var(--color-text-3)]">
            Sürüm {info?.appVersion ?? "—"}
            <Button onClick={onCheckUpdate} title="GitHub'da yeni sürüm var mı diye bakar">
              <RefreshCw size={12} /> Güncellemeleri denetle
            </Button>
          </span>
          <Button onClick={onClose}>Vazgeç</Button>
          <Button
            tone="accent"
            onClick={async () => {
              await guard(
                api.updateSettings({
                  teacherName,
                  askAtStartup,
                  extraDiscoveryHosts: hosts.split("\n").map((h) => h.trim()).filter(Boolean),
                }),
                "Ayarlar kaydedildi.",
              );
              onSaved();
              onClose();
            }}
          >
            Kaydet
          </Button>
        </>
      }
    >
      <div className="space-y-3">
        <Field label="Öğretmen adı">
          <TextInput
            className="w-full"
            value={teacherName}
            onChange={(e) => setTeacherName(e.target.value)}
          />
        </Field>
        <div className="flex items-center gap-2 rounded bg-[var(--color-bg-2)] px-2.5 py-2">
          <span className="min-w-0 flex-1 text-[11.5px] leading-snug text-[var(--color-text-2)]">
            Yönetilen sınıf:{" "}
            <strong className="text-[var(--color-text)]">{info?.className ?? "—"}</strong>
            <span className="block text-[11px] text-[var(--color-text-3)]">
              {info?.activePrefix
                ? `Adı ${info.activePrefix} ile başlayan bilgisayarlar`
                : info?.activeRoom
                  ? `Oda adı “${info.activeRoom}” olan bilgisayarlar`
                  : "Tüm bilgisayarlar"}
            </span>
          </span>
          <Button onClick={onChangeClass}>Sınıf değiştir</Button>
        </div>
        <label className="flex items-center gap-2 rounded bg-[var(--color-bg-2)] px-2.5 py-2
          text-[11.5px] text-[var(--color-text-2)]">
          <input
            type="checkbox"
            checked={askAtStartup}
            onChange={(e) => setAskAtStartup(e.target.checked)}
          />
          Açılışta sınıf seçim penceresini göster
        </label>
        <Field
          label="Ek keşif adresleri"
          hint="Okul ağı yayın paketlerini engelliyorsa her satıra bir hedef yazın: yayın adresi (192.168.1.255), tek IP, alan adı veya aralık (192.168.1.10-40, en fazla 254 adres)."
        >
          <textarea
            value={hosts}
            onChange={(e) => setHosts(e.target.value)}
            rows={3}
            placeholder={"192.168.1.255\n192.168.2.255"}
            className="w-full resize-y rounded border border-[var(--color-stroke-strong)]
              bg-[var(--color-surface)] px-2 py-1.5 font-mono text-[11px]
              focus:border-[var(--color-accent)] focus:outline-none"
          />
        </Field>

        {info && (
          <div className="rounded border border-[var(--color-stroke)] bg-[var(--color-surface-2)] p-2.5">
            <dl className="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 text-[11px]">
              <dt className="text-[var(--color-text-3)]">Bağlantı portu</dt>
              <dd className="tabular-nums">{info.controlPort}</dd>
              <dt className="text-[var(--color-text-3)]">Keşif portu</dt>
              <dd className="tabular-nums">{info.discoveryPort}</dd>
              <dt className="text-[var(--color-text-3)]">Yerel adresler</dt>
              <dd data-selectable className="font-mono">
                {info.localAddresses.join(", ") || "—"}
              </dd>
              <dt className="text-[var(--color-text-3)]">Sertifika parmak izi</dt>
              <dd data-selectable className="break-all font-mono text-[10px]">
                {info.certFingerprint}
              </dd>
            </dl>
          </div>
        )}
      </div>
    </Modal>
  );
}

// ------------------------------------------------------------------ kayıtlar

export function AuditDialog({ open, onClose }: { open: boolean; onClose: () => void }) {
  const { guard } = useToast();
  const [entries, setEntries] = useState<AuditEntry[]>([]);

  const load = async () => {
    const list = await guard(api.getAuditLog(200));
    if (list) setEntries(list);
  };

  useEffect(() => {
    if (open) void load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);

  return (
    <Modal
      open={open}
      width="md"
      title="Denetim kayıtları"
      onClose={onClose}
      footer={
        <>
          <Button onClick={load}>
            <RefreshCw size={13} /> Yenile
          </Button>
          <Button onClick={onClose}>Kapat</Button>
        </>
      }
    >
      <p className="mb-2 text-[11px] text-[var(--color-text-3)]">
        Son 200 olay. Parola, eşleştirme kodu, anahtar ve ekran içeriği kaydedilmez.
      </p>
      {entries.length === 0 ? (
        <Empty title="Henüz kayıt yok" />
      ) : (
        <ul className="divide-y divide-[var(--color-stroke)] text-[11.5px]">
          {entries.map((e, i) => (
            <li key={`${e.at}-${i}`} className="flex gap-3 py-1">
              <span className="shrink-0 tabular-nums font-mono text-[var(--color-text-3)]">
                {formatTime(e.at)}
              </span>
              <span className="w-[180px] shrink-0 font-medium">
                {AUDIT_LABELS[e.kind] ?? e.kind}
              </span>
              <span className="truncate text-[var(--color-text-2)]">{e.detail ?? ""}</span>
            </li>
          ))}
        </ul>
      )}
    </Modal>
  );
}

// ---------------------------------------------------------------- yayın modu

/**
 * Ekran yayını başlatılırken mod seçimi.
 *
 * Penceresiz mod dikkat dağınıklığını keser ama öğrencinin not almasını da
 * engeller. Hangisinin doğru olduğu derse göre değişir, bu yüzden her
 * yayında soruluyor.
 */
export function BroadcastDialog({
  open,
  onStart,
  onClose,
}: {
  open: boolean;
  onStart: (fullscreen: boolean) => void;
  onClose: () => void;
}) {
  const [fullscreen, setFullscreen] = useState(true);

  return (
    <Modal
      open={open}
      title="Ekranımı yayınla"
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>Vazgeç</Button>
          <Button tone="accent" onClick={() => onStart(fullscreen)}>
            <ScreenShare size={13} /> Yayını Başlat
          </Button>
        </>
      }
    >
      <div className="space-y-2 p-3">
        <ModeOption
          selected={fullscreen}
          onSelect={() => setFullscreen(true)}
          title="Penceresiz (tam ekran)"
          detail="Ekranınız öğrencinin ekranını kaplar ve üstte durur.
            Anlatım sırasında dikkatin dağılmasını engeller."
        />
        <ModeOption
          selected={!fullscreen}
          onSelect={() => setFullscreen(false)}
          title="Pencereli"
          detail="Öğrenci Alt+Tab ile başka uygulamalara geçebilir —
            anlatımı izlerken not almasını istiyorsanız bunu seçin."
        />
      </div>
    </Modal>
  );
}

function ModeOption({
  selected,
  onSelect,
  title,
  detail,
}: {
  selected: boolean;
  onSelect: () => void;
  title: string;
  detail: string;
}) {
  return (
    <label
      className={`flex cursor-pointer items-start gap-2.5 rounded border p-2.5 ${
        selected
          ? "border-[var(--color-accent)] bg-[var(--color-accent)]/8"
          : "border-[var(--color-line)]"
      }`}
    >
      <input
        type="radio"
        checked={selected}
        onChange={onSelect}
        className="mt-0.5 accent-[var(--color-accent)]"
      />
      <span className="min-w-0">
        <span className="block text-[12.5px] font-medium">{title}</span>
        <span className="block text-[11.5px] leading-relaxed text-[var(--color-text-3)]">
          {detail}
        </span>
      </span>
    </label>
  );
}

// -------------------------------------------------------------- ekran kilidi

/** Öğrenci ekranlarını kilitleme iletişim kutusu. */
export function LockDialog({
  open,
  count,
  onLock,
  onClose,
}: {
  open: boolean;
  count: number;
  onLock: (message: string) => void;
  onClose: () => void;
}) {
  const [message, setMessage] = useState("");

  return (
    <Modal
      open={open}
      title={`${count} cihazın ekranını kilitle`}
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>Vazgeç</Button>
          <Button tone="danger" onClick={() => onLock(message)}>
            <Lock size={13} /> Kilitle
          </Button>
        </>
      }
    >
      <div className="space-y-3 p-3">
        <label className="block space-y-1">
          <span className="text-[11.5px] font-medium text-[var(--color-text-2)]">
            Öğrenciye gösterilecek açıklama
          </span>
          <textarea
            value={message}
            onChange={(e) => setMessage(e.target.value)}
            rows={3}
            placeholder="Örn: Tahtaya bakın, 5 dakika sonra devam edeceğiz."
            className="w-full resize-none rounded border border-[var(--color-line)]
              bg-[var(--color-bg-1)] px-2 py-1.5 text-[12.5px]
              outline-none focus:border-[var(--color-accent)]"
          />
        </label>

        <p className="rounded bg-[var(--color-bg-2)] p-2.5 text-[11.5px] leading-relaxed
          text-[var(--color-text-3)]">
          Klavye ve fare kilitlenir, ekranda nedenini açıklayan bir pencere
          görünür. Kilit konsol kapanınca, bağlantı kopunca veya bir saat sonra
          kendiliğinden kalkar. <strong>Ctrl+Alt+Del engellenmez</strong> —
          öğrenci her zaman makineyi kurtarabilir.
        </p>
      </div>
    </Modal>
  );
}

// ------------------------------------------------------------ sınıf seçimi

/** Sınıf seçim penceresinin sonucu. Oda ve ön ek ikisi de boşsa "tümü". */
export interface ClassChoice {
  name: string;
  room: string;
  prefix: string;
}

/** "Tüm bilgisayarlar" kartının listedeki anahtarı. */
const ALL_KEY = "\u0000all";

/** Kayıtlı bir sınıfın yönettiği kapsam: ön ek varsa ön ek, yoksa adı oda olarak. */
function classChoice(group: DeviceGroup): ClassChoice {
  const prefix = group.prefix.trim().toUpperCase();
  return prefix
    ? { name: group.name, room: "", prefix }
    : { name: group.name, room: group.name, prefix: "" };
}

/** Bir cihazın verilen kapsama girip girmediği (Rust'taki `scope_match` ile aynı kural). */
function inChoice(device: DeviceView, choice: ClassChoice): boolean {
  const room = choice.room.trim().toUpperCase();
  const prefix = choice.prefix.trim().toUpperCase();
  if (room && (device.room ?? "").trim().toUpperCase() !== room) return false;
  if (
    prefix &&
    !device.hostname.trim().toUpperCase().startsWith(prefix) &&
    !(device.room ?? "").trim().toUpperCase().startsWith(prefix)
  )
    return false;
  return true;
}

type ClassForm = {
  /** Düzenlenen sınıfın eski adı; yeni sınıfta `null`. */
  original: string | null;
  mode: "prefix" | "room";
  name: string;
  value: string;
};

/**
 * Açılışta hangi sınıfın yönetileceğini sorar.
 *
 * NetSupport'un Class Wizard'ının karşılığı: kayıtlı sınıflar listelenir,
 * öğretmen birini seçip başlatır. Sınıf, bilgisayar adı ön eki (Browse kipi)
 * ya da oda adı (Room kipi) ile tanımlanır ve bir kez kaydedilince sonraki
 * açılışlarda hazır durur.
 *
 * Seçim yalnızca listeyi süzmez: konsol bu kapsamı ağa duyurur ve yalnızca
 * kapsama giren öğrenciler kendiliğinden bağlanıp yetkilendirilir. Bu yüzden
 * pencereyi kapatmak bir seçim sayılmaz; "tüm bilgisayarlar" ayrı ve açık bir
 * seçenektir.
 */
export function ClassPickerDialog({
  open,
  info,
  groups,
  devices,
  onStart,
  onClose,
  onGroupsChanged,
}: {
  open: boolean;
  info: ServerInfo | null;
  groups: DeviceGroup[];
  devices: DeviceView[];
  onStart: (choice: ClassChoice, askAtStartup: boolean) => void;
  onClose: () => void;
  onGroupsChanged: () => Promise<void> | void;
}) {
  const { guard } = useToast();
  const [selected, setSelected] = useState<string | null>(null);
  const [form, setForm] = useState<ClassForm | null>(null);
  const [skipAsk, setSkipAsk] = useState(false);

  // Açılırken yürürlükteki sınıf seçili gelir; öğretmen yalnızca Enter'a basar.
  useEffect(() => {
    if (!open || !info) return;
    setForm(null);
    setSkipAsk(!info.askAtStartup);
    const room = (info.activeRoom ?? "").trim().toUpperCase();
    const prefix = (info.activePrefix ?? "").trim().toUpperCase();
    const current = groups.find((g) => {
      const c = classChoice(g);
      return c.room.trim().toUpperCase() === room && c.prefix === prefix;
    });
    if (current) setSelected(current.name);
    else if (info.setupCompleted && !room && !prefix) setSelected(ALL_KEY);
    else setSelected(groups[0]?.name ?? null);
    // Yalnızca pencere açılırken çalışır; liste değişince seçim korunur.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);

  const choice: ClassChoice | null = useMemo(() => {
    if (selected === ALL_KEY) return { name: "Tüm bilgisayarlar", room: "", prefix: "" };
    const group = groups.find((g) => g.name === selected);
    return group ? classChoice(group) : null;
  }, [selected, groups]);

  function start() {
    if (choice && !form) onStart(choice, !skipAsk);
  }

  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      // Odak bir düğmedeyse Enter o düğmeye aittir.
      if (e.key !== "Enter" || form || e.target instanceof HTMLButtonElement) return;
      start();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, form, choice, skipAsk]);

  // Öğretmen bilgisayarının adından çıkan öneri; zaten kayıtlıysa gösterilmez.
  const suggestion =
    info?.suggestedPrefix &&
    !groups.some((g) => g.prefix.trim().toUpperCase() === info.suggestedPrefix)
      ? info.suggestedPrefix
      : null;

  // Ağda görülen adlardan ve odalardan form önerileri.
  const hints = useMemo(() => {
    const prefixes = new Map<string, number>();
    const rooms = new Map<string, number>();
    for (const d of devices) {
      const oda = d.room?.trim();
      if (oda) rooms.set(oda, (rooms.get(oda) ?? 0) + 1);
      const ad = d.hostname.trim().toUpperCase();
      const rakam = ad.length - ad.replace(/\d+$/, "").length;
      for (let kalan = 0; kalan < rakam; kalan++) {
        const aday = ad.slice(0, ad.length - rakam + kalan);
        if (aday) prefixes.set(aday, (prefixes.get(aday) ?? 0) + 1);
      }
    }
    return {
      prefix: [...prefixes.entries()]
        .filter(([, n]) => n >= 2)
        .sort((a, b) => b[1] - a[1] || b[0].length - a[0].length)
        .slice(0, 6),
      room: [...rooms.entries()].sort(
        (a, b) => b[1] - a[1] || a[0].localeCompare(b[0], "tr"),
      ),
    };
  }, [devices]);

  async function saveForm() {
    if (!form) return;
    const value = form.value.trim();
    const name = form.mode === "room" ? value : form.name.trim() || value.toUpperCase();
    if (!value || !name) return;
    const prefix = form.mode === "prefix" ? value.toUpperCase() : "";
    const ok = await guard(api.saveGroup(name, prefix), `${name} sınıfı kaydedildi.`);
    if (ok === undefined) return;
    // Ad değiştiyse eski kayıt kaldırılır; kayıtlar ada göre tutulur.
    if (form.original && form.original.toUpperCase() !== name.toUpperCase()) {
      await guard(api.deleteGroup(form.original));
    }
    await onGroupsChanged();
    setSelected(name);
    setForm(null);
  }

  const card = (active: boolean) =>
    `group/card flex w-full items-center gap-2.5 rounded border px-2.5 py-2 text-left
     transition-colors ${
       active
         ? "border-[var(--color-accent)] bg-[var(--color-accent-soft)]"
         : "border-[var(--color-stroke)] hover:border-[var(--color-stroke-strong)]"
     }`;

  return (
    <Modal
      open={open}
      title="Sınıf seçin"
      onClose={onClose}
      footer={
        <>
          <label className="mr-auto flex items-center gap-1.5 text-[11px] text-[var(--color-text-2)]">
            <input
              type="checkbox"
              checked={skipAsk}
              onChange={(e) => setSkipAsk(e.target.checked)}
            />
            Açılışta sorma
          </label>
          <Button onClick={onClose}>{info?.setupCompleted ? "Vazgeç" : "Daha sonra"}</Button>
          <Button tone="accent" disabled={!choice || form !== null} onClick={start}>
            Başlat
          </Button>
        </>
      }
    >
      <div className="space-y-2.5">
        {suggestion && !form && (
          <div className="flex items-center gap-2 rounded bg-[var(--color-accent-soft)] px-2.5 py-2">
            <span className="flex-1 text-[11.5px] leading-snug text-[var(--color-text-2)]">
              Bu bilgisayarın adı <strong>{info?.hostname}</strong>. Adı{" "}
              <code>{suggestion}</code> ile başlayan bilgisayarlar için sınıf oluşturulsun mu?
            </span>
            <Button
              tone="accent"
              onClick={async () => {
                const ok = await guard(api.saveGroup(suggestion, suggestion));
                if (ok === undefined) return;
                await onGroupsChanged();
                setSelected(suggestion);
              }}
            >
              Oluştur
            </Button>
          </div>
        )}

        {!form && (
          <div className="space-y-1.5" role="listbox" aria-label="Kayıtlı sınıflar">
            {groups.map((g) => {
              const c = classChoice(g);
              const members = devices.filter((d) => inChoice(d, c));
              return (
                <div
                  key={g.name}
                  role="option"
                  aria-selected={selected === g.name}
                  tabIndex={0}
                  className={`${card(selected === g.name)} cursor-default`}
                  onClick={() => setSelected(g.name)}
                  onDoubleClick={() => onStart(c, !skipAsk)}
                >
                  <Monitor size={16} className="shrink-0 text-[var(--color-text-3)]" />
                  <span className="min-w-0 flex-1">
                    <span className="block truncate text-[12px] font-medium">{g.name}</span>
                    <span className="block truncate text-[11px] text-[var(--color-text-3)]">
                      {c.prefix ? (
                        <>
                          Adı <code>{c.prefix}</code> ile başlayanlar
                        </>
                      ) : (
                        <>Oda adı “{c.room}” olanlar</>
                      )}
                      {" · "}
                      {members.filter((d) => d.online).length}/{members.length} bağlı
                    </span>
                  </span>
                  <button
                    type="button"
                    title="Düzenle"
                    aria-label={`${g.name} sınıfını düzenle`}
                    className="rounded p-1 text-[var(--color-text-3)] hover:bg-[var(--color-surface-3)]
                      hover:text-[var(--color-text)]"
                    onClick={(e) => {
                      e.stopPropagation();
                      setForm({
                        original: g.name,
                        mode: c.prefix ? "prefix" : "room",
                        name: g.name,
                        value: c.prefix || c.room,
                      });
                    }}
                  >
                    <Pencil size={13} />
                  </button>
                  <button
                    type="button"
                    title="Sil"
                    aria-label={`${g.name} sınıfını sil`}
                    className="rounded p-1 text-[var(--color-text-3)] hover:bg-[var(--color-surface-3)]
                      hover:text-[var(--color-danger)]"
                    onClick={async (e) => {
                      e.stopPropagation();
                      const ok = await guard(api.deleteGroup(g.name), `${g.name} sınıfı silindi.`);
                      if (ok === undefined) return;
                      if (selected === g.name) setSelected(null);
                      await onGroupsChanged();
                    }}
                  >
                    <Trash2 size={13} />
                  </button>
                </div>
              );
            })}

            <div
              role="option"
              aria-selected={selected === ALL_KEY}
              tabIndex={0}
              className={`${card(selected === ALL_KEY)} cursor-default`}
              onClick={() => setSelected(ALL_KEY)}
              onDoubleClick={() =>
                onStart({ name: "Tüm bilgisayarlar", room: "", prefix: "" }, !skipAsk)
              }
            >
              <Globe size={16} className="shrink-0 text-[var(--color-text-3)]" />
              <span className="min-w-0 flex-1">
                <span className="block text-[12px] font-medium">Tüm bilgisayarlar</span>
                <span className="block text-[11px] leading-snug text-[var(--color-text-3)]">
                  Ağdaki her öğrenci bilgisayarı kodsuz kabul edilir. Aynı ağda
                  birden çok öğretmen varsa bir sınıf seçin.
                </span>
              </span>
            </div>

            <Button
              onClick={() => setForm({ original: null, mode: "prefix", name: "", value: "" })}
            >
              <Plus size={13} /> Yeni sınıf
            </Button>
          </div>
        )}

        {form && (
          <div className="space-y-2.5">
            <div role="tablist" className="flex gap-1 rounded bg-[var(--color-bg-2)] p-0.5">
              {(
                [
                  ["prefix", "Bilgisayar adına göre"],
                  ["room", "Oda adına göre"],
                ] as const
              ).map(([key, label]) => (
                <button
                  key={key}
                  role="tab"
                  aria-selected={form.mode === key}
                  onClick={() => setForm({ ...form, mode: key, value: "" })}
                  className={`flex-1 rounded px-2 py-1.5 text-[11.5px] transition-colors ${
                    form.mode === key
                      ? "bg-[var(--color-bg-1)] font-medium text-[var(--color-text)] shadow-sm"
                      : "text-[var(--color-text-2)] hover:text-[var(--color-text)]"
                  }`}
                >
                  {label}
                </button>
              ))}
            </div>

            {form.mode === "prefix" ? (
              <>
                <Field
                  label="Bilgisayar adı başlangıcı"
                  hint="LAB1 yazarsanız LAB101, LAB102 … bilgisayarları bu sınıfa girer. Büyük/küçük harf fark etmez."
                >
                  <TextInput
                    className="w-full font-mono"
                    value={form.value}
                    autoFocus
                    placeholder="LAB1"
                    onChange={(e) => setForm({ ...form, value: e.target.value.toUpperCase() })}
                  />
                </Field>
                <Field label="Sınıf adı" hint="Boş bırakılırsa ad başlangıcı kullanılır.">
                  <TextInput
                    className="w-full"
                    value={form.name}
                    placeholder={form.value || "Lab 1"}
                    onChange={(e) => setForm({ ...form, name: e.target.value })}
                  />
                </Field>
              </>
            ) : (
              <Field
                label="Oda adı"
                hint="Öğrenci bilgisayarlarında kurulumda girilen laboratuvar adı. Sınıf bu adla kaydedilir."
              >
                <TextInput
                  className="w-full"
                  value={form.value}
                  autoFocus
                  placeholder="Lab 1"
                  onChange={(e) => setForm({ ...form, value: e.target.value })}
                />
              </Field>
            )}

            {hints[form.mode].length > 0 && (
              <div>
                <p className="mb-1 text-[11px] text-[var(--color-text-3)]">Ağda görülenler</p>
                <div className="flex flex-wrap gap-1.5">
                  {hints[form.mode].map(([deger, adet]) => (
                    <button
                      key={deger}
                      onClick={() => setForm({ ...form, value: deger })}
                      className="rounded border border-[var(--color-stroke-strong)] px-2 py-1
                        text-[11px] hover:border-[var(--color-accent)]
                        hover:text-[var(--color-accent)]"
                    >
                      {deger}
                      <span className="ml-1.5 text-[var(--color-text-3)]">{adet}</span>
                    </button>
                  ))}
                </div>
              </div>
            )}

            <div className="flex justify-end gap-2">
              <Button onClick={() => setForm(null)}>Geri</Button>
              <Button tone="accent" disabled={!form.value.trim()} onClick={saveForm}>
                Kaydet
              </Button>
            </div>
          </div>
        )}
      </div>
    </Modal>
  );
}

// ---------------------------------------------------- dosya gönder / topla

const ROOT_LABELS: Record<FileDestination, string> = {
  desktop: "Masaüstü",
  documents: "Belgeler",
  downloads: "İndirilenler",
};

const ROOT_ICONS: Record<FileDestination, React.ReactNode> = {
  desktop: <Monitor size={14} />,
  documents: <Folder size={14} />,
  downloads: <Download size={14} />,
};

const ROOT_OPTIONS: FileDestination[] = ["desktop", "documents", "downloads"];

/** Öğrencinin kullanıcı klasörlerinden birini seçtiren üçlü düğme. */
function RootPicker({
  value,
  onChange,
}: {
  value: FileDestination;
  onChange: (root: FileDestination) => void;
}) {
  return (
    <div role="radiogroup" className="grid grid-cols-3 gap-1.5">
      {ROOT_OPTIONS.map((r) => {
        const active = r === value;
        return (
          <button
            key={r}
            type="button"
            role="radio"
            aria-checked={active}
            onClick={() => onChange(r)}
            className={`flex h-[34px] items-center justify-center gap-1.5 rounded-md border
              text-[12px] font-medium transition-colors ${
                active
                  ? "border-[var(--color-accent)] bg-[var(--color-accent-soft)] text-[var(--color-accent)]"
                  : `border-[var(--color-stroke-strong)] text-[var(--color-text-2)]
                     hover:border-[var(--color-accent)] hover:text-[var(--color-text)]`
              }`}
          >
            {ROOT_ICONS[r]}
            {ROOT_LABELS[r]}
          </button>
        );
      })}
    </div>
  );
}

/** İşlemin uygulanacağı bilgisayarların özeti. */
function TargetStrip({ targets }: { targets: DeviceView[] }) {
  const shown = targets.slice(0, 6).map((d) => d.hostname).join(", ");
  const rest = targets.length - 6;
  return (
    <div
      className="flex items-center gap-2 rounded-md bg-[var(--color-surface-2)] px-2.5 py-2
        text-[11px] text-[var(--color-text-2)]"
      title={targets.map((d) => d.hostname).join(", ")}
    >
      <span
        className="shrink-0 rounded bg-[var(--color-accent)] px-1.5 py-0.5 text-[11px]
          font-semibold text-white"
      >
        {targets.length} bilgisayar
      </span>
      <span className="truncate">
        {targets.length === 0
          ? "Bağlı bilgisayar yok"
          : `${shown}${rest > 0 ? ` ve ${rest} bilgisayar daha` : ""}`}
      </span>
    </div>
  );
}

/** Numaralı bölüm başlığı: pencerenin adımlarını sırayla okutur. */
function Step({
  n,
  title,
  children,
}: {
  n: number;
  title: string;
  children: React.ReactNode;
}) {
  return (
    <section>
      <h4 className="mb-1.5 flex items-center gap-2 text-[12px] font-semibold">
        <span
          className="flex size-[18px] items-center justify-center rounded-full
            bg-[var(--color-surface-2)] text-[10.5px] text-[var(--color-text-2)]"
        >
          {n}
        </span>
        {title}
      </h4>
      {children}
    </section>
  );
}

/** Yoldan dosya adını ayırır (Windows ve Unix ayırıcılarıyla). */
function baseName(path: string): string {
  return path.split(/[\\/]/).pop() || path;
}

/** Alt klasör adında kabul edilmeyen bir şey varsa açıklamasını döner. */
function subfolderProblem(value: string): string | null {
  const v = value.trim();
  if (!v) return null;
  if (/[:*?"<>|]/.test(v)) return 'Klasör adında : * ? " < > | kullanılamaz.';
  if (/^[\\/]/.test(v) || v.split(/[\\/]/).some((p) => !p.trim() || p === "." || p === ".."))
    return "Yalnızca klasör adı yazın (örnek: Sınav ya da Ödevler/Hafta 3).";
  return null;
}

/**
 * Seçili bilgisayarlara dosya gönderme.
 *
 * Birden çok dosya seçilebilir; hepsi bütün hedeflere sırayla gider. Hedef,
 * öğrencinin kendi kullanıcı klasörlerinden biridir; dosyalar doğrudan oraya
 * ya da adı verilen alt klasöre kaydedilir.
 */
export function SendFilesDialog({
  open,
  targets,
  onClose,
  onSent,
}: {
  open: boolean;
  targets: DeviceView[];
  onClose: () => void;
  onSent: () => void;
}) {
  const { guard, show } = useToast();
  const [files, setFiles] = useState<string[]>([]);
  const [root, setRoot] = useState<FileDestination>("desktop");
  const [subfolder, setSubfolder] = useState("");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (!open) return;
    setFiles([]);
    setSubfolder("");
    setBusy(false);
  }, [open]);

  async function addFiles() {
    const picked = await openFileDialog({
      multiple: true,
      directory: false,
      title: "Gönderilecek dosyaları seçin",
    });
    if (!picked) return;
    const list = Array.isArray(picked) ? picked : [picked];
    setFiles((prev) => [...prev, ...list.filter((p) => !prev.includes(p))]);
  }

  const problem = subfolderProblem(subfolder);
  const place = subfolder.trim()
    ? `${ROOT_LABELS[root]}\\${subfolder.trim().replace(/\//g, "\\")}`
    : ROOT_LABELS[root];

  async function send() {
    setBusy(true);
    const n = await guard(
      api.sendFiles(
        targets.map((d) => d.deviceId),
        files,
        root,
        subfolder.trim() || null,
      ),
    );
    setBusy(false);
    if (n === undefined) return;
    show(
      "success",
      `${n === 1 ? baseName(files[0] ?? "") : `${n} dosya`} ${targets.length} bilgisayara gönderiliyor: ${place}`,
    );
    onSent();
    onClose();
  }

  return (
    <Modal
      open={open}
      title="Dosya gönder"
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>Vazgeç</Button>
          <Button
            tone="accent"
            onClick={send}
            disabled={busy || files.length === 0 || targets.length === 0 || problem !== null}
          >
            <FileUp size={13} />
            {busy ? "Gönderiliyor…" : `${targets.length} bilgisayara gönder`}
          </Button>
        </>
      }
    >
      <div className="space-y-4">
        <TargetStrip targets={targets} />

        <Step n={1} title="Dosyalar">
          <div className="rounded-md border border-[var(--color-stroke-strong)]">
            {files.length === 0 ? (
              <button
                type="button"
                onClick={addFiles}
                className="flex w-full flex-col items-center gap-1 px-3 py-5 text-center
                  text-[var(--color-text-3)] hover:text-[var(--color-accent)]"
              >
                <FileUp size={20} />
                <span className="text-[12px] font-medium">Dosya seçmek için tıklayın</span>
                <span className="text-[11px]">Birden çok dosya seçebilirsiniz.</span>
              </button>
            ) : (
              <>
                <ul className="max-h-[132px] divide-y divide-[var(--color-stroke)] overflow-auto">
                  {files.map((path) => (
                    <li key={path} className="flex items-center gap-2 px-2.5 py-1.5" title={path}>
                      <FileText size={13} className="shrink-0 text-[var(--color-text-3)]" />
                      <span className="truncate text-[12px]">{baseName(path)}</span>
                      <button
                        type="button"
                        aria-label={`${baseName(path)} dosyasını listeden çıkar`}
                        onClick={() => setFiles((prev) => prev.filter((p) => p !== path))}
                        className="ml-auto shrink-0 rounded p-0.5 text-[var(--color-text-3)]
                          hover:bg-[var(--color-surface-2)] hover:text-[var(--color-danger)]"
                      >
                        <X size={13} />
                      </button>
                    </li>
                  ))}
                </ul>
                <div
                  className="flex items-center border-t border-[var(--color-stroke)]
                    bg-[var(--color-surface-2)] px-2.5 py-1.5"
                >
                  <span className="text-[11px] text-[var(--color-text-3)]">
                    {files.length} dosya
                  </span>
                  <Button className="ml-auto" onClick={addFiles}>
                    <Plus size={13} /> Dosya ekle
                  </Button>
                </div>
              </>
            )}
          </div>
        </Step>

        <Step n={2} title="Öğrencide nereye kaydedilsin?">
          <RootPicker value={root} onChange={setRoot} />
          <div className="mt-2">
            <Field
              label="Alt klasör (isteğe bağlı)"
              hint={
                problem ??
                `Dosyalar öğrencinin ${place} klasörüne kaydedilir. Aynı adlı dosyanın üzerine yazılmaz.`
              }
            >
              <TextInput
                className="w-full"
                value={subfolder}
                onChange={(e) => setSubfolder(e.target.value)}
                placeholder="Boş bırakılırsa doğrudan klasörün içine"
                aria-invalid={problem !== null}
              />
            </Field>
          </div>
        </Step>
      </div>
    </Modal>
  );
}

/** Toplama deseni için hazır seçenekler. */
const PATTERN_PRESETS: { label: string; pattern: string }[] = [
  { label: "Tüm dosyalar", pattern: "*" },
  { label: "ZIP", pattern: "*.zip" },
  { label: "Word", pattern: "*.docx" },
  { label: "PDF", pattern: "*.pdf" },
  { label: "Resim", pattern: "*.png" },
];

/**
 * Seçili bilgisayarlardan dosya toplama.
 *
 * Öğretmen bir klasör ve bir dosya adı (ya da `*.zip` gibi bir desen) verir;
 * her öğrenci o klasörde uyan dosyaları gönderir. Toplananlar öğretmenin
 * bilgisayarında, her öğrencinin kendi adını taşıyan klasöre kaydedilir.
 */
export function CollectDialog({
  open,
  targets,
  onClose,
  onCollected,
  onShowResults,
}: {
  open: boolean;
  targets: DeviceView[];
  onClose: () => void;
  onCollected: () => void;
  /** Önceki toplamanın sonuç tablosunu açar; toplama yapılmadıysa verilmez. */
  onShowResults?: () => void;
}) {
  const { guard, show } = useToast();
  const [root, setRoot] = useState<FileDestination>("desktop");
  const [pattern, setPattern] = useState("");
  const [deleteSource, setDeleteSource] = useState(false);
  const [busy, setBusy] = useState(false);
  const [saveDir, setSaveDir] = useState("");

  useEffect(() => {
    if (!open) return;
    setRoot("desktop");
    setPattern("sınav.zip");
    setDeleteSource(false);
    void api.getDownloadRoot().then(setSaveDir).catch(() => setSaveDir(""));
  }, [open]);

  async function collect() {
    const p = pattern.trim();
    if (!p) {
      show("error", "Dosya adı boş olamaz.");
      return;
    }
    setBusy(true);
    const sent = await guard(
      api.collectFiles(
        targets.map((d) => d.deviceId),
        p,
        root,
        deleteSource,
      ),
    );
    setBusy(false);
    if (sent !== undefined) {
      onCollected();
      onClose();
    }
  }

  return (
    <Modal
      open={open}
      title="Dosya topla"
      onClose={onClose}
      footer={
        <>
          {onShowResults && (
            <Button
              className="mr-auto"
              onClick={() => {
                onClose();
                onShowResults();
              }}
            >
              Son sonuçlar
            </Button>
          )}
          <Button onClick={onClose}>Vazgeç</Button>
          <Button
            tone={deleteSource ? "danger" : "accent"}
            onClick={collect}
            disabled={busy || targets.length === 0 || !pattern.trim()}
          >
            <FileArchive size={13} />
            {busy ? "İsteniyor…" : `${targets.length} bilgisayardan topla`}
          </Button>
        </>
      }
    >
      <div className="space-y-4">
        <TargetStrip targets={targets} />

        <Step n={1} title="Öğrencinin hangi klasöründen?">
          <RootPicker value={root} onChange={setRoot} />
        </Step>

        <Step n={2} title="Hangi dosyalar?">
          <TextInput
            className="w-full font-mono"
            value={pattern}
            onChange={(e) => setPattern(e.target.value)}
            placeholder="sınav.zip"
            aria-label="Dosya adı ya da desen"
          />
          <div className="mt-1.5 flex flex-wrap gap-1">
            {PATTERN_PRESETS.map((preset) => (
              <button
                key={preset.pattern}
                type="button"
                onClick={() => setPattern(preset.pattern)}
                className={`rounded-full border px-2 py-0.5 text-[11px] transition-colors ${
                  pattern.trim() === preset.pattern
                    ? "border-[var(--color-accent)] bg-[var(--color-accent-soft)] text-[var(--color-accent)]"
                    : `border-[var(--color-stroke-strong)] text-[var(--color-text-2)]
                       hover:border-[var(--color-accent)]`
                }`}
              >
                {preset.label}
              </button>
            ))}
          </div>
          <p className="mt-1.5 text-[11px] leading-relaxed text-[var(--color-text-3)]">
            Tam dosya adı yazın ya da yıldızla tür seçin (*.zip). Yalnızca{" "}
            {ROOT_LABELS[root]} klasörünün içindeki dosyalara bakılır; alt klasörlere girilmez.
          </p>
        </Step>

        <Step n={3} title="Toplananlar nereye kaydedilecek?">
          <div
            className="flex items-center gap-2 rounded-md border border-[var(--color-stroke-strong)]
              px-2.5 py-1.5"
          >
            <FolderOpen size={14} className="shrink-0 text-[var(--color-text-3)]" />
            <span data-selectable className="min-w-0 truncate text-[11.5px]" title={saveDir}>
              {saveDir || "Belgeler\\OpenClass\\Toplanan"}
            </span>
            <Button className="ml-auto shrink-0" onClick={() => void guard(api.openDownloadsDir())}>
              Aç
            </Button>
          </div>
          <p className="mt-1.5 text-[11px] text-[var(--color-text-3)]">
            Her öğrencinin dosyaları kendi bilgisayar adını taşıyan klasöre kaydedilir.
          </p>
        </Step>

        <label
          className={`flex cursor-pointer items-start gap-2 rounded-md border p-2.5 ${
            deleteSource
              ? "border-[var(--color-danger)] bg-[var(--color-danger)]/5"
              : "border-[var(--color-stroke-strong)]"
          }`}
        >
          <input
            type="checkbox"
            checked={deleteSource}
            onChange={(e) => setDeleteSource(e.target.checked)}
            className="mt-0.5 accent-[var(--color-danger)]"
          />
          <span className="min-w-0">
            <span className="block text-[12px] font-medium">
              Toplandıktan sonra öğrenciden sil
            </span>
            <span className="block text-[11px] leading-relaxed text-[var(--color-text-3)]">
              Dosya öğretmene ulaştığı doğrulanınca öğrencinin bilgisayarından kalıcı olarak
              silinir. Geri alınamaz.
            </span>
          </span>
        </label>
      </div>
    </Modal>
  );
}

/** Toplama tablosunda bir satırın durum yazısı ve rengi. */
function collectStatus(t: CollectTarget): { text: string; tone: "ok" | "warn" | "danger" | "muted" } {
  const files = `${t.files} dosya`;
  if (t.errors.length > 0 && t.files === 0) return { text: t.errors[0]!, tone: "danger" };
  switch (t.state) {
    case "waiting":
      return { text: "Bekleniyor…", tone: "muted" };
    case "receiving":
      return { text: `${files} alındı, sürüyor…`, tone: "muted" };
    case "done":
      if (t.matched === 0) return { text: "Dosya bulunamadı", tone: "warn" };
      if (t.errors.length > 0 || (t.matched !== null && t.files < t.matched))
        return { text: `${files} alındı, ${t.matched ?? "?"} dosyadan`, tone: "warn" };
      return { text: `${files} teslim edildi`, tone: "ok" };
    case "no_reply":
      return t.files > 0
        ? { text: `${files} alındı`, tone: "ok" }
        : { text: "Yanıt yok (dosya yok ya da eski sürüm)", tone: "warn" };
    case "offline":
      return t.files > 0
        ? { text: `${files} alındı, bağlantı koptu`, tone: "warn" }
        : { text: "Bağlı değil", tone: "danger" };
  }
}

const COLLECT_TONE = {
  ok: "text-[var(--color-ok)]",
  warn: "text-[var(--color-warn)]",
  danger: "text-[var(--color-danger)]",
  muted: "text-[var(--color-text-3)]",
};

/**
 * Dosya toplamanın sonuç tablosu: kim teslim etti, kimde dosya yok, kimde
 * hata var. Toplama sürerken canlı güncellenir.
 */
export function CollectResultsDialog({
  open,
  collection,
  onClose,
}: {
  open: boolean;
  collection: CollectionView | null;
  onClose: () => void;
}) {
  const { guard } = useToast();
  const targets = collection?.targets ?? [];
  const delivered = targets.filter((t) => t.files > 0).length;
  const busy = targets.some((t) => t.state === "waiting" || t.state === "receiving");
  const total = targets.reduce((n, t) => n + t.files, 0);

  return (
    <Modal
      open={open && collection !== null}
      title="Dosya toplama sonuçları"
      width="md"
      onClose={onClose}
      footer={
        <>
          <span className="mr-auto text-[11px] text-[var(--color-text-3)]">
            {busy ? "Toplama sürüyor…" : "Toplama bitti."}
          </span>
          <Button onClick={() => void guard(api.openDownloadsDir())}>
            <FolderOpen size={13} /> Klasörü aç
          </Button>
          <Button tone="accent" onClick={onClose}>
            Kapat
          </Button>
        </>
      }
    >
      {collection && (
        <div className="space-y-3">
          <div className="grid grid-cols-3 gap-2">
            {[
              { label: "Teslim eden", value: `${delivered} / ${targets.length}` },
              { label: "Toplanan dosya", value: String(total) },
              { label: "Aranan", value: `${collection.root} · ${collection.pattern}` },
            ].map((m) => (
              <div
                key={m.label}
                className="rounded-md bg-[var(--color-surface-2)] px-2.5 py-2"
                title={m.value}
              >
                <div className="text-[10.5px] text-[var(--color-text-3)]">{m.label}</div>
                <div className="truncate text-[13px] font-semibold">{m.value}</div>
              </div>
            ))}
          </div>

          <table className="w-full border-collapse text-[12px]">
            <thead>
              <tr className="border-b border-[var(--color-stroke)] text-left text-[11px]
                text-[var(--color-text-3)]">
                <th className="py-1.5 pr-3 font-medium">Bilgisayar</th>
                <th className="py-1.5 font-medium">Durum</th>
              </tr>
            </thead>
            <tbody>
              {targets.map((t) => {
                const status = collectStatus(t);
                return (
                  <tr key={t.deviceId} className="border-b border-[var(--color-stroke)] align-top">
                    <td className="py-1.5 pr-3 font-medium">{t.hostname}</td>
                    <td className={`py-1.5 ${COLLECT_TONE[status.tone]}`}>
                      {status.text}
                      {t.errors.length > 0 && t.files > 0 && (
                        <span data-selectable className="block text-[11px] text-[var(--color-danger)]">
                          {t.errors.slice(0, 3).join(" · ")}
                        </span>
                      )}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>

          <p data-selectable className="text-[11px] text-[var(--color-text-3)]" title={collection.dir}>
            Dosyalar {collection.dir} klasöründe, her bilgisayarın kendi adını taşıyan klasöre
            kaydedildi.
            {collection.deleteSource && " Teslim edilen dosyalar öğrenci bilgisayarından silindi."}
          </p>
        </div>
      )}
    </Modal>
  );
}

// ------------------------------------------------------------- güncelleme

/**
 * Yeni sürüm bildirimi.
 *
 * Program kendini güncellemez; öğretmen isterse indirme sayfası açılır.
 * "Bu sürümü atla" denirse aynı sürüm için bir daha sorulmaz.
 */
export function UpdateDialog({
  update,
  current,
  onSkip,
  onClose,
}: {
  update: UpdateInfo | null;
  current: string;
  onSkip: () => void;
  onClose: () => void;
}) {
  const { guard } = useToast();

  return (
    <Modal
      open={update !== null}
      title="Yeni sürüm var"
      onClose={onClose}
      footer={
        <>
          <Button className="mr-auto" onClick={onSkip}>
            Bu sürümü atla
          </Button>
          <Button onClick={onClose}>Sonra</Button>
          <Button
            tone="accent"
            onClick={async () => {
              await guard(api.openReleasePage());
              onClose();
            }}
          >
            <Download size={13} /> İndirme sayfasını aç
          </Button>
        </>
      }
    >
      {update && (
        <div className="space-y-2.5">
          <p className="text-[12px] leading-relaxed">
            OpenClass <strong>{update.version}</strong> yayımlandı. Bu bilgisayarda{" "}
            <strong>{current}</strong> kurulu.
          </p>
          {update.notes && (
            <pre
              data-selectable
              className="max-h-[180px] overflow-auto whitespace-pre-wrap rounded border
                border-[var(--color-stroke)] bg-[var(--color-surface-2)] p-2.5 font-sans
                text-[11px] leading-relaxed text-[var(--color-text-2)]"
            >
              {update.notes}
            </pre>
          )}
          <p className="text-[11px] leading-relaxed text-[var(--color-text-3)]">
            Hiçbir şey kendiliğinden indirilmez ya da kurulmaz. Yeni sürümü kurarsanız öğrenci
            bilgisayarlarındaki programı da aynı sürüme güncelleyin.
          </p>
        </div>
      )}
    </Modal>
  );
}

// ---------------------------------------------------------- uzaktan kapatma

/** Tek bir öğrencinin bilgisayarını kapatma onayı. */
export function ShutdownDialog({
  targets,
  onClose,
  onConfirm,
  restart = false,
}: {
  /** İşlemin uygulanacağı bilgisayarlar; boşsa pencere kapalıdır. */
  targets: DeviceView[];
  onClose: () => void;
  onConfirm: () => void;
  /** Kapatmak yerine yeniden başlat. */
  restart?: boolean;
}) {
  const many = targets.length > 1;
  const action = restart ? "yeniden başlatılacak" : "kapatılacak";
  const subject = many ? `${targets.length} bilgisayar` : (targets[0]?.hostname ?? "");

  return (
    <Modal
      open={targets.length > 0}
      title={`${subject} ${action}`}
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>Vazgeç</Button>
          <Button tone="danger" onClick={onConfirm}>
            {restart ? <RotateCw size={13} /> : <Power size={13} />}
            {restart ? "Yeniden Başlat" : many ? "Bilgisayarları Kapat" : "Bilgisayarı Kapat"}
          </Button>
        </>
      }
    >
      <div className="space-y-3 p-3">
        <p className="rounded bg-[var(--color-warn)]/10 p-2.5 text-[12px] leading-relaxed
          text-[var(--color-text)]">
          <strong>{subject}</strong> {action}. Komut geri alınamaz; öğrencilere önce kısa bir
          bildirim gösterilir.
        </p>
        {many && (
          // Toplu işlemde hangi bilgisayarların etkileneceği açıkça görünsün.
          <p data-selectable className="max-h-[120px] overflow-auto rounded border
            border-[var(--color-stroke)] p-2 text-[11px] leading-relaxed text-[var(--color-text-2)]">
            {targets.map((d) => d.hostname).join(", ")}
          </p>
        )}
        <p className="text-[11px] leading-relaxed text-[var(--color-text-3)]">
          {restart
            ? "Kaydedilmemiş çalışma kaybolabilir. Bilgisayarlar açıldığında öğrenci ajanı kendiliğinden yeniden bağlanır."
            : "Kaydedilmemiş çalışma kaybolabilir. Kapanan bilgisayarlar konsoldan yeniden açılamaz."}
        </p>
      </div>
    </Modal>
  );
}

// ------------------------------------------------------------- hata raporu

/**
 * Önceki çalışmada uygulama çöktüyse, raporun gönderilmesi için onay ister.
 *
 * Hiçbir şey kendiliğinden gönderilmez. Gidecek metnin tamamı pencerede
 * görünür; öğretmen "Gönderme" derse kayıt silinir ve bir daha sorulmaz.
 */
export function CrashReportDialog({
  reports,
  onDone,
}: {
  reports: CrashReport[];
  onDone: () => void;
}) {
  const { show } = useToast();
  const [busy, setBusy] = useState(false);

  async function finish(send: boolean) {
    setBusy(true);
    let failed = false;
    if (send) {
      for (const report of reports) {
        try {
          await sendCrashReport(report);
        } catch {
          failed = true;
        }
      }
    }
    // Gönderim başarısızsa kayıt saklanır; bir sonraki açılışta yeniden sorulur.
    if (!failed) await api.clearCrashReports().catch(() => {});
    setBusy(false);
    if (send) {
      show(
        failed ? "error" : "success",
        failed
          ? "Rapor gönderilemedi (internet bağlantısı yok olabilir). Sonraki açılışta yeniden sorulacak."
          : "Hata raporu gönderildi. Teşekkürler.",
      );
    }
    onDone();
  }

  return (
    <Modal
      open={reports.length > 0}
      title={
        reports.every((r) => r.app !== "teacher")
          ? "Öğrenci programı beklenmedik şekilde kapandı"
          : "OpenClass beklenmedik şekilde kapandı"
      }
      onClose={() => void finish(false)}
      footer={
        <>
          <Button disabled={busy} onClick={() => void finish(false)}>
            Gönderme
          </Button>
          <Button tone="accent" disabled={busy} onClick={() => void finish(true)}>
            {busy ? "Gönderiliyor…" : "Raporu Gönder"}
          </Button>
        </>
      }
    >
      <div className="space-y-2.5">
        <p className="text-[12px] leading-relaxed text-[var(--color-text-2)]">
          Sorunu düzeltebilmemiz için hata raporu geliştiricilere gönderilsin mi? Gönderilecek
          bilginin tamamı aşağıdadır. Bilgisayar adı, kullanıcı adı, IP adresi ve ekran içeriği{" "}
          <strong>gönderilmez</strong>.
        </p>
        {reports.map((r, i) => (
          <dl
            key={`${r.at}-${i}`}
            data-selectable
            className="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 rounded border
              border-[var(--color-stroke)] bg-[var(--color-surface-2)] p-2.5 text-[11px]"
          >
            <dt className="text-[var(--color-text-3)]">Program</dt>
            <dd>
              {r.app === "teacher" ? "Öğretmen" : "Öğrenci"} {r.version} · {r.os}
            </dd>
            <dt className="text-[var(--color-text-3)]">Zaman</dt>
            <dd>{formatTime(r.at)}</dd>
            <dt className="text-[var(--color-text-3)]">Hata</dt>
            <dd className="break-words font-mono">{r.message}</dd>
            <dt className="text-[var(--color-text-3)]">Konum</dt>
            <dd className="break-all font-mono">{r.location || "—"}</dd>
          </dl>
        ))}
      </div>
    </Modal>
  );
}
