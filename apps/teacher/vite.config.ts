import { readFileSync } from "node:fs";

import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

// Tauri geliştirme sunucusu sabit bir portta çalışmalı; port değişirse
// tauri.conf.json içindeki devUrl de güncellenmelidir.
/**
 * Geliştirme eklentisi.
 *
 * `/mock-frame/<id>` adresine, üretimdeki `ocframe://` ile **aynı ikili
 * biçimde** sahte bir ekran güncellemesi döner. Böylece tarayıcıda çalışırken
 * tuval birleştirme yolu gerçek kodla sınanır, ayrı bir sahte yol açılmaz.
 *
 * `?shot=1` ile açıldığında `load` olayını kısa süre geciktirir; başsız
 * tarayıcıyla ekran görüntüsü alırken veriler yerleşmiş olur.
 *
 * Üretim derlemesinde bu eklenti hiç çalışmaz (`apply: "serve"`).
 */
function mockFramePlugin() {
  const screens: Record<string, string> = JSON.parse(
    readFileSync(new URL("./mock-screens.json", import.meta.url), "utf8"),
  );

  /** Üretimdeki `OCU1` paketini kurar: tek anahtar kare, tek bölge. */
  function buildUpdate(jpeg: Buffer, width: number, height: number, seq: number): Buffer {
    const header = Buffer.alloc(13);
    header.write("OCU1", 0, "ascii");
    header.writeUInt8(1, 4); // anahtar kareyle başlıyor
    header.writeUInt16LE(width, 5);
    header.writeUInt16LE(height, 7);
    header.writeUInt32LE(1, 9); // güncelleme sayısı

    const meta = Buffer.alloc(11);
    meta.writeBigUInt64LE(BigInt(seq), 0);
    meta.writeUInt8(1, 8); // anahtar kare
    meta.writeUInt16LE(1, 9); // bölge sayısı

    const rect = Buffer.alloc(12);
    rect.writeUInt16LE(0, 0);
    rect.writeUInt16LE(0, 2);
    rect.writeUInt16LE(width, 4);
    rect.writeUInt16LE(height, 6);
    rect.writeUInt32LE(jpeg.length, 8);

    return Buffer.concat([header, meta, rect, jpeg]);
  }

  return {
    name: "openclass-mock-frame",
    apply: "serve" as const,
    transformIndexHtml(html: string, ctx: { originalUrl?: string }) {
      if (!ctx.originalUrl?.includes("shot=1")) return html;
      return html.replace(
        "</body>",
        '<script type="module" src="/__oc_shot_delay.js"></script></body>',
      );
    },
    configureServer(server: {
      middlewares: { use: (fn: (req: any, res: any, next: () => void) => void) => void };
    }) {
      server.middlewares.use((req, res, next) => {
        if (!(req.url ?? "").startsWith("/__oc_shot_delay.js")) return next();
        setTimeout(() => {
          res.setHeader("Content-Type", "application/javascript");
          res.end("// ekran görüntüsü gecikmesi");
        }, 2200);
      });

      server.middlewares.use((req, res, next) => {
        const url: string = req.url ?? "";
        if (!url.startsWith("/mock-frame/")) return next();

        const [path, query = ""] = url.slice("/mock-frame/".length).split("?");
        const since = Number(new URLSearchParams(query).get("since") ?? 0);

        // Sahte arka uç tek bir anahtar kare tutar; arayüz onu bir kez
        // çizdiyse yeni veri yoktur.
        if (since > 0) {
          res.statusCode = 204;
          res.end();
          return;
        }

        const key = String((parseInt(path.slice(-4).replace(/\D/g, ""), 10) % 16) + 1);
        const jpeg = Buffer.from(screens[key] ?? screens["1"], "base64");

        res.setHeader("Content-Type", "application/octet-stream");
        res.setHeader("Cache-Control", "no-store");
        res.end(buildUpdate(jpeg, 480, 270, 1));
      });
    },
  };
}

export default defineConfig({
  plugins: [react(), tailwindcss(), mockFramePlugin()],
  clearScreen: false,
  server: {
    // Adres açıkça sabitlenir. Vite'ın varsayılanı "localhost"tur ve bu, sistem
    // çözümlemesine göre yalnızca [::1]'e bağlanabilir; WebView ise localhost'u
    // 127.0.0.1'e çözerse "connection refused" alır. Her iki tarafı da
    // 127.0.0.1'e sabitleyerek bu belirsizliği kaldırıyoruz.
    host: "127.0.0.1",
    port: 1420,
    strictPort: true,
    watch: {
      // Rust tarafındaki değişiklikler Vite'ı yeniden yüklemesin.
      ignored: ["**/src-tauri/**"],
    },
  },
  build: {
    // Tauri WebView2 kullanır; modern hedef güvenli.
    target: "chrome120",
    sourcemap: false,
  },
});
