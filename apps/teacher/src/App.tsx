import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  AlertTriangle,
  Cast,
  CheckSquare,
  ClipboardList,
  Hand,
  FileArchive,
  FileDown,
  FileUp,
  Keyboard,
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
  Power,
  RotateCcw,
  Rows3,
  ScreenShare,
  ScreenShareOff,
  KeyRound,
  Settings as SettingsIcon,
  ShieldCheck,
  ShieldOff,
  SquareDashed,
  Sun,
  UserRoundCog,
  Wifi,
  WifiOff,
} from "lucide-react";

import { api, AUDIT_LABELS, formatTime } from "./api";
import {
  AuditDialog,
  BroadcastDialog,
  CollectDialog,
  FocusDialog,
  LockDialog,
  RoomPickerDialog,
  GroupDialog,
  MessageDialog,
  PairingDialog,
  SettingsDialog,
  ShutdownDialog,
} from "./dialogs";
import { BrowseDialog } from "./BrowseDialog";
import { open as openFileDialog, open as openFolderDialog } from "@tauri-apps/plugin-dialog";
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
  Modal,
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
  DeviceGroup,
  DeviceView,
  FrameEvent,
  HelpEvent,
  KeyEvent,
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
  const [info, setInfo] = useState<ServerInfo | null>(null);
  const [frameBaseUrl, setFrameBaseUrl] = useState("");
  const [broadcast, setBroadcast] = useState<BroadcastInfo | null>(null);
  const [controlled, setControlled] = useState<string | null>(null);
  const [broadcastOpen, setBroadcastOpen] = useState(false);
  const [lockOpen, setLockOpen] = useState(false);
  const [collectOpen, setCollectOpen] = useState(false);
  const [shutdownTarget, setShutdownTarget] = useState<DeviceView | null>(null);
  const [transfersOpen, setTransfersOpen] = useState(false);
  // Uzak dosya gezgini: "Dosya Al" yalnızca tek seçili öğrenciyle açılır.
  const [browseOpen, setBrowseOpen] = useState(false);
  const [browseDevice, setBrowseDevice] = useState<DeviceView | null>(null);

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

  // Klavye izleme: seçili tek cihazın tuş basımlarını canlı gösterir.
  const [keyMonitorOpen, setKeyMonitorOpen] = useState(false);
  const [keyMonitorDevice, setKeyMonitorDevice] = useState<DeviceView | null>(null);
  const [keyMonitoring, setKeyMonitoring] = useState(false);
  const [keyEvents, setKeyEvents] = useState<KeyLogEntry[]>([]);
  // Dinleyici kapanağı taze tutmak yerine hedef cihazı ref'te izleriz; olay
  // dinleyicisi bir kez kurulur ve daima güncel hedefe bakar.
  const monitorTargetRef = useRef<string | null>(null);
  const keyEventSeqRef = useRef(0);

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
      // Klavye izleme: yalnızca o an izlenen cihazın basımları listeye girer.
      listen<KeyEvent>("key-event", (e) => {
        const { deviceId, code, pressed, ts } = e.payload;
        if (monitorTargetRef.current !== deviceId || !pressed) return;
        setKeyEvents((prev) => [
          ...prev.slice(-199),
          { id: keyEventSeqRef.current++, label: keyLabel(code), code, ts },
        ]);
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
   * Arama bilgisayar adında (ön ek), oda adında ve IP adresinde yapılır;
   * öğretmen hangisini yazdığını belirtmek zorunda değil.
   */
  const visible = useMemo(() => {
    const search = appliedQuery.trim().toUpperCase();
    const oda = info?.activeRoom?.trim().toUpperCase() ?? "";
    const onek = info?.activePrefix?.trim().toUpperCase() ?? "";
    return devices.filter((d) => {
      // Laboratuvar seçiliyse yalnızca o odanın bilgisayarları listelenir.
      // Odası bilinmeyen cihazlar dışarıda kalır: hangi laboratuvarda
      // oldukları bilinmediği için "bu odada" demek doğru olmaz.
      if (oda && (d.room ?? "").trim().toUpperCase() !== oda) return false;
      // Ön ek süzmesi oda süzmesinden bağımsızdır; ikisi birlikte verilirse
      // ikisi birden uygulanır.
      if (onek && !d.hostname.toUpperCase().startsWith(onek)) return false;
      if (!inGroup(d, groupFilter)) return false;
      if (!search) return true;
      return (
        d.hostname.toUpperCase().startsWith(search) ||
        (d.room ?? "").toUpperCase().includes(search) ||
        (d.address ?? "").toUpperCase().includes(search)
      );
    });
  }, [devices, groupFilter, appliedQuery, info?.activeRoom, info?.activePrefix]);

  const groupTabs: GroupTab[] = useMemo(() => {
    const count = (group: DeviceGroup | null) => {
      const members = devices.filter((d) => inGroup(d, group));
      return { total: members.length, online: members.filter((d) => d.online).length };
    };
    return [
      { group: null, ...count(null) },
      ...groups.map((g) => ({ group: g, ...count(g) })),
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
    await guard(api.focusDevice(deviceId));
  }

  async function closeFocus() {
    // Pencere kapanırken kontrol de bırakılır: görmediğin ekranı sürmek
    // öğrenci için tehlikeli olurdu.
    if (controlled) await api.stopRemoteControl().catch(() => {});
    setFocused(null);
    await api.unfocusDevices().catch(() => {});
  }

  /**
   * Gerçek Windows iletişim kutularıyla dosya gönderir.
   *
   * İki kutu art arda açılır — ikisi de işletim sisteminin kendi gezginidir
   * (Tauri `plugin-dialog` native pencereyi açar):
   *   1. Gönderilecek yerel dosya seçilir.
   *   2. Hedef klasör seçilir; seçilen yol öğrencinin kullanıcı klasöründeki
   *      karşılığına göreli olarak çözülür (Belgeler/Masaüstü/İndirilenler).
   * Seçim iptal edilirse gönderim başlamaz. Sonuç Gönderimler listesinde izlenir.
   */
  async function pickAndSendFile() {
    const hedefler = actionTargets;
    if (hedefler.length === 0) {
      show("error", "Dosya gönderilecek bağlı cihaz yok.");
      return;
    }

    // 1) Gönderilecek dosya (işletim sisteminin native gezgini).
    const picked = await openFileDialog({
      multiple: false,
      directory: false,
      title: "Öğrencilere gönderilecek dosyayı seçin",
    });
    if (typeof picked !== "string") return;

    // 2) Hedef klasör (native gezgin). Öğrencinin kullanıcı klasörüne
    //    göreli çözülür; Öğretmen kendi makinesinde bir klasör seçer.
    const hedef = await openFolderDialog({
      directory: true,
      multiple: false,
      title: "Öğrencinin hangi klasörüne göndersin?",
    });
    if (typeof hedef !== "string") return;

    const resolved = resolveDestination(hedef);
    if (!resolved) {
      show(
        "error",
        "Bu klasör öğrenciyle paylaşılan konumlardan değil. Belgeler, Masaüstü veya İndirilenler içinde bir klasör seçin.",
      );
      return;
    }

    const ids = hedefler.map((d) => d.deviceId);
    const ok = await guard(
      api.sendFile(ids, picked, resolved.destination, resolved.subpath),
    );
    if (ok !== undefined) {
      show("success", `Dosya ${ids.length} cihaza gönderilecek: ${resolved.shown}`);
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

  async function openKeyMonitor() {
    const device = selectedDevices[0];
    if (!device) {
      show("error", "Klavye izleme için tek bir öğrenci seçin.");
      return;
    }
    setKeyMonitorDevice(device);
    setKeyEvents([]);
    setKeyMonitoring(false);
    monitorTargetRef.current = device.deviceId;
    setKeyMonitorOpen(true);
  }

  async function toggleKeyMonitor() {
    if (!keyMonitorDevice) return;
    if (keyMonitoring) {
      await guard(
        api.stopKeyMonitor(keyMonitorDevice.deviceId),
        "Klavye izleme durduruldu.",
      );
      setKeyMonitoring(false);
    } else {
      setKeyEvents([]);
      await guard(api.startKeyMonitor(keyMonitorDevice.deviceId));
      setKeyMonitoring(true);
    }
  }

  async function closeKeyMonitor() {
    // İzleme açık kalmasın: pencere kapanınca öğrenci tarafındaki kanca da
    // durur (bağlantı koparsa kod yine biter, ama temiz kapatmak daha iyi).
    if (keyMonitoring && keyMonitorDevice) {
      await api.stopKeyMonitor(keyMonitorDevice.deviceId).catch(() => {});
    }
    setKeyMonitoring(false);
    monitorTargetRef.current = null;
    setKeyMonitorOpen(false);
    setKeyMonitorDevice(null);
    setKeyEvents([]);
  }

  // ----------------------------------------------------------------- şerit

  const ribbonTabs: RibbonTab[] = useMemo(
    () => [
      {
        key: "class",
        label: "Sınıf",
        groups: [
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
            // Gönderme ve alma aynı işin iki yönü; şeritte de yan yana
            // durmaları gerekir. 0.1.0'da alma ("Dosya Gezgini") başka
            // sekmedeydi ve kimse bulamıyordu.
            caption: "Dosyalar",
            commands: [
              {
                label: "Dosya Gönder",
                icon: <FileUp size={18} />,
                disabled: actionTargets.length === 0,
                title: "Seçili öğrencilere dosya gönder",
                onClick: pickAndSendFile,
              },
              {
                label: "Topla",
                icon: <FileArchive size={18} />,
                disabled: actionTargets.length === 0,
                title: "Seçili öğrencilerden dosya(ları) topla",
                onClick: () => setCollectOpen(true),
              },
              {
                label: "Dosya Al",
                icon: <FileDown size={18} />,
                disabled: selectedDevices.length !== 1 || selectedDevices[0]?.online !== true,
                title:
                  selectedDevices.length !== 1
                    ? "Tek bir öğrenci seçin"
                    : "Seçili öğrencinin dosyalarını gezip indirir",
                onClick: () => {
                  const d = selectedDevices[0];
                  if (!d || !d.online) return;
                  setBrowseDevice(d);
                  setBrowseOpen(true);
                },
              },
              {
                label: "Gönderimler",
                icon: <ListChecks size={18} />,
                onClick: () => setTransfersOpen(true),
              },
            ],
          },
          {
            caption: "İletişim",
            commands: [
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
          {
            caption: "Sistem",
            commands: [
              {
                label: "İnterneti Kes",
                icon: <WifiOff size={18} />,
                disabled: actionTargets.length === 0,
                title:
                  actionTargets.length === 0
                    ? "Öğrenci seçin"
                    : "Seçili öğrencilerin internete çıkışını keser (öğretmen bağlantısı açık kalır)",
                onClick: () =>
                  void (async () => {
                    const ids = actionTargets.map((d) => d.deviceId);
                    const n = await guard(api.blockInternet(ids, true));
                    if (n !== undefined) show("success", `${n} öğrencinin interneti kesildi.`);
                  })(),
              },
              {
                label: "İnterneti Aç",
                icon: <Wifi size={18} />,
                disabled: actionTargets.length === 0,
                title:
                  actionTargets.length === 0
                    ? "Öğrenci seçin"
                    : "Seçili öğrencilerin internetini geri açar",
                onClick: () =>
                  void (async () => {
                    const ids = actionTargets.map((d) => d.deviceId);
                    const n = await guard(api.blockInternet(ids, false));
                    if (n !== undefined) show("success", `${n} öğrencinin interneti açıldı.`);
                  })(),
              },
              {
                label: "Bilgisayarı Kapat",
                icon: <Power size={18} />,
                danger: true,
                disabled: selectedDevices.length !== 1,
                title:
                  selectedDevices.length === 1
                    ? "Seçili öğrencinin bilgisayarını kapat"
                    : "Tek bir cihaz seçin",
                onClick: () => {
                  const d = selectedDevices[0];
                  if (d) setShutdownTarget(d);
                },
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
                disabled: actionTargets.length === 0,
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
            caption: "Klavye izleme",
            commands: [
              {
                label: "Klavye İzle",
                icon: <KeyRound size={18} />,
                disabled: selectedDevices.length !== 1,
                title:
                  selectedDevices.length === 1
                    ? "Seçili öğrencinin tuş basımlarını canlı göster"
                    : "Tek bir cihaz seçin",
                onClick: () => void openKeyMonitor(),
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
            caption: "Sistem",
            commands: [
              {
                label: "İnterneti Kes",
                icon: <WifiOff size={18} />,
                disabled: actionTargets.length === 0,
                title:
                  actionTargets.length === 0
                    ? "Öğrenci seçin"
                    : "Seçili öğrencilerin internete çıkışını keser (öğretmen bağlantısı açık kalır)",
                onClick: () =>
                  void (async () => {
                    const ids = actionTargets.map((d) => d.deviceId);
                    const n = await guard(api.blockInternet(ids, true));
                    if (n !== undefined) show("success", `${n} öğrencinin interneti kesildi.`);
                  })(),
              },
              {
                label: "İnterneti Aç",
                icon: <Wifi size={18} />,
                disabled: actionTargets.length === 0,
                title:
                  actionTargets.length === 0
                    ? "Öğrenci seçin"
                    : "Seçili öğrencilerin internetini geri açar",
                onClick: () =>
                  void (async () => {
                    const ids = actionTargets.map((d) => d.deviceId);
                    const n = await guard(api.blockInternet(ids, false));
                    if (n !== undefined) show("success", `${n} öğrencinin interneti açıldı.`);
                  })(),
              },
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
              {
                label: "Bilgisayarı Kapat",
                icon: <Power size={18} />,
                danger: true,
                disabled: selectedDevices.length !== 1,
                title:
                  selectedDevices.length === 1
                    ? "Seçili öğrencinin bilgisayarını kapat"
                    : "Tek bir cihaz seçin",
                onClick: () => {
                  const d = selectedDevices[0];
                  if (d) setShutdownTarget(d);
                },
              },
            ],
          },
        ],
      },
    ],
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [actionTargets.length, selectedDevices, helpDevices.length, listMode, selected.size, selectWhere, broadcast, info?.onlineCount],
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
          disabled: !menuDevice.online,
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
          label: "Alınan klasörü aç",
          icon: <FileDown size={13} />,
          onSelect: () => void guard(api.openDownloadsDir()),
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
          label: "Yetkilendir",
          icon: <ShieldCheck size={13} />,
          disabled:
            menuDevice.authorization === "authorized" || !menuDevice.online,
          onSelect: async () => {
            await guard(
              api.authorizeDevice(menuDevice.deviceId),
              `${menuDevice.hostname} yetkilendirildi.`,
            );
            void refreshDevices();
          },
        },
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
          <strong className="font-semibold">{info?.className ?? "Laboratuvar"}</strong>
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
          <button
            onClick={openKeyMonitor}
            className="flex h-[54px] flex-col items-center justify-center gap-1
              rounded text-[10.5px] text-[var(--color-text-2)] transition-colors
              hover:bg-[var(--color-surface-3)]"
            title={
              selectedDevices.length === 1
                ? "Seçili öğrencinin klavyesini izle"
                : "Klavye izleme (tek cihaz seçin)"
            }
          >
            <Keyboard size={18} />
            Klavye
          </button>
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
        className={info?.className ?? "—"}
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
      <CollectDialog
        open={collectOpen}
        targets={actionTargets}
        onClose={() => setCollectOpen(false)}
        onCollected={refreshDevices}
      />
      <BrowseDialog
        open={browseOpen}
        device={browseDevice}
        onClose={() => setBrowseOpen(false)}
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

      <TransfersDialog
        open={transfersOpen}
        devices={devices}
        onClose={() => setTransfersOpen(false)}
      />

      <RoomPickerDialog
        open={info !== null && !info.setupCompleted}
        devices={devices}
        onPick={async ({ room, prefix }) => {
          await guard(
            api.updateSettings({
              activeRoom: room,
              activePrefix: prefix,
              setupCompleted: true,
            }),
          );
          setInfo(await api.getServerInfo().catch(() => info));
          show(
            "success",
            room
              ? `${room} laboratuvarı seçildi.`
              : prefix
                ? `${prefix} ile başlayan bilgisayarlar listelenecek.`
                : "Bütün bilgisayarlar listelenecek.",
          );
        }}
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

      <ShutdownDialog
        open={shutdownTarget !== null}
        device={shutdownTarget}
        onClose={() => setShutdownTarget(null)}
        onShutdown={async (deviceId) => {
          const device = shutdownTarget;
          setShutdownTarget(null);
          if (!device) return;
          await guard(
            api.shutdownDevice(deviceId),
            `${device.hostname} kapatılıyor.`,
          );
        }}
      />

      <FocusDialog
        device={focusedDevice}
        frameBaseUrl={frameBaseUrl}
        controlled={controlled !== null && controlled === focusedDevice?.deviceId}
        onToggleControl={toggleControl}
        onClose={closeFocus}
      />

      <KeyMonitorDialog
        open={keyMonitorOpen}
        device={keyMonitorDevice}
        monitoring={keyMonitoring}
        events={keyEvents}
        onToggle={toggleKeyMonitor}
        onClose={closeKeyMonitor}
      />

      {menu && (
        <ContextMenu x={menu.x} y={menu.y} items={contextItems} onClose={() => setMenu(null)} />
      )}
    </div>
  );
}

// --------------------------------------------------------------- alt görünümler

/**
 * Cihaz verilen gruba ait mi?
 *
 * **Oda adı bilgisayar adı önekinden önce gelir.** Oda, öğrencinin ilk
 * açılışta kendi girdiği değerdir; bilgisayar adı önekine dayanan eşleme ise
 * okulun adlandırma disiplinine bağlıdır ve `DESKTOP-4F9K2A` gibi varsayılan
 * adlarda hiç çalışmaz.
 *
 * Odası bilinen bir cihaz **yalnızca** o odanın grubunda görünür; öneki başka
 * bir grupla eşleşse bile oraya düşmez. Aksi hâlde `LAB1` önekli ama "Lab 2"
 * odasına taşınmış bir makine iki grupta birden görünürdü.
 */
function inGroup(device: DeviceView, group: DeviceGroup | null): boolean {
  if (!group) return true;
  if (device.room) {
    return device.room.trim().toUpperCase() === group.name.trim().toUpperCase();
  }
  // Ön eki boş bir grup yalnızca odaya göre eşleşir. Boş metin her adın
  // başında bulunduğundan `startsWith("")` bütün cihazları yakalar; odası
  // bilinmeyen makineler o gruba düşmemeli.
  const prefix = group.prefix.trim();
  if (!prefix) return false;
  return device.hostname.toUpperCase().startsWith(prefix.toUpperCase());
}

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

/**
 * Native gezginden seçilen bir Windows yolunu öğrenciyle paylaşılan bir
 * hedefe çözer.
 *
 * Öğretmen kendi makinesinde `Belgeler → Ders Notları` gibi bir klasör
 * seçer. Seçilen yol paylaşılan köklerden birine (Belgeler / Masaüstü /
 * İndirilenler) denk geliyorsa kalan göreli yol öğrencinin kullanıcı
 * klasöründeki aynı konuma gönderilir. Yol paylaşılan köklerden hiçbirinin
 * altında değilse `null` döner.
 *
 * Klasör adları hem Türkçe hem İngilizce olabilir (`Belgeler`/`Documents`,
 * `Masaüstü`/`Desktop`, `İndirilenler`/`Downloads`).
 */
function resolveDestination(
  path: string,
): { destination: "documents" | "desktop" | "downloads"; subpath: string | null; shown: string } | null {
  const parts = path.replace(/\\/g, "/").split("/").filter(Boolean);
  if (parts.length === 0) return null;

  const last = parts[parts.length - 1]!.toLowerCase();
  const parent = parts.length >= 2 ? parts[parts.length - 2]!.toLowerCase() : "";

  // Kök klasörün kendisi mi seçildi?
  if (destOf(last)) {
    return {
      destination: destOf(last)!,
      subpath: null,
      shown: destLabel(destOf(last)!),
    };
  }
  // Kök klasörün hemen içindeki bir alt klasör mü?
  if (destOf(parent)) {
    return {
      destination: destOf(parent)!,
      subpath: last,
      shown: `${destLabel(destOf(parent)!)} → ${parts[parts.length - 1]}`,
    };
  }
  return null;
}

function destOf(s: string): "documents" | "desktop" | "downloads" | null {
  switch (s) {
    case "belgeler":
    case "documents":
      return "documents";
    case "masaüstü":
    case "desktop":
      return "desktop";
    case "indirilenler":
    case "downloads":
      return "downloads";
    default:
      return null;
  }
}

function destLabel(d: "documents" | "desktop" | "downloads"): string {
  return d === "documents" ? "Belgeler" : d === "desktop" ? "Masaüstü" : "İndirilenler";
}

// ------------------------------------------------------------ klavye izleme

interface KeyLogEntry {
  id: number;
  label: string;
  code: number;
  ts: number;
}

/** Windows sanal tuş kodunu (VK_*) okunur bir etikete çevirir. */
function keyLabel(code: number): string {
  // Harfler ve rakamlar doğrudan karşılıklarını gösterir.
  if (code >= 0x41 && code <= 0x5a) return String.fromCharCode(code);
  if (code >= 0x30 && code <= 0x39) return String.fromCharCode(code);
  if (code >= 0x70 && code <= 0x87) return `F${code - 0x70 + 1}`;

  const named: Record<number, string> = {
    0x08: "Backspace",
    0x09: "Tab",
    0x0d: "Enter",
    0x10: "Shift",
    0x11: "Ctrl",
    0x12: "Alt",
    0x14: "Caps Lock",
    0x1b: "Escape",
    0x20: "Boşluk",
    0x25: "←",
    0x26: "↑",
    0x27: "→",
    0x28: "↓",
    0x2c: "PrintScreen",
    0x2d: "Insert",
    0x2e: "Delete",
    0xa0: "Sol Shift",
    0xa1: "Sağ Shift",
    0xa2: "Sol Ctrl",
    0xa3: "Sağ Ctrl",
    0xa4: "Sol Alt",
    0xa5: "Sağ Alt",
    0x5b: "Windows",
    0x5c: "Windows",
  };
  return named[code] ?? `VK ${code}`;
}

/** Klavye izleme penceresi: seçili cihazın canlı tuş akışı. */
function KeyMonitorDialog({
  open,
  device,
  monitoring,
  events,
  onToggle,
  onClose,
}: {
  open: boolean;
  device: DeviceView | null;
  monitoring: boolean;
  events: KeyLogEntry[];
  onToggle: () => Promise<unknown>;
  onClose: () => void;
}) {
  const { guard } = useToast();
  const listRef = useRef<HTMLDivElement>(null);

  // Yeni tuş geldikçe liste en alta kayar.
  useEffect(() => {
    const el = listRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [events]);

  return (
    <Modal
      open={open}
      title={device ? `${device.hostname} — klavye izleme` : "Klavye izleme"}
      onClose={onClose}
      width="md"
      footer={
        <>
          {device && (
            <Button
              tone={monitoring ? "danger" : "accent"}
              onClick={() => void guard(onToggle())}
              disabled={!device.online}
            >
              {monitoring ? "Yakalamayı Durdur" : "Yakala"}
            </Button>
          )}
          <Button onClick={onClose}>Kapat</Button>
        </>
      }
    >
      {!device ? (
        <Empty title="Cihaz seçilmedi" />
      ) : !device.online ? (
        <Empty
          title="Cihaz bağlı değil"
          detail="Klavye izleme için öğrencinin çevrimiçi olması gerekir."
        />
      ) : events.length === 0 ? (
        <Empty
          title={monitoring ? "Henüz tuş basılmadı" : "İzleme kapalı"}
          detail={
            monitoring
              ? "Öğrencinin bastığı tuşlar burada canlı görünür."
              : "Klavye İzle penceresindeki 'Yakala' ile izlemeyi başlatın."
          }
        />
      ) : (
        <div
          ref={listRef}
          className="h-[320px] overflow-y-auto rounded border border-[var(--color-stroke)]
            bg-[var(--color-surface-2)] p-2"
        >
          <div className="flex flex-wrap gap-1">
            {events.map((e) => (
              <span
                key={e.id}
                className="inline-flex h-[28px] min-w-[28px] items-center justify-center rounded border
                  border-[var(--color-stroke-strong)] bg-[var(--color-surface)] px-1.5
                  font-mono text-[12.5px] text-[var(--color-text-1)]"
                title={`VK ${e.code}`}
              >
                {e.label}
              </span>
            ))}
          </div>
        </div>
      )}
    </Modal>
  );
}
