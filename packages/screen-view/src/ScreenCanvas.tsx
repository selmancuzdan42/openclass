import { useCallback, useEffect, useRef, useState } from "react";

/**
 * Uzak bir ekranı çizen tuval.
 *
 * İki yerde kullanılır: öğretmen konsolunda öğrenci ekranları,
 * öğrenci tarafında öğretmen ekranı. İkisi de aynı ikili biçimi alır,
 * bu yüzden bileşen paylaşılır.
 *
 * Öğrenciler ekranın yalnızca **değişen bölgelerini** gönderir; tam kare
 * değil. Rust tarafı bu bölgeleri çözmez, olduğu gibi iletir — 20 cihaz için
 * kare başına JPEG çözme + yeniden kodlama öğretmen makinesini gereksiz
 * yorardı. Birleştirme burada yapılır: tarayıcı JPEG çözmeyi zaten donanım
 * hızlandırmalı yapar.
 *
 * Akış:
 *
 * 1. `seq` değiştiğinde `ocframe://<cihaz>?since=<elimizdeki sıra>` çekilir.
 * 2. Yanıt, bir veya birden çok güncelleme taşıyan ikili pakettir.
 * 3. Her bölge `createImageBitmap` ile çözülüp tuvale çizilir.
 *
 * Arayüz geride kalırsa Rust tarafı anahtar kareden itibaren her şeyi
 * gönderir, böylece tuval kendini toparlar.
 */
export function ScreenCanvas({
  deviceId,
  frameBaseUrl,
  seq,
  className = "",
  alt,
  onError,
}: {
  /** Kaynağı tanımlayan yol parçası (cihaz kimliği veya boş). */
  deviceId: string;
  /** Özel şemanın taban adresi (`ocframe://localhost` gibi). */
  frameBaseUrl: string;
  /** Rust tarafındaki son güncelleme sırası; değişince çekim tetiklenir. */
  seq: number;
  className?: string;
  alt: string;
  /** Çizim başarısız olursa çağrılır; `null` hatanın geçtiğini bildirir. */
  onError?: (message: string | null) => void;
}) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [failure, setFailure] = useState<string | null>(null);
  /** Tuvale işlenmiş son sıra numarası. */
  const appliedSeq = useRef(0);
  /** Aynı anda birden çok çekim yapılmasını engeller. */
  const busy = useRef(false);
  /** Çekim sürerken gelen yeni sıra; bittiğinde tekrar denenir. */
  const pending = useRef(false);

  // Cihaz değişirse tuval başka bir ekranı gösteriyor demektir.
  useEffect(() => {
    appliedSeq.current = 0;
  }, [deviceId]);

  const draw = useCallback(async () => {
    const canvas = canvasRef.current;
    if (!canvas || !frameBaseUrl) return;

    if (busy.current) {
      pending.current = true;
      return;
    }
    busy.current = true;

    try {
      do {
        pending.current = false;

        const path = deviceId ? `/${deviceId}` : "";
        const response = await fetch(
          `${frameBaseUrl}${path}?since=${appliedSeq.current}`,
        );
        // 204: yeni veri yok, mevcut tuval korunur.
        if (response.status === 204) break;
        if (response.status !== 200) {
          throw new Error(`ekran verisi alınamadı (HTTP ${response.status})`);
        }

        const buffer = await response.arrayBuffer();
        const applied = await paint(canvas, buffer);
        if (applied !== null) appliedSeq.current = applied;
      } while (pending.current);

      setFailure(null);
      onError?.(null);
    } catch (err) {
      // Sessizce yutmak, "veri geliyor ama ekran siyah" gibi teşhis edilmesi
      // zor bir duruma yol açıyor. Hem konsola hem arayüze bildirilir.
      const message = err instanceof Error ? err.message : String(err);
      // eslint-disable-next-line no-console
      console.error("[OpenClass] ekran çizilemedi", deviceId, err);
      setFailure(message);
      onError?.(message);
    } finally {
      busy.current = false;
    }
  }, [deviceId, frameBaseUrl]);

  useEffect(() => {
    if (seq > appliedSeq.current) void draw();
  }, [seq, draw]);

  return (
    <span className="relative block size-full">
      <canvas ref={canvasRef} className={className} role="img" aria-label={alt} />
      {failure && (
        // Veri geldiği hâlde tuvalin siyah kalması teşhis edilmesi en zor
        // durumlardan biri; sebebi ekranda yazılır.
        <span
          className="absolute inset-0 flex items-center justify-center bg-black/75 p-2
            text-center text-[10.5px] leading-snug text-[var(--color-danger)]"
          title={failure}
        >
          Ekran çizilemedi: {failure}
        </span>
      )}
    </span>
  );
}

/**
 * İkili paketi çözüp tuvale çizer; işlenen son sıra numarasını döner.
 *
 * Paket biçimi (küçük sonlu):
 *
 * ```text
 * "OCU1"            4 bayt
 * anahtarla_başlıyor u8
 * genişlik           u16
 * yükseklik          u16
 * güncelleme_sayısı  u32
 *   sıra             u64
 *   anahtar_kare     u8
 *   bölge_sayısı     u16
 *     x, y, w, h     u16 ×4
 *     uzunluk        u32
 *     JPEG           uzunluk bayt
 * ```
 */
async function paint(canvas: HTMLCanvasElement, buffer: ArrayBuffer): Promise<number | null> {
  const view = new DataView(buffer);
  if (buffer.byteLength < 13) return null;

  // Sihirli imza: yanlış biçimde sessizce vazgeç.
  if (
    view.getUint8(0) !== 0x4f ||
    view.getUint8(1) !== 0x43 ||
    view.getUint8(2) !== 0x55 ||
    view.getUint8(3) !== 0x31
  ) {
    return null;
  }

  let offset = 4;
  offset += 1; // anahtarla_başlıyor — tuval boyutu zaten aşağıda ayarlanıyor
  const width = view.getUint16(offset, true);
  offset += 2;
  const height = view.getUint16(offset, true);
  offset += 2;
  const updateCount = view.getUint32(offset, true);
  offset += 4;

  if (canvas.width !== width || canvas.height !== height) {
    canvas.width = width;
    canvas.height = height;
  }

  const ctx = canvas.getContext("2d");
  if (!ctx) return null;

  let lastSeq: number | null = null;

  for (let u = 0; u < updateCount; u++) {
    const seq = Number(view.getBigUint64(offset, true));
    offset += 8;
    offset += 1; // anahtar_kare — bölgeler zaten tam kareyi kaplıyor
    const rectCount = view.getUint16(offset, true);
    offset += 2;

    for (let r = 0; r < rectCount; r++) {
      const x = view.getUint16(offset, true);
      offset += 2;
      const y = view.getUint16(offset, true);
      offset += 2;
      const w = view.getUint16(offset, true);
      offset += 2;
      const h = view.getUint16(offset, true);
      offset += 2;
      const length = view.getUint32(offset, true);
      offset += 4;

      const bytes = new Uint8Array(buffer, offset, length);
      offset += length;

      const bitmap = await createImageBitmap(new Blob([bytes], { type: "image/jpeg" }));
      ctx.drawImage(bitmap, x, y, w, h);
      bitmap.close();
    }
    lastSeq = seq;
  }

  return lastSeq;
}
