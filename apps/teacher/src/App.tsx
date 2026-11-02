import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import {
  AlertTriangle,
  Cast,
  CheckSquare,
  ClipboardList,
  Hand,
  FileUp,
  FolderOpen,
  LayoutGrid,
  Link2,
  ListChecks,
  Lock,
  LockOpen,
  Maximize2,
  MessageSquare,
  Monitor,
  MonitorOff,
  MousePointerClick,
  Moon,
  Play,
  RotateCcw,
  Rows3,
  ScreenShare,
  ScreenShareOff,
  Settings as SettingsIcon,
  ShieldOff,
  Square,
  SquareDashed,
  Sun,
  UserRoundCog,
  Wifi,
} from "lucide-react";

import { api, AUDIT_LABELS, formatDuration, formatTime } from "./api";
import {
  AuditDialog,
  BroadcastDialog,
  FocusDialog,
  LockDialog,
  GroupDialog,
  MessageDialog,
  PairingDialog,
  SettingsDialog,
} from "./dialogs";
import { BrowserDialog } from "./BrowserDialog";
import { TransfersDialog } from "./TransfersDialog";
import { GroupBar, type GroupTab } from "./GroupBar";
import { DetailsTable, ThumbnailGrid } from "./ListView";
import { Ribbon, type RibbonTab } from "./Ribbon";
import { StatusBar, type ListMode } from "./StatusBar";
import {
  Button,
  ContextMenu,
  Empty,
  IconButton,
  Panel,
  TextInput,
  ToastProvider,
  useToast,
  type MenuItem,
} from "./ui";
import { useTheme } from "./useTheme";
import type {
  AuditEntry,
  BroadcastInfo,
  ClassSession,
  DeviceGroup,
  DeviceView,
  FrameEvent,
  HelpEvent,
  PairingInfo,
  ServerInfo,
} from "./types";

type ViewMode = "monitor" | "help" | "devices" | "audit";

const VIEW_MODES: { key: ViewMode; label: string; icon: typeof Monitor }[] = [
  { key: "monitor", label: "İzleme", icon: Monitor },
  { key: "help", label: "Yardım", icon: Hand },
  { key: "devices", label: "Cihazlar", icon: UserRoundCog },
  { key: "audit", label: "Kayıtlar", icon: ClipboardList },
];

export default function App() {
  return (
    <ToastProvider>
      <Console />
    </ToastProvider>
  );
}

function Console() {
  const { theme, toggle } = useTheme();
  const { guard, show } = useToast();

  // Görünüm
  const [ribbonTab, setRibbonTab] = useState("class");
  const [view, setView] = useState<ViewMode>("monitor");
  const [listMode, setListMode] = useState<ListMode>("thumbnails");
  const [thumbSize, setThumbSize] = useState(230);

  // Veri
  const [devices, setDevices] = useState<DeviceView[]>([]);
  const [groups, setGroups] = useState<DeviceGroup[]>([]);
  const [session, setSession] = useState<ClassSession | null>(null);
  const [info, setInfo] = useState<ServerInfo | null>(null);
  const [frameBaseUrl, setFrameBaseUrl] = useState("");
  const [broadcast, setBroadcast] = useState<BroadcastInfo | null>(null);
  const [controlled, setControlled] = useState<string | null>(null);
  const [broadcastOpen, setBroadcastOpen] = useState(false);
  const [lockOpen, setLockOpen] = useState(false);
  const [transfersOpen, setTransfersOpen] = useState(false);
  const [browserOpen, setBrowserOpen] = useState(false);

  // Seçim ve filtre
  const [activeGroup, setActiveGroup] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [appliedQuery, setAppliedQuery] = useState("");
  const [selected, setSelected] = useState<Set<string>>(new Set());

  // Diyaloglar
  const [pairing, setPairing] = useState<PairingInfo | null>(null);
  const [pairingOpen, setPairingOpen] = useState(false);
  const [messageOpen, setMessageOpen] = useState(false);
  const [groupOpen, setGroupOpen] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [auditOpen, setAuditOpen] = useState(false);
  const [focused, setFocused] = useState<string | null>(null);
  const [menu, setMenu] = useState<{ deviceId: string; x: number; y: number } | null>(null);

  const refreshDevices = useCallback(async () => {
    try {
      // Sunucu tarafı filtresi yerine tam liste alınır: grup sekmeleri ve
      // durum çubuğu sayaçları her zaman bütünü görmek zorunda.
      setDevices(await api.searchDevices(""));
    } catch (err) {
      show("error", err instanceof Error ? err.message : String(err));
    }
  }, [show]);

  const refreshInfo = useCallback(async () => {
    try {
      setInfo(await api.getServerInfo());
    } catch {
      /* açılışta kısa süre başarısız olabilir */
    }
  }, []);

  const refreshGroups = useCallback(async () => {
    try {
      setGroups(await api.listGroups());
    } catch {
      /* yok sayılır */
    }
  }, []);

  useEffect(() => {
    void (async () => {
      setFrameBaseUrl(await api.frameBaseUrl().catch(() => ""));
      setSession(await api.getSession().catch(() => null));
      setBroadcast(await api.getBroadcast().catch(() => null));
      await Promise.all([refreshInfo(), refreshDevices(), refreshGroups()]);
    })();
  }, [refreshDevices, refreshInfo, refreshGroups]);

  useEffect(() => {
    const unlisten = [
      listen("devices-changed", () => void refreshDevices()),
      listen("groups-changed", () => void refreshGroups()),
      listen<BroadcastInfo | null>("broadcast-changed", (e) => setBroadcast(e.payload)),
      listen<string | null>("control-changed", (e) => setControlled(e.payload)),
      listen<ClassSession | null>("session-changed", (e) => setSession(e.payload)),
      listen("pairing-changed", () => void api.getPairing().then(setPairing).catch(() => {})),
      listen<string>("network-error", (e) => show("error", e.payload)),
      listen<HelpEvent>("help-request", (e) => {
        show("info", `${e.payload.hostname} yardım istiyor.`);
        void refreshDevices();
      }),
      // Kare olayları saniyede onlarca gelir; tüm listeyi tazelemek yerine
      // yalnızca ilgili cihazın kare bilgisi güncellenir.
      listen<FrameEvent>("frame", (e) => {
        const { deviceId, seq, width, height } = e.payload;
        setDevices((list) =>
          list.map((d) =>
            d.deviceId === deviceId
              ? { ...d, frameSeq: seq, frameWidth: width, frameHeight: height, frameAgeMs: 0, streaming: true }
              : d,
          ),
        );
      }),
    ];
    return () => {
      void Promise.all(unlisten).then((fns) => fns.forEach((f) => f()));
    };
  }, [refreshDevices, refreshGroups, show]);

  // Çevrimdışına düşen cihazlar ve kare yaşı için düzenli tazeleme.
  useEffect(() => {
    const id = window.setInterval(() => {
      void refreshDevices();
      void refreshInfo();
    }, 3000);
    return () => window.clearInterval(id);
  }, [refreshDevices, refreshInfo]);

  useEffect(() => {
    if (!pairingOpen) return;
    const id = window.setInterval(() => {
      void api.getPairing().then(setPairing).catch(() => {});
    }, 1000);
    return () => window.clearInterval(id);
  }, [pairingOpen]);

  // ----------------------------------------------------------- türetilenler

  const groupFilter = useMemo(
    () => groups.find((g) => g.name === activeGroup) ?? null,
    [groups, activeGroup],
  );

  /**
   * Grup sekmesi ve arama kutusu birlikte uygulanır.
   *
   * Arama hem bilgisayar adında (ön ek) hem IP adresinde yapılır; öğretmen
   * hangisini yazdığını belirtmek zorunda değil.
   */
  const visible = useMemo(() => {
    const prefix = groupFilter?.prefix.toUpperCase() ?? "";
    const search = appliedQuery.trim().toUpperCase();
    return devices.filter((d) => {
      const host = d.hostname.toUpperCase();
      if (!host.startsWith(prefix)) return false;
      if (!search) return true;
      return host.startsWith(search) || (d.address ?? "").toUpperCase().includes(search);
    });
  }, [devices, groupFilter, appliedQuery]);

  const groupTabs: GroupTab[] = useMemo(() => {
    const count = (prefix: string) => {
      const p = prefix.toUpperCase();
      const members = devices.filter((d) => d.hostname.toUpperCase().startsWith(p));
      return { total: members.length, online: members.filter((d) => d.online).length };
    };
    return [
      { group: null, ...count("") },
      ...groups.map((g) => ({ group: g, ...count(g.prefix) })),
    ];
  }, [devices, groups]);

  const selectedDevices = useMemo(
    () => devices.filter((d) => selected.has(d.deviceId)),
    [devices, selected],
  );
  const selectedOnline = selectedDevices.filter((d) => d.online);
  /** Seçim yoksa komutlar görünür listedeki bağlı cihazlara uygulanır. */
  const actionTargets = selectedOnline.length > 0 ? selectedOnline : visible.filter((d) => d.online);
  const helpDevices = devices.filter((d) => d.helpRequested);
  const focusedDevice = devices.find((d) => d.deviceId === focused) ?? null;
  const menuDevice = devices.find((d) => d.deviceId === menu?.deviceId) ?? null;

  // --------------------------------------------------------------- eylemler

  const toggleSelect = useCallback((deviceId: string, additive: boolean) => {
    setSelected((prev) => {
      if (!additive) {
        // Düz tıklama: yalnızca bu cihaz seçili kalır (zaten tekse seçimi kaldırır).
        return prev.size === 1 && prev.has(deviceId) ? new Set() : new Set([deviceId]);
      }
      const next = new Set(prev);
      if (!next.delete(deviceId)) next.add(deviceId);
      return next;
    });
  }, []);

  const selectWhere = useCallback(
    (pred: (d: DeviceView) => boolean) => setSelected(new Set(visible.filter(pred).map((d) => d.deviceId))),
    [visible],
  );

  async function runSearch() {
    setAppliedQuery(query);
    setView("monitor");
  }

  async function openPairing() {
    const started = await guard(api.startPairing());
    if (started) {
      setPairing(started);
      setPairingOpen(true);
    }
  }

  async function openFocus(deviceId: string) {
    setFocused(deviceId);
    if (session) await guard(api.focusDevice(deviceId));
  }

  async function closeFocus() {
    // Pencere kapanırken kontrol de bırakılır: görmediğin ekranı sürmek
    // öğrenci için tehlikeli olurdu.
    if (controlled) await api.stopRemoteControl().catch(() => {});
    setFocused(null);
    if (session) await api.unfocusDevices().catch(() => {});
  }

  /**
   * Dosya seçer ve seçili öğrencilere gönderir.
   *
   * Seçim penceresi işletim sisteminin kendi iletişim kutusudur; öğretmen
   * neyi gönderdiğini görür. Gönderim arka planda yürür, durum
   * "Gönderimler" penceresinden izlenir.
   */
  async function pickAndSendFile() {
    const picked = await openFileDialog({
      multiple: false,
      directory: false,
      title: "Öğrencilere gönderilecek dosyayı seçin",
    });
    if (typeof picked !== "string") return;

    const ids = actionTargets.map((d) => d.deviceId);
    const ok = await guard(api.sendFile(ids, picked));
    if (ok !== undefined) {
      show("success", `Dosya ${ids.length} cihaza gönderiliyor.`);
      setTransfersOpen(true);
    }
  }

  async function toggleControl() {
    if (!focusedDevice) return;
    if (controlled === focusedDevice.deviceId) {
      await guard(api.stopRemoteControl(), "Kontrol bırakıldı.");
    } else {
      await guard(
        api.startRemoteControl(focusedDevice.deviceId),
        `${focusedDevice.hostname} kontrolü sizde.`,
      );
    }
  }

  async function startStreams() {
    const n = await guard(api.startScreenStreams(actionTargets.map((d) => d.deviceId), "preview"));
    if (n !== undefined) show("success", `${n} cihazda ekran yayını başladı.`);
  }

  async function stopStreams() {
    await guard(
      api.stopScreenStreams(actionTargets.map((d) => d.deviceId)),
      "Ekran yayınları durduruldu.",
    );
  }

  // ----------------------------------------------------------------- şerit

  const ribbonTabs: RibbonTab[] = useMemo(
    () => [
      {
        key: "class",
        label: "Sınıf",
        groups: [
          {
            caption: "Ders",
            commands: [
              session
                ? {
                    label: "Dersi Bitir",
                    icon: <Square size={18} />,
                    onClick: () => void guard(api.endSession(), "Ders bitirildi, yayınlar durdu."),
                  }
                : {
                    label: "Dersi Başlat",
                    icon: <Play size={18} />,
                    onClick: () => void guard(api.startSession(), "Ders başladı."),
                  },
            ],
          },
          {
            caption: "Cihazlar",
            commands: [
              { label: "Cihaz Eşleştir", icon: <Link2 size={18} />, onClick: openPairing },
              {
                label: "Grup Ekle",
                icon: <LayoutGrid size={18} />,
                onClick: () => setGroupOpen(true),
              },
            ],
          },
          {
            caption: "Ekranım",
            commands: [
              broadcast
                ? {
                    label: "Yayını Durdur",
                    icon: <ScreenShareOff size={18} />,
                    danger: true,
                    onClick: () =>
                      void guard(api.stopBroadcast(), "Ekran yayını durduruldu."),
                  }
                : {
                    label: "Ekranımı Yayınla",
                    icon: <ScreenShare size={18} />,
                    disabled: (info?.onlineCount ?? 0) === 0,
                    title:
                      (info?.onlineCount ?? 0) === 0
                        ? "Bağlı cihaz yok"
                        : "Ekranınız bağlı tüm öğrencilere gösterilir",
                    onClick: () => setBroadcastOpen(true),
                  },
            ],
          },
          {
            caption: "Ekran kilidi",
            commands: [
              {
                label: "Ekranı Kilitle",
                icon: <Lock size={18} />,
                danger: true,
                disabled: actionTargets.length === 0,
                title: "Seçili öğrencilerin klavye ve faresi kilitlenir",
                onClick: () => setLockOpen(true),
              },
              {
                label: "Kilidi Aç",
                icon: <LockOpen size={18} />,
                disabled: (info?.onlineCount ?? 0) === 0,
                title: "Seçim yoksa bağlı tüm cihazların kilidi açılır",
                onClick: async () => {
                  const ids = selectedDevices.map((d) => d.deviceId);
                  const n = await guard(api.unlockDevices(ids));
                  if (n !== undefined) show("success", `${n} cihazın kilidi açıldı.`);
                },
              },
            ],
          },
          {
            caption: "İletişim",
            commands: [
              {
                label: "Dosya Gönder",
                icon: <FileUp size={18} />,
                disabled: !session || actionTargets.length === 0,
                title: session
                  ? "Dosya öğrencinin Belgeler\\OpenClass klasörüne kaydedilir"
                  : "Önce ders oturumunu başlatın",
                onClick: pickAndSendFile,
              },
              {
                label: "Gönderimler",
                icon: <ListChecks size={18} />,
                onClick: () => setTransfersOpen(true),
              },
              {
                label: "Mesaj Gönder",
                icon: <MessageSquare size={18} />,
                disabled: actionTargets.length === 0,
                onClick: () => setMessageOpen(true),
              },
              {
                label:
                  helpDevices.length > 0
                    ? `Yardım (${helpDevices.length})`
                    : "Yardım Talepleri",
                icon: <Hand size={18} />,
                danger: helpDevices.length > 0,
                onClick: () => setView("help"),
                title:
                  helpDevices.length > 0
                    ? `${helpDevices.length} bekleyen talep`
                    : "Bekleyen talep yok",
              },
            ],
          },
        ],
      },
      {
        key: "monitor",
        label: "İzleme",
        groups: [
          {
            caption: "Ekran yayını",
            commands: [
              {
                label: "Ekranları Göster",
                icon: <Monitor size={18} />,
                disabled: !session || actionTargets.length === 0,
                title: session ? undefined : "Önce ders oturumunu başlatın",
                onClick: startStreams,
              },
              {
                label: "Yayını Durdur",
                icon: <MonitorOff size={18} />,
                disabled: actionTargets.length === 0,
                onClick: stopStreams,
              },
              {
                label: "Büyüt",
                icon: <Maximize2 size={18} />,
                disabled: selectedDevices.length !== 1,
                title: "Tek bir cihaz seçin",
                onClick: () => selectedDevices[0] && void openFocus(selectedDevices[0].deviceId),
              },
            ],
          },
          {
            caption: "Liste",
            commands: [
              {
                label: "Küçük Resim",
                icon: <LayoutGrid size={18} />,
                active: listMode === "thumbnails",
                onClick: () => setListMode("thumbnails"),
              },
              {
                label: "Ayrıntı",
                icon: <Rows3 size={18} />,
                active: listMode === "details",
                onClick: () => setListMode("details"),
              },
              {
                label: "Yenile",
                icon: <RotateCcw size={18} />,
                onClick: () => void refreshDevices(),
              },
            ],
          },
        ],
      },
      {
        key: "select",
        label: "Seçim",
        groups: [
          {
            caption: "Seç",
            commands: [
              {
                label: "Tümü",
                icon: <CheckSquare size={18} />,
                onClick: () => selectWhere(() => true),
              },
              {
                label: "Bağlı Olanlar",
                icon: <Wifi size={18} />,
                onClick: () => selectWhere((d) => d.online),
              },
              {
                label: "Yardım İsteyenler",
                icon: <Hand size={18} />,
                disabled: helpDevices.length === 0,
                onClick: () => selectWhere((d) => d.helpRequested),
              },
              {
                label: "Temizle",
                icon: <SquareDashed size={18} />,
                disabled: selected.size === 0,
                onClick: () => setSelected(new Set()),
              },
            ],
          },
        ],
      },
      {
        key: "admin",
        label: "Yönetim",
        groups: [
          {
            caption: "Cihaz",
            commands: [
              {
                label: "Cihaz Yönetimi",
                icon: <UserRoundCog size={18} />,
                onClick: () => setView("devices"),
              },
              {
                label: "Yetkiyi Kaldır",
                icon: <ShieldOff size={18} />,
                danger: true,
                disabled: selectedDevices.length === 0,
                onClick: async () => {
                  for (const d of selectedDevices) {
                    await guard(api.revokeDevice(d.deviceId));
                  }
                  show("success", `${selectedDevices.length} cihazın yetkisi kaldırıldı.`);
                  setSelected(new Set());
                  void refreshDevices();
                },
              },
            ],
          },
          {
            caption: "Dosyalar",
            commands: [
              {
                label: "Dosya Gezgini",
                icon: <FolderOpen size={18} />,
                disabled: !session || selectedDevices.length !== 1,
                title: !session
                  ? "Önce ders oturumunu başlatın"
                  : selectedDevices.length !== 1
                    ? "Tek bir cihaz seçin"
                    : "Öğrencinin dosyalarını görüntüleyin (yalnızca okuma)",
                onClick: async () => {
                  const device = selectedDevices[0];
                  if (!device) return;
                  const ok = await guard(api.openBrowser(device.deviceId));
                  if (ok !== undefined) setBrowserOpen(true);
                },
              },
            ],
          },
          {
            caption: "Sistem",
            commands: [
              {
                label: "Ayarlar",
                icon: <SettingsIcon size={18} />,
                onClick: () => setSettingsOpen(true),
              },
              {
                label: "Denetim Kayıtları",
                icon: <ClipboardList size={18} />,
                onClick: () => setAuditOpen(true),
              },
            ],
          },
        ],
      },
    ],
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [session, actionTargets.length, selectedDevices, helpDevices.length, listMode, selected.size, selectWhere, broadcast, info?.onlineCount],
  );

  const contextItems: (MenuItem | null)[] = menuDevice
    ? [
        {
          label: "Ekranı büyüt",
          icon: <Maximize2 size={13} />,
          disabled: !menuDevice.online,
          onSelect: () => void openFocus(menuDevice.deviceId),
        },
        {
          label: menuDevice.streaming ? "Yayını durdur" : "Ekranı göster",
          icon: menuDevice.streaming ? <MonitorOff size={13} /> : <Monitor size={13} />,
          disabled: !menuDevice.online || (!session && !menuDevice.streaming),
          onSelect: () => {
            if (menuDevice.streaming) {
              void guard(api.stopScreenStreams([menuDevice.deviceId]));
            } else {
              void guard(api.startScreenStreams([menuDevice.deviceId], "preview"));
            }
          },
        },
        null,
        {
          label: "Kontrolü al",
          icon: <MousePointerClick size={13} />,
          disabled: !menuDevice.online,
          onSelect: async () => {
            // Kontrol büyütülmüş görünümde yürür: öğretmen sürdüğü ekranı
            // görmeden tıklamamalı.
            await openFocus(menuDevice.deviceId);
            await guard(
              api.startRemoteControl(menuDevice.deviceId),
              `${menuDevice.hostname} kontrolü sizde.`,
            );
          },
        },
        {
          label: "Ekranı kilitle",
          icon: <Lock size={13} />,
          disabled: !menuDevice.online,
          onSelect: () => {
            setSelected(new Set([menuDevice.deviceId]));
            setLockOpen(true);
          },
        },
        {
          label: "Kilidi aç",
          icon: <LockOpen size={13} />,
          disabled: !menuDevice.online,
          onSelect: () => void guard(api.unlockDevices([menuDevice.deviceId])),
        },
        {
          label: "Dosyalarını görüntüle",
          icon: <FolderOpen size={13} />,
          disabled: !menuDevice.online,
          onSelect: async () => {
            const ok = await guard(api.openBrowser(menuDevice.deviceId));
            if (ok !== undefined) setBrowserOpen(true);
          },
        },
        null,
        {
          label: "Mesaj gönder",
          icon: <MessageSquare size={13} />,
          disabled: !menuDevice.online,
          onSelect: () => {
            setSelected(new Set([menuDevice.deviceId]));
            setMessageOpen(true);
          },
        },
        {
          label: "Yardım talebini kapat",
          icon: <Hand size={13} />,
          disabled: !menuDevice.helpRequested,
          onSelect: () => void guard(api.clearHelpRequest(menuDevice.deviceId)),
        },
        null,
        {
          label: "Yetkiyi kaldır",
          icon: <ShieldOff size={13} />,
          danger: true,
          disabled: menuDevice.authorization !== "authorized",
          onSelect: async () => {
            await guard(api.revokeDevice(menuDevice.deviceId), `${menuDevice.hostname} yetkisi kaldırıldı.`);
            void refreshDevices();
          },
        },
      ]
    : [];

  const listProps = {
    devices: visible,
    selected,
    onSelect: toggleSelect,
    onOpen: (id: string) => void openFocus(id),
    onContextMenu: (deviceId: string, x: number, y: number) => setMenu({ deviceId, x, y }),
    emptyTitle: appliedQuery
      ? `"${appliedQuery}" ile eşleşen bilgisayar yok`
      : groupFilter
        ? `${groupFilter.name} grubunda cihaz yok`
        : "Henüz cihaz yok",
    emptyDetail: appliedQuery
      ? "Ön eki kontrol edin. Arama büyük/küçük harf duyarsızdır."
      : "Öğrenci bilgisayarlarını eşleştirmek için Sınıf › Cihaz Eşleştir adımını kullanın.",
  };

  // ------------------------------------------------------------------ çizim

  return (
    <div className="flex h-full flex-col bg-[var(--color-canvas)]">
      {/* Başlık çubuğu */}
      <header className="flex h-[38px] shrink-0 items-center gap-3 border-b
        border-[var(--color-stroke)] bg-[var(--color-chrome)] px-3">
        <span className="flex items-center gap-1.5">
          <Cast size={16} className="text-[var(--color-accent)]" />
          <span className="text-[12.5px] font-semibold tracking-tight">OpenClass</span>
        </span>

        <span className="h-4 w-px bg-[var(--color-stroke-strong)]" />

        <span className="truncate text-[12px]">
          {session ? (
            <>
              <strong className="font-semibold">{session.className}</strong>
              <span className="ml-2 tabular-nums text-[var(--color-text-2)]">
                {formatDuration(session.startedAt)}
              </span>
            </>
          ) : (
            <span className="text-[var(--color-text-2)]">
              {info?.className ?? "Laboratuvar"} · ders kapalı
            </span>
          )}
        </span>

        <div className="ml-auto flex items-center gap-1.5">
          {broadcast && (
            <span className="flex h-[24px] items-center gap-1.5 rounded bg-[var(--color-ok)]
              px-2 text-[11px] font-semibold text-white">
              <ScreenShare size={12} /> Ekranınız yayınlanıyor
            </span>
          )}
          {helpDevices.length > 0 && (
            <button
              onClick={() => setView("help")}
              className="flex h-[24px] items-center gap-1.5 rounded bg-[var(--color-warn)]
                px-2 text-[11px] font-semibold text-white"
            >
              <Hand size={12} /> {helpDevices.length} yardım talebi
            </button>
          )}
          {session ? (
            <Button
              tone="danger"
              onClick={() => void guard(api.endSession(), "Ders bitirildi.")}
            >
              <Square size={12} /> Dersi Bitir
            </Button>
          ) : (
            <Button tone="accent" onClick={() => void guard(api.startSession(), "Ders başladı.")}>
              <Play size={12} /> Dersi Başlat
            </Button>
          )}
          <IconButton label="Temayı değiştir" onClick={toggle}>
            {theme === "dark" ? <Sun size={14} /> : <Moon size={14} />}
          </IconButton>
        </div>
      </header>

      {/* Ağ servisleri başlatılamadıysa kalıcı şerit. Geçici bildirim yetmiyor:
          bu durumda konsol sağlıklı görünür ama hiçbir öğrenci bağlanamaz. */}
      {info?.networkError && (
        <div
          role="alert"
          className="flex shrink-0 items-start gap-2 border-b border-[var(--color-danger)]
            bg-[var(--color-danger)] px-3 py-1.5 text-[11.5px] leading-snug text-white"
        >
          <AlertTriangle size={14} className="mt-px shrink-0" />
          <span data-selectable>{info.networkError}</span>
        </div>
      )}

      <Ribbon tabs={ribbonTabs} activeTab={ribbonTab} onTabChange={setRibbonTab} />

      <GroupBar
        tabs={groupTabs}
        activeGroup={activeGroup}
        onSelect={(name) => {
          setActiveGroup(name);
          setSelected(new Set());
        }}
        onAdd={() => setGroupOpen(true)}
        onRemove={async (name) => {
          await guard(api.deleteGroup(name), `${name} grubu kaldırıldı.`);
          if (activeGroup === name) setActiveGroup(null);
          void refreshGroups();
        }}
      />

      {/* Arama satırı */}
      <div className="flex h-[38px] shrink-0 items-center gap-2 border-b
        border-[var(--color-stroke)] bg-[var(--color-surface)] px-2">
        <TextInput
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") void runSearch();
          }}
          placeholder="Bilgisayar adı veya IP girin (Örn: LAB1 · 192.168.1.)"
          className="h-[28px] max-w-[460px] flex-1"
          aria-label="Bilgisayar ara"
        />
        <Button tone="accent" className="!h-[28px]" onClick={runSearch}>
          Bilgisayarları Ara
        </Button>
        {appliedQuery && (
          <Button
            className="!h-[28px]"
            onClick={() => {
              setQuery("");
              setAppliedQuery("");
            }}
          >
            Filtreyi kaldır
          </Button>
        )}
        <span className="ml-auto pr-1 text-[11px] text-[var(--color-text-3)]">
          {visible.length} cihaz gösteriliyor
        </span>
      </div>

      {/* Ana alan: sol görünüm rayı + içerik */}
      <div className="flex min-h-0 flex-1">
        <nav className="flex w-[74px] shrink-0 flex-col gap-0.5 border-r
          border-[var(--color-stroke)] bg-[var(--color-surface)] p-1">
          {VIEW_MODES.map(({ key, label, icon: Icon }) => {
            const active = view === key;
            const badge = key === "help" ? helpDevices.length : 0;
            return (
              <button
                key={key}
                onClick={() => setView(key)}
                className={`relative flex h-[54px] flex-col items-center justify-center gap-1
                  rounded text-[10.5px] transition-colors
                  ${
                    active
                      ? "bg-[var(--color-accent-soft)] font-semibold text-[var(--color-accent)]"
                      : "text-[var(--color-text-2)] hover:bg-[var(--color-surface-3)]"
                  }`}
              >
                <Icon size={18} />
                {label}
                {badge > 0 && (
                  <span className="absolute right-1.5 top-1.5 rounded-full bg-[var(--color-warn)]
                    px-1 text-[9px] font-bold text-white">
                    {badge}
                  </span>
                )}
              </button>
            );
          })}
        </nav>

        <main className="min-w-0 flex-1 bg-[var(--color-surface)]">
          {view === "monitor" &&
            (listMode === "thumbnails" ? (
              <ThumbnailGrid {...listProps} frameBaseUrl={frameBaseUrl} size={thumbSize} />
            ) : (
              <DetailsTable {...listProps} />
            ))}

          {view === "help" && (
            <HelpView devices={helpDevices} onRefresh={refreshDevices} onOpen={openFocus} />
          )}

          {view === "devices" && (
            <DeviceAdminView devices={devices} onRefresh={refreshDevices} />
          )}

          {view === "audit" && <AuditView />}
        </main>
      </div>

      <StatusBar
        className={session ? session.className : (info?.className ?? "—")}
        sessionActive={session !== null}
        connected={info?.onlineCount ?? devices.filter((d) => d.online).length}
        selected={selected.size}
        inGroup={visible.length}
        helpCount={helpDevices.length}
        broadcast={broadcast}
        mode={listMode}
        onModeChange={setListMode}
        thumbSize={thumbSize}
        onThumbSizeChange={setThumbSize}
        onSelectOnline={() => selectWhere((d) => d.online)}
        onSelectOffline={() => selectWhere((d) => !d.online)}
        onSelectHelp={() => selectWhere((d) => d.helpRequested)}
      />

      {/* Diyaloglar */}
      <PairingDialog
        open={pairingOpen}
        pairing={pairing}
        info={info}
        onClose={() => {
          setPairingOpen(false);
          void api.cancelPairing().catch(() => {});
        }}
        onRegenerate={openPairing}
      />
      <MessageDialog
        open={messageOpen}
        targets={actionTargets}
        onClose={() => setMessageOpen(false)}
        onSent={refreshDevices}
      />
      <GroupDialog
        open={groupOpen}
        existing={groups}
        onClose={() => setGroupOpen(false)}
        onSaved={refreshGroups}
      />
      <SettingsDialog
        open={settingsOpen}
        info={info}
        onClose={() => setSettingsOpen(false)}
        onSaved={refreshInfo}
      />
      <AuditDialog open={auditOpen} onClose={() => setAuditOpen(false)} />
      <BroadcastDialog
        open={broadcastOpen}
        onClose={() => setBroadcastOpen(false)}
        onStart={async (fullscreen) => {
          setBroadcastOpen(false);
          await guard(
            api.startBroadcast(fullscreen),
            fullscreen
              ? "Ekranınız tam ekran yayınlanıyor."
              : "Ekranınız pencereli olarak yayınlanıyor.",
          );
        }}
      />

      <BrowserDialog
        open={browserOpen}
        onClose={() => setBrowserOpen(false)}
      />

      <TransfersDialog
        open={transfersOpen}
        devices={devices}
        onClose={() => setTransfersOpen(false)}
      />

      <LockDialog
        open={lockOpen}
        count={actionTargets.length}
        onClose={() => setLockOpen(false)}
        onLock={async (message) => {
          setLockOpen(false);
          const ids = actionTargets.map((d) => d.deviceId);
          const n = await guard(api.lockDevices(ids, message));
          if (n !== undefined) show("success", `${n} cihazın ekranı kilitlendi.`);
        }}
      />

      <FocusDialog
        device={focusedDevice}
        frameBaseUrl={frameBaseUrl}
        sessionActive={session !== null}
        controlled={controlled !== null && controlled === focusedDevice?.deviceId}
        onToggleControl={toggleControl}
        onClose={closeFocus}
      />

      {menu && (
        <ContextMenu x={menu.x} y={menu.y} items={contextItems} onClose={() => setMenu(null)} />
      )}
    </div>
  );
}

// --------------------------------------------------------------- alt görünümler

function HelpView({
  devices,
  onRefresh,
  onOpen,
}: {
  devices: DeviceView[];
  onRefresh: () => void;
  onOpen: (id: string) => void;
}) {
  const { guard } = useToast();

  if (devices.length === 0) {
    return (
      <Empty
        title="Bekleyen yardım talebi yok"
        detail="Öğrenciler Yardım İste'ye bastığında talepler burada toplanır."
      />
    );
  }

  return (
    <div className="h-full overflow-auto p-2">
      <ul className="space-y-1">
        {devices.map((d) => (
          <li
            key={d.deviceId}
            className="hairline flex items-center gap-3 rounded border
              border-[var(--color-warn)]/40 bg-[var(--color-warn)]/5 px-3 py-2"
          >
            <Hand size={16} className="shrink-0 text-[var(--color-warn)]" />
            <div className="min-w-0 flex-1">
              <p className="truncate text-[12.5px] font-medium">{d.hostname}</p>
              <p className="truncate text-[11px] text-[var(--color-text-3)]">
                {d.displayName ?? d.address ?? "—"}
              </p>
            </div>
            <Button onClick={() => onOpen(d.deviceId)} disabled={!d.online}>
              Ekranını aç
            </Button>
            <Button
              tone="accent"
              onClick={async () => {
                await guard(api.clearHelpRequest(d.deviceId));
                onRefresh();
              }}
            >
              Görüldü
            </Button>
          </li>
        ))}
      </ul>
    </div>
  );
}

function DeviceAdminView({
  devices,
  onRefresh,
}: {
  devices: DeviceView[];
  onRefresh: () => void;
}) {
  const { guard } = useToast();
  const known = devices.filter((d) => d.authorization !== "unknown");

  if (known.length === 0) {
    return (
      <Empty
        title="Kayıtlı cihaz yok"
        detail="Sınıf › Cihaz Eşleştir ile öğrenci bilgisayarlarını ekleyin."
      />
    );
  }

  return (
    <div className="h-full overflow-auto">
      <table className="w-full border-collapse text-[12px]">
        <thead className="sticky top-0 bg-[var(--color-surface-2)] text-left text-[11px]
          text-[var(--color-text-2)]">
          <tr className="border-b border-[var(--color-stroke)]">
            <th className="w-[180px] px-2.5 py-1.5 font-medium">Bilgisayar</th>
            <th className="w-[220px] px-2.5 py-1.5 font-medium">Görünen ad</th>
            <th className="w-[120px] px-2.5 py-1.5 font-medium">Durum</th>
            <th className="w-[140px] px-2.5 py-1.5 font-medium">Eşleştirme</th>
            <th className="px-2.5 py-1.5 font-medium">Son görülme</th>
            <th className="w-[150px] px-2.5 py-1.5" />
          </tr>
        </thead>
        <tbody>
          {known.map((d) => (
            <tr key={d.deviceId} className="border-b border-[var(--color-stroke)]">
              <td className="px-2.5 py-1 font-medium">{d.hostname}</td>
              <td className="px-2.5 py-1">
                <TextInput
                  defaultValue={d.displayName ?? ""}
                  placeholder="—"
                  className="w-full !border-transparent hover:!border-[var(--color-stroke-strong)]"
                  onBlur={async (e) => {
                    const next = e.target.value.trim();
                    if (next === (d.displayName ?? "")) return;
                    await guard(api.setDisplayName(d.deviceId, next || null));
                    onRefresh();
                  }}
                />
              </td>
              <td className="px-2.5 py-1 text-[var(--color-text-2)]">
                {d.online ? "Bağlı" : d.discovered ? "Ağda" : "Çevrimdışı"}
              </td>
              <td className="px-2.5 py-1 text-[var(--color-text-2)]">
                {d.authorization === "authorized"
                  ? "Yetkili"
                  : d.authorization === "pending"
                    ? "Bekliyor"
                    : "Kaldırıldı"}
              </td>
              <td className="px-2.5 py-1 tabular-nums text-[var(--color-text-3)]">
                {formatTime(d.lastSeen)}
              </td>
              <td className="px-2.5 py-1 text-right">
                {d.authorization === "authorized" && (
                  <Button
                    tone="subtle"
                    className="!text-[var(--color-danger)]"
                    onClick={async () => {
                      await guard(api.revokeDevice(d.deviceId), `${d.hostname} yetkisi kaldırıldı.`);
                      onRefresh();
                    }}
                  >
                    <ShieldOff size={12} /> Yetkiyi kaldır
                  </Button>
                )}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function AuditView() {
  const { guard } = useToast();
  const [entries, setEntries] = useState<AuditEntry[]>([]);
  const loaded = useRef(false);

  const load = useCallback(async () => {
    const list = await guard(api.getAuditLog(300));
    if (list) setEntries(list);
  }, [guard]);

  useEffect(() => {
    if (loaded.current) return;
    loaded.current = true;
    void load();
  }, [load]);

  return (
    <Panel
      title="Denetim kayıtları"
      className="m-2 h-[calc(100%-1rem)] !rounded"
      actions={
        <Button tone="subtle" onClick={load}>
          <RotateCcw size={12} /> Yenile
        </Button>
      }
    >
      {entries.length === 0 ? (
        <Empty title="Henüz kayıt yok" />
      ) : (
        <ul className="h-full divide-y divide-[var(--color-stroke)] overflow-auto text-[11.5px]">
          {entries.map((e, i) => (
            <li key={`${e.at}-${i}`} className="flex gap-3 px-3 py-1">
              <span className="shrink-0 font-mono tabular-nums text-[var(--color-text-3)]">
                {formatTime(e.at)}
              </span>
              <span className="w-[190px] shrink-0 font-medium">
                {AUDIT_LABELS[e.kind] ?? e.kind}
              </span>
              <span className="truncate text-[var(--color-text-2)]">{e.detail ?? ""}</span>
            </li>
          ))}
        </ul>
      )}
    </Panel>
  );
}
