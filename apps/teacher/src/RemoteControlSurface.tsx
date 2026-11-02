import { useCallback, useEffect, useRef } from "react";
import type { ReactNode } from "react";

import { api } from "./api";
import type { InputEvent } from "./types";

/**
 * Uzaktan kontrol yüzeyi.
 *
 * İçindeki görüntünün üzerine saydam bir katman koyar ve fare/klavye
 * olaylarını yakalayıp öğrenci makinesine iletir.
 *
 * # Koordinatlar
 *
 * Konum **normalleştirilmiş** (0.0–1.0) gönderilir. Öğretmenin gördüğü
 * görüntü ölçeklenmiştir ve öğrencinin çözünürlüğü farklı olabilir; piksel
 * göndermek yanlış yere tıklatırdı. Ayrıca görüntü `object-contain` ile
 * yerleştiği için kutunun kenarlarında boşluk kalabilir — o boşluktaki
 * olaylar yok sayılır, aksi hâlde ekranın dışına tıklanmış olurdu.
 *
 * # Gönderim hızı
 *
 * Fare hareketi saniyede onlarca olay üretir. Olaylar biriktirilip sabit
 * aralıklarla toplu gönderilir; ardışık hareketlerin yalnızca sonuncusu
 * taşınır, aradakiler zaten geçersizdir.
 */
export function RemoteControlSurface({
  active,
  aspectRatio,
  children,
}: {
  active: boolean;
  /** Uzak ekranın en/boy oranı; harf kutusu boşluğunu hesaplamak için. */
  aspectRatio: number;
  children: ReactNode;
}) {
  const hostRef = useRef<HTMLDivElement>(null);
  const queue = useRef<InputEvent[]>([]);
  const flushing = useRef(false);
  const sentOnce = useRef(false);
  const failedOnce = useRef(false);

  /** Biriken olayları gönderir. */
  const flush = useCallback(async () => {
    if (flushing.current || queue.current.length === 0) return;
    flushing.current = true;
    const batch = queue.current;
    queue.current = [];
    try {
      await api.sendRemoteInput(batch);
      if (!sentOnce.current) {
        sentOnce.current = true;
        console.info(`[kontrol] ilk girdi yığını gönderildi (${batch.length} olay)`);
      }
    } catch (err) {
      // Hata yutulursa kontrolün neden çalışmadığı anlaşılmaz. İlk hatayı
      // bildir; sonrakileri susturup konsolu boğmaktan kaçın.
      if (!failedOnce.current) {
        failedOnce.current = true;
        console.error("[kontrol] girdi gönderilemedi:", err);
      }
    } finally {
      flushing.current = false;
    }
  }, []);

  useEffect(() => {
    if (!active) {
      queue.current = [];
      return;
    }
    const id = window.setInterval(() => void flush(), 33); // ~30 Hz
    return () => window.clearInterval(id);
  }, [active, flush]);

  const push = useCallback((event: InputEvent) => {
    // Ardışık fare hareketlerinin yalnızca sonuncusu anlamlı.
    if (event.t === "mouse_move") {
      const last = queue.current[queue.current.length - 1];
      if (last?.t === "mouse_move") {
        queue.current[queue.current.length - 1] = event;
        return;
      }
    }
    queue.current.push(event);
  }, []);

  /** Olay konumunu uzak ekranın normalleştirilmiş koordinatına çevirir. */
  const normalize = useCallback(
    (clientX: number, clientY: number): { x: number; y: number } | null => {
      const host = hostRef.current;
      if (!host) return null;
      const box = host.getBoundingClientRect();
      if (box.width === 0 || box.height === 0) return null;

      // `object-contain` ile görüntü kutuya sığdırılır; artan yer boşluktur.
      const boxRatio = box.width / box.height;
      let drawW = box.width;
      let drawH = box.height;
      if (boxRatio > aspectRatio) {
        drawW = box.height * aspectRatio;
      } else {
        drawH = box.width / aspectRatio;
      }
      const offsetX = (box.width - drawW) / 2;
      const offsetY = (box.height - drawH) / 2;

      const x = (clientX - box.left - offsetX) / drawW;
      const y = (clientY - box.top - offsetY) / drawH;
      if (x < 0 || x > 1 || y < 0 || y > 1) {
        // Harf kutusu boşluğuna düşen olaylar elenir; bu beklenen durum
        // ama hepsi eleniyorsa oran hesabı yanlış demektir.
        return null;
      }
      return { x, y };
    },
    [aspectRatio],
  );

  useEffect(() => {
    if (!active) return;
    const host = hostRef.current;
    if (!host) {
      console.error("[kontrol] yüzey öğesi bulunamadı, olaylar dinlenemiyor");
      return;
    }
    const box = host.getBoundingClientRect();
    console.info(
      `[kontrol] dinleme başladı — yüzey ${Math.round(box.width)}x${Math.round(box.height)}, oran ${aspectRatio.toFixed(3)}`,
    );
    if (box.width === 0 || box.height === 0) {
      console.error("[kontrol] yüzeyin boyutu sıfır, hiçbir olay gönderilemez");
    }

    const onMove = (e: PointerEvent) => {
      const p = normalize(e.clientX, e.clientY);
      if (p) push({ t: "mouse_move", x: p.x, y: p.y });
    };

    const button = (index: number): "left" | "right" | "middle" | null =>
      index === 0 ? "left" : index === 1 ? "middle" : index === 2 ? "right" : null;

    const onDown = (e: PointerEvent) => {
      const p = normalize(e.clientX, e.clientY);
      const b = button(e.button);
      if (!p || !b) return;
      e.preventDefault();
      push({ t: "mouse_move", x: p.x, y: p.y });
      push({ t: "mouse_button", button: b, pressed: true });
    };

    const onUp = (e: PointerEvent) => {
      const b = button(e.button);
      if (!b) return;
      e.preventDefault();
      push({ t: "mouse_button", button: b, pressed: false });
    };

    const onWheel = (e: WheelEvent) => {
      e.preventDefault();
      // Tarayıcı piksel/satır birimleri verir; adım sayısına çeviriyoruz.
      const dy = e.deltaY === 0 ? 0 : e.deltaY > 0 ? -1 : 1;
      const dx = e.deltaX === 0 ? 0 : e.deltaX > 0 ? 1 : -1;
      push({ t: "mouse_wheel", dx, dy });
    };

    const onContext = (e: Event) => e.preventDefault();

    const onKey = (e: KeyboardEvent) => {
      const code = virtualKey(e);
      if (code === null) return;
      e.preventDefault();
      push({ t: "key", code, pressed: e.type === "keydown" });
    };

    host.addEventListener("pointermove", onMove);
    host.addEventListener("pointerdown", onDown);
    window.addEventListener("pointerup", onUp);
    host.addEventListener("wheel", onWheel, { passive: false });
    host.addEventListener("contextmenu", onContext);
    window.addEventListener("keydown", onKey);
    window.addEventListener("keyup", onKey);

    return () => {
      host.removeEventListener("pointermove", onMove);
      host.removeEventListener("pointerdown", onDown);
      window.removeEventListener("pointerup", onUp);
      host.removeEventListener("wheel", onWheel);
      host.removeEventListener("contextmenu", onContext);
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("keyup", onKey);
    };
  }, [active, aspectRatio, normalize, push]);

  return (
    <div
      ref={hostRef}
      tabIndex={active ? 0 : -1}
      className={`relative size-full ${active ? "cursor-crosshair outline-none" : ""}`}
    >
      {children}
    </div>
  );
}

/**
 * Tarayıcı klavye olayını Windows sanal tuş koduna (VK_*) çevirir.
 *
 * `event.code` fiziksel tuşu verir; yerleşimden bağımsız olduğu için
 * Türkçe Q/F klavye farkı sorun çıkarmaz. Eşleşmeyen tuşlar yok sayılır.
 */
function virtualKey(e: KeyboardEvent): number | null {
  const code = e.code;

  if (/^Key[A-Z]$/.test(code)) return code.charCodeAt(3);
  if (/^Digit[0-9]$/.test(code)) return code.charCodeAt(5);
  if (/^Numpad[0-9]$/.test(code)) return 0x60 + Number(code.slice(6));
  if (/^F([1-9]|1[0-9]|2[0-4])$/.test(code)) return 0x6f + Number(code.slice(1));

  const table: Record<string, number> = {
    Backspace: 0x08,
    Tab: 0x09,
    Enter: 0x0d,
    NumpadEnter: 0x0d,
    ShiftLeft: 0xa0,
    ShiftRight: 0xa1,
    ControlLeft: 0xa2,
    ControlRight: 0xa3,
    AltLeft: 0xa4,
    AltRight: 0xa5,
    Pause: 0x13,
    CapsLock: 0x14,
    Escape: 0x1b,
    Space: 0x20,
    PageUp: 0x21,
    PageDown: 0x22,
    End: 0x23,
    Home: 0x24,
    ArrowLeft: 0x25,
    ArrowUp: 0x26,
    ArrowRight: 0x27,
    ArrowDown: 0x28,
    PrintScreen: 0x2c,
    Insert: 0x2d,
    Delete: 0x2e,
    MetaLeft: 0x5b,
    MetaRight: 0x5c,
    NumpadMultiply: 0x6a,
    NumpadAdd: 0x6b,
    NumpadSubtract: 0x6d,
    NumpadDecimal: 0x6e,
    NumpadDivide: 0x6f,
    NumLock: 0x90,
    ScrollLock: 0x91,
    Semicolon: 0xba,
    Equal: 0xbb,
    Comma: 0xbc,
    Minus: 0xbd,
    Period: 0xbe,
    Slash: 0xbf,
    Backquote: 0xc0,
    BracketLeft: 0xdb,
    Backslash: 0xdc,
    BracketRight: 0xdd,
    Quote: 0xde,
  };
  return table[code] ?? null;
}
