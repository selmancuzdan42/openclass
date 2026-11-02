import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

// Tauri geliştirme sunucusu sabit bir portta çalışmalı; port değişirse
// tauri.conf.json içindeki devUrl de güncellenmelidir.
/**
 * Geliştirme eklentisi: `?shot=1` ile açıldığında `load` olayını geciktirir,
 * böylece başsız tarayıcıyla ekran görüntüsü alınırken veriler yerleşmiş olur.
 * Üretim derlemesinde yoktur.
 */
function shotDelayPlugin() {
  return {
    name: "openclass-shot-delay",
    apply: "serve" as const,
    transformIndexHtml(html: string, ctx: { originalUrl?: string }) {
      if (!ctx.originalUrl?.includes("shot=1")) return html;
      return html.replace("</body>", '<script type="module" src="/__oc_shot_delay.js"></script></body>');
    },
    configureServer(server: { middlewares: { use: (fn: (req: any, res: any, next: () => void) => void) => void } }) {
      server.middlewares.use((req, res, next) => {
        if (!(req.url ?? "").startsWith("/__oc_shot_delay.js")) return next();
        setTimeout(() => {
          res.setHeader("Content-Type", "application/javascript");
          res.end("// ekran görüntüsü gecikmesi");
        }, 2200);
      });
    },
  };
}

export default defineConfig({
  plugins: [react(), tailwindcss(), shotDelayPlugin()],
  clearScreen: false,
  server: {
    // Adres açıkça sabitlenir. Vite'ın varsayılanı "localhost"tur ve bu, sistem
    // çözümlemesine göre yalnızca [::1]'e bağlanabilir; WebView ise localhost'u
    // 127.0.0.1'e çözerse "connection refused" alır. Her iki tarafı da
    // 127.0.0.1'e sabitleyerek bu belirsizliği kaldırıyoruz.
    host: "127.0.0.1",
    port: 1421,
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
