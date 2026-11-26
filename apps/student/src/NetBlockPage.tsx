import { WifiOff } from "lucide-react";

/**
 * İnternet kesikken tarayıcının üstünü kaplayan sayfa.
 *
 * Rust tarafı bu pencereyi öndeki tarayıcının sayfa alanına yerleştirir;
 * öğrenci sitenin neden açılmadığını tarayıcının kendi hata sayfası yerine
 * buradan okur. Pencereyi Rust tarafı gösterir, gizler ve kapatır; burada
 * hiçbir komut çağrılmaz.
 */
export function NetBlockPage() {
  return (
    <div className="flex size-full select-none flex-col items-center justify-center gap-6
      bg-[#f4f6fa] px-10 text-center text-[#1f2933]">
      <div className="flex size-24 items-center justify-center rounded-full bg-[#c62828]/10">
        <WifiOff size={46} className="text-[#c62828]" />
      </div>

      <div className="space-y-3">
        <h1 className="text-[30px] font-semibold tracking-tight">
          İnternet bağlantınız kesildi
        </h1>
        <p className="mx-auto max-w-xl text-[16px] leading-relaxed text-[#52606d]">
          İnternet erişimi öğretmeniniz tarafından kapatıldı. Öğretmeniniz yeniden açana kadar
          internet siteleri açılmaz.
        </p>
      </div>

      <p className="rounded-full bg-[#1f2933]/5 px-4 py-1.5 text-[12.5px] font-medium
        text-[#52606d]">
        OpenClass sınıf yönetimi
      </p>
    </div>
  );
}
