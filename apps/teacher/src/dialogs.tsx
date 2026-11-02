/** Komut şeridinden açılan iletişim kutuları. */
import { useEffect, useState } from "react";
import { RefreshCw } from "lucide-react";

import { api, AUDIT_LABELS, formatTime } from "./api";
import { ScreenCanvas } from "@openclass/screen-view";
import { Lock, MousePointerBan, MousePointerClick, ScreenShare } from "lucide-react";

import { RemoteControlSurface } from "./RemoteControlSurface";
import { Button, Empty, Field, Modal, TextInput, useToast } from "./ui";
import type {
  AuditEntry,
  DeviceGroup,
  DeviceView,
  GroupSuggestion,
  PairingInfo,
  ServerInfo,
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

// ------------------------------------------------------------------- grup

export function GroupDialog({
  open,
  existing,
  onClose,
  onSaved,
}: {
  open: boolean;
  existing: DeviceGroup[];
  onClose: () => void;
  onSaved: () => void;
}) {
  const { guard } = useToast();
  const [name, setName] = useState("");
  const [prefix, setPrefix] = useState("");
  const [suggestions, setSuggestions] = useState<GroupSuggestion[]>([]);

  useEffect(() => {
    if (!open) return;
    setName("");
    setPrefix("");
    void api.suggestGroups().then(setSuggestions).catch(() => setSuggestions([]));
  }, [open]);

  const taken = new Set(existing.map((g) => g.prefix));
  const fresh = suggestions.filter((s) => !taken.has(s.prefix));

  return (
    <Modal
      open={open}
      title="Laboratuvar grubu ekle"
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>Vazgeç</Button>
          <Button
            tone="accent"
            disabled={!name.trim() || !prefix.trim()}
            onClick={async () => {
              const ok = await guard(api.saveGroup(name, prefix), `${name} grubu eklendi.`);
              if (ok !== undefined) {
                onSaved();
                onClose();
              }
            }}
          >
            Ekle
          </Button>
        </>
      }
    >
      <p className="mb-3 text-[11px] text-[var(--color-text-2)]">
        Grup, bilgisayar adı ön ekinden ibarettir. <code>LAB1</code> ön eki{" "}
        <code>LAB101</code>, <code>LAB102</code> … cihazlarını kapsar. Hiçbir adlandırma
        düzeni önceden varsayılmaz.
      </p>

      {fresh.length > 0 && (
        <div className="mb-3">
          <p className="mb-1.5 text-[11px] font-medium text-[var(--color-text-2)]">
            Ağdaki adlardan öneriler
          </p>
          <div className="flex flex-wrap gap-1.5">
            {fresh.map((s) => (
              <button
                key={s.prefix}
                onClick={() => {
                  setPrefix(s.prefix);
                  if (!name.trim()) setName(s.prefix);
                }}
                className="rounded border border-[var(--color-stroke-strong)] px-2 py-1
                  text-[11px] hover:border-[var(--color-accent)] hover:text-[var(--color-accent)]"
              >
                <span className="font-mono">{s.prefix}</span>
                <span className="ml-1.5 text-[var(--color-text-3)]">{s.deviceCount} cihaz</span>
              </button>
            ))}
          </div>
        </div>
      )}

      <div className="space-y-3">
        <Field label="Grup adı">
          <TextInput
            className="w-full"
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="Lab 1"
          />
        </Field>
        <Field label="Bilgisayar adı ön eki" hint="Büyük/küçük harf fark etmez.">
          <TextInput
            className="w-full font-mono"
            value={prefix}
            onChange={(e) => setPrefix(e.target.value.toUpperCase())}
            placeholder="LAB1"
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
  sessionActive,
  controlled,
  onToggleControl,
  onClose,
}: {
  device: DeviceView | null;
  frameBaseUrl: string;
  sessionActive: boolean;
  /** Bu cihaz şu anda uzaktan kontrol ediliyor mu? */
  controlled: boolean;
  onToggleControl: () => void;
  onClose: () => void;
}) {
  const ratio =
    device && device.frameHeight > 0 ? device.frameWidth / device.frameHeight : 16 / 9;

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
              {device.frameWidth}×{device.frameHeight} · {device.address ?? "—"} · kare #
              {device.frameSeq}
            </span>
            {controlled && (
              <span className="mr-2 flex items-center gap-1.5 rounded bg-[var(--color-warn)]
                px-2 py-1 text-[11px] font-semibold text-white">
                <MousePointerClick size={12} />
                Kontrol sizde — fare ve klavye öğrenciye gidiyor
              </span>
            )}
            <Button
              tone={controlled ? "danger" : "accent"}
              disabled={!device.online || !sessionActive}
              title={
                sessionActive
                  ? undefined
                  : "Uzaktan kontrol için önce ders oturumunu başlatın"
              }
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
        (device.frameSeq > 0 ? (
          <RemoteControlSurface active={controlled} aspectRatio={ratio}>
            <ScreenCanvas
              deviceId={device.deviceId}
              frameBaseUrl={frameBaseUrl}
              seq={device.frameSeq}
              alt={`${device.hostname} ekranı`}
              className="max-h-[70vh] w-full bg-[#101418] object-contain"
            />
          </RemoteControlSurface>
        ) : (
          <Empty
            title="Henüz görüntü yok"
            detail={
              device.screenError ??
              (sessionActive
                ? "Ekran yayını başlatıldı, ilk kare bekleniyor."
                : "Ekran izlemek için önce ders oturumunu başlatın.")
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
}: {
  open: boolean;
  info: ServerInfo | null;
  onClose: () => void;
  onSaved: () => void;
}) {
  const { guard } = useToast();
  const [teacherName, setTeacherName] = useState("");
  const [className, setClassName] = useState("");
  const [hosts, setHosts] = useState("");

  useEffect(() => {
    if (!info || !open) return;
    setTeacherName(info.teacherName);
    setClassName(info.className);
    setHosts(info.extraDiscoveryHosts.join("\n"));
  }, [info, open]);

  return (
    <Modal
      open={open}
      title="Ayarlar"
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>Vazgeç</Button>
          <Button
            tone="accent"
            onClick={async () => {
              await guard(
                api.updateSettings({
                  teacherName,
                  className,
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
        <Field label="Varsayılan sınıf adı">
          <TextInput
            className="w-full"
            value={className}
            onChange={(e) => setClassName(e.target.value)}
          />
        </Field>
        <Field
          label="Ek keşif adresleri"
          hint="Okul ağı yayın paketlerini engelliyorsa alt ağların yayın adreslerini her satıra bir tane yazın."
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
          görünür. Kilit ders bitince, bağlantı kopunca veya bir saat sonra
          kendiliğinden kalkar. <strong>Ctrl+Alt+Del engellenmez</strong> —
          öğrenci her zaman makineyi kurtarabilir.
        </p>
      </div>
    </Modal>
  );
}
