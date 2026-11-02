/** Komut şeridinden açılan iletişim kutuları. */
import { useEffect, useMemo, useState } from "react";
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

  // Zaten eklenmiş gruplar önerilerden düşülür. Oda önerisi ada göre,
  // ön ek önerisi ön eke göre karşılaştırılır.
  const alinanOnekler = new Set(existing.map((g) => g.prefix.trim().toUpperCase()));
  const alinanOdalar = new Set(existing.map((g) => g.name.trim().toUpperCase()));
  const fresh = suggestions.filter((s) =>
    s.room
      ? !alinanOdalar.has(s.room.trim().toUpperCase())
      : !alinanOnekler.has(s.prefix.trim().toUpperCase()),
  );

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
            disabled={!name.trim()}
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
      <p className="mb-3 text-[11px] leading-relaxed text-[var(--color-text-2)]">
        Bir grup iki yolla eşleşir. <strong>Oda adı</strong> önce gelir:
        öğrencinin ilk açılışta girdiği laboratuvar adı grup adıyla aynıysa
        cihaz bu gruba düşer. Odası bilinmeyen cihazlar için{" "}
        <strong>bilgisayar adı ön eki</strong> kullanılır — <code>LAB1</code>{" "}
        ön eki <code>LAB101</code>, <code>LAB102</code> … cihazlarını kapsar.
        Yalnızca odaya göre eşleşen bir grup için ön eki boş bırakın.
      </p>

      {fresh.length > 0 && (
        <div className="mb-3">
          <p className="mb-1.5 text-[11px] font-medium text-[var(--color-text-2)]">
            Öneriler — bildirilen odalar ve ağdaki adlar
          </p>
          <div className="flex flex-wrap gap-1.5">
            {fresh.map((s) => (
              <button
                key={s.room ? `oda:${s.room}` : `onek:${s.prefix}`}
                title={
                  s.room
                    ? "Öğrencilerin bildirdiği oda adı"
                    : "Bilgisayar adlarından çıkarılan ön ek"
                }
                onClick={() => {
                  if (s.room) {
                    setName(s.room);
                    setPrefix("");
                  } else {
                    setPrefix(s.prefix);
                    if (!name.trim()) setName(s.prefix);
                  }
                }}
                className={`rounded border px-2 py-1 text-[11px]
                  hover:border-[var(--color-accent)] hover:text-[var(--color-accent)]
                  ${
                    s.room
                      ? "border-[var(--color-accent)]/40 bg-[var(--color-accent-soft)]"
                      : "border-[var(--color-stroke-strong)]"
                  }`}
              >
                {s.room ? (
                  <span>{s.room}</span>
                ) : (
                  <span className="font-mono">{s.prefix}</span>
                )}
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
        <Field
          label="Bilgisayar adı ön eki"
          hint="Büyük/küçük harf fark etmez. Yalnızca odaya göre eşleşmesi için boş bırakın."
        >
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
  controlled,
  onToggleControl,
  onClose,
}: {
  device: DeviceView | null;
  frameBaseUrl: string;
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
}: {
  open: boolean;
  info: ServerInfo | null;
  onClose: () => void;
  onSaved: () => void;
}) {
  const { guard } = useToast();
  const [teacherName, setTeacherName] = useState("");
  const [className, setClassName] = useState("");
  const [activeRoom, setActiveRoom] = useState("");
  const [activePrefix, setActivePrefix] = useState("");
  const [hosts, setHosts] = useState("");

  useEffect(() => {
    if (!info || !open) return;
    setTeacherName(info.teacherName);
    setClassName(info.className);
    setActiveRoom(info.activeRoom ?? "");
    setActivePrefix(info.activePrefix ?? "");
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
                  activeRoom,
                  activePrefix,
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
          label="Yönetilen laboratuvar"
          hint="Öğrencilerin bildirdiği oda adı. Boş bırakılırsa oda süzmesi yapılmaz."
        >
          <TextInput
            className="w-full"
            value={activeRoom}
            placeholder="Tümü"
            onChange={(e) => setActiveRoom(e.target.value)}
          />
        </Field>
        <Field
          label="Bilgisayar adı öneki"
          hint="LAB1 öneki LAB101, LAB102 … bilgisayarlarını kapsar. Oda adıyla birlikte verilirse ikisi de uygulanır."
        >
          <TextInput
            className="w-full font-mono"
            value={activePrefix}
            placeholder="Tümü"
            onChange={(e) => setActivePrefix(e.target.value.toUpperCase())}
          />
        </Field>
        <div className="flex items-center gap-2 rounded bg-[var(--color-bg-2)] px-2.5 py-2">
          <span className="flex-1 text-[11px] leading-snug text-[var(--color-text-2)]">
            Açılıştaki laboratuvar seçim ekranını yeniden görmek isterseniz
            sıfırlayın; konsol bir sonraki açılışta tekrar sorar.
          </span>
          <Button
            onClick={async () => {
              const ok = await guard(
                api.updateSettings({ setupCompleted: false }),
                "Açılışta yeniden sorulacak.",
              );
              if (ok !== undefined) onSaved();
            }}
          >
            <RefreshCw size={13} /> Yeniden sor
          </Button>
        </div>
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
          görünür. Kilit ders bitince, bağlantı kopunca veya bir saat sonra
          kendiliğinden kalkar. <strong>Ctrl+Alt+Del engellenmez</strong> —
          öğrenci her zaman makineyi kurtarabilir.
        </p>
      </div>
    </Modal>
  );
}

// ------------------------------------------------------- laboratuvar seçimi

/**
 * Açılışta hangi bilgisayarların yönetileceğini sorar.
 *
 * NetSupport'un Class Wizard'ındaki Room ve PC modlarının karşılığı: ölçüt
 * **yazılır**, liste ondan sonra süzülür. Keşifte görülen odalar ve ön ekler
 * altta öneri olarak durur ama zorunlu değildir — henüz açılmamış bir
 * laboratuvarın adı da baştan girilebilir, makineler açıldıkça listeye
 * düşerler.
 *
 * Yalnızca `setupCompleted` false iken çıkar; seçim Ayarlar'dan değişir.
 */
export function RoomPickerDialog({
  open,
  devices,
  onPick,
}: {
  open: boolean;
  devices: DeviceView[];
  onPick: (secim: { room: string; prefix: string }) => void;
}) {
  const [mode, setMode] = useState<"room" | "prefix" | "all">("room");
  const [room, setRoom] = useState("");
  const [prefix, setPrefix] = useState("");

  // Keşifte görülen odalar ve cihaz sayıları.
  const odalar = useMemo(() => {
    const sayac = new Map<string, number>();
    for (const d of devices) {
      const oda = d.room?.trim();
      if (!oda) continue;
      sayac.set(oda, (sayac.get(oda) ?? 0) + 1);
    }
    return [...sayac.entries()].sort(
      (a, b) => b[1] - a[1] || a[0].localeCompare(b[0], "tr"),
    );
  }, [devices]);

  // Bilgisayar adlarından çıkarılan ön ek adayları.
  const onekler = useMemo(() => {
    const sayac = new Map<string, number>();
    for (const d of devices) {
      const ad = d.hostname.trim().toUpperCase();
      const rakam = ad.length - ad.replace(/\d+$/, "").length;
      if (rakam === 0) continue;
      for (let kalan = 0; kalan < rakam; kalan++) {
        const aday = ad.slice(0, ad.length - rakam + kalan);
        if (aday) sayac.set(aday, (sayac.get(aday) ?? 0) + 1);
      }
    }
    return [...sayac.entries()]
      .filter(([, n]) => n >= 2)
      .sort((a, b) => b[1] - a[1] || b[0].length - a[0].length)
      .slice(0, 6);
  }, [devices]);

  const gecerli =
    mode === "all" ||
    (mode === "room" && room.trim().length > 0) ||
    (mode === "prefix" && prefix.trim().length > 0);

  function uygula() {
    if (!gecerli) return;
    onPick({
      room: mode === "room" ? room.trim() : "",
      prefix: mode === "prefix" ? prefix.trim() : "",
    });
  }

  const sekmeler: { key: typeof mode; label: string }[] = [
    { key: "room", label: "Laboratuvar adı" },
    { key: "prefix", label: "Bilgisayar adı öneki" },
    { key: "all", label: "Tümü" },
  ];

  return (
    <Modal
      open={open}
      title="Hangi bilgisayarları yöneteceksiniz?"
      onClose={() => onPick({ room: "", prefix: "" })}
      footer={
        <>
          <Button onClick={() => onPick({ room: "", prefix: "" })}>Tümünü göster</Button>
          <Button tone="accent" disabled={!gecerli} onClick={uygula}>
            Devam
          </Button>
        </>
      }
    >
      <div className="space-y-3 p-3">
        {/* Mod seçimi */}
        <div role="tablist" className="flex gap-1 rounded bg-[var(--color-bg-2)] p-0.5">
          {sekmeler.map((t) => (
            <button
              key={t.key}
              role="tab"
              aria-selected={mode === t.key}
              onClick={() => setMode(t.key)}
              className={`flex-1 rounded px-2 py-1.5 text-[11.5px] transition-colors
                ${
                  mode === t.key
                    ? "bg-[var(--color-bg-1)] font-medium text-[var(--color-text)] shadow-sm"
                    : "text-[var(--color-text-2)] hover:text-[var(--color-text)]"
                }`}
            >
              {t.label}
            </button>
          ))}
        </div>

        {mode === "room" && (
          <div className="space-y-2">
            <Field
              label="Laboratuvar adı"
              hint="Öğrencilerin kendi bilgisayarlarında ilk kurulumda girdiği ad."
            >
              <TextInput
                className="w-full"
                value={room}
                autoFocus
                placeholder="Lab 1"
                onChange={(e) => setRoom(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") uygula();
                }}
              />
            </Field>
            {odalar.length > 0 && (
              <div>
                <p className="mb-1 text-[11px] text-[var(--color-text-3)]">
                  Ağda görülenler
                </p>
                <div className="flex flex-wrap gap-1.5">
                  {odalar.map(([oda, adet]) => (
                    <button
                      key={oda}
                      onClick={() => setRoom(oda)}
                      className="rounded border border-[var(--color-stroke-strong)] px-2 py-1
                        text-[11px] hover:border-[var(--color-accent)]
                        hover:text-[var(--color-accent)]"
                    >
                      {oda}
                      <span className="ml-1.5 text-[var(--color-text-3)]">{adet}</span>
                    </button>
                  ))}
                </div>
              </div>
            )}
          </div>
        )}

        {mode === "prefix" && (
          <div className="space-y-2">
            <Field
              label="Bilgisayar adı öneki"
              hint="LAB1 öneki LAB101, LAB102 … bilgisayarlarını kapsar. Büyük/küçük harf fark etmez."
            >
              <TextInput
                className="w-full font-mono"
                value={prefix}
                autoFocus
                placeholder="LAB1"
                onChange={(e) => setPrefix(e.target.value.toUpperCase())}
                onKeyDown={(e) => {
                  if (e.key === "Enter") uygula();
                }}
              />
            </Field>
            {onekler.length > 0 && (
              <div>
                <p className="mb-1 text-[11px] text-[var(--color-text-3)]">
                  Ağdaki adlardan
                </p>
                <div className="flex flex-wrap gap-1.5">
                  {onekler.map(([onek, adet]) => (
                    <button
                      key={onek}
                      onClick={() => setPrefix(onek)}
                      className="rounded border border-[var(--color-stroke-strong)] px-2 py-1
                        font-mono text-[11px] hover:border-[var(--color-accent)]
                        hover:text-[var(--color-accent)]"
                    >
                      {onek}
                      <span className="ml-1.5 font-sans text-[var(--color-text-3)]">
                        {adet}
                      </span>
                    </button>
                  ))}
                </div>
              </div>
            )}
          </div>
        )}

        {mode === "all" && (
          <p className="rounded bg-[var(--color-bg-2)] p-2.5 text-[11.5px] leading-relaxed
            text-[var(--color-text-2)]">
            Ağda bulunan bütün öğrenci bilgisayarları listelenir. Süzmeyi
            sonradan Ayarlar'dan veya arama kutusundan yapabilirsiniz.
          </p>
        )}

        <p className="text-[11px] leading-relaxed text-[var(--color-text-3)]">
          Ölçüt yazdığınız anda uygulanır; bilgisayarlar açıldıkça listeye
          düşerler. Henüz hiçbir makine açılmamışken de ad girebilirsiniz.
          Seçim Ayarlar'dan değiştirilebilir.
        </p>
      </div>
    </Modal>
  );
}
