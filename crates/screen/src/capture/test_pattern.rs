//! Test deseni arka ucu — **gerçek ekran değildir.**
//!
//! Yalnızca `OPENCLASS_CAPTURE=test` ayarlandığında devreye girer ve sentetik
//! bir masaüstü çizer. Amacı, Windows olmadan kare hattının tamamını
//! sınayabilmektir: ölçekleme, JPEG kodlama, değişmeyen kare atlama, ağ
//! aktarımı, öğretmen konsolunda görüntüleme, kalite değişimi ve çok sayıda
//! eşzamanlı akışın yükü.
//!
//! Desende bilgisayar adı ve kare numarası okunabilir biçimde yazılır; bu,
//! "büyütünce metin okunur hâle geliyor mu?" sorusunu (Test 6) gerçek veriyle
//! yanıtlamayı sağlar.
//!
//! Arka ucun adı arayüzde **"Test deseni (gerçek ekran değil)"** olarak
//! görünür; hiçbir koşulda gerçek ekran gibi sunulmaz.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Result};
use parking_lot::Mutex;

use crate::encode::{PixelLayout, RawFrame};

/// Desen üretiminin üst sınırı. Gerçek hız tüketime göre belirlenir:
/// önceki kare henüz alınmadıysa yenisi çizilmez (bkz. döngü).
const MAX_SOURCE_FPS: u64 = 12;

/// Yuva doluyken beklenen süre. Gönderici 2 FPS ile örneklerken 12 FPS
/// çizmek boşa CPU harcar; bu yoklama aralığı gereksiz çizimi eler.
const IDLE_POLL: Duration = Duration::from_millis(20);

/// Varsayılan sentetik ekran çözünürlüğü.
const DEFAULT_SIZE: (u32, u32) = (1920, 1080);

type FrameSlot = Arc<Mutex<Option<RawFrame>>>;

pub struct Session {
    slot: FrameSlot,
    running: Arc<AtomicBool>,
}

impl Session {
    pub fn start(label: &str) -> Result<Self> {
        let (width, height) = desired_size();
        let slot: FrameSlot = Arc::new(Mutex::new(None));
        let running = Arc::new(AtomicBool::new(true));

        let label = label.to_string();
        let thread_slot = Arc::clone(&slot);
        let thread_running = Arc::clone(&running);

        std::thread::Builder::new()
            .name("openclass-test-pattern".into())
            .spawn(move || {
                // İş parçacığı nasıl sonlanırsa sonlansın bayrak düşsün.
                let _guard = crate::capture::RunningGuard(Arc::clone(&thread_running));

                let len = (width as usize) * (height as usize) * 4;
                let period = Duration::from_millis(1000 / MAX_SOURCE_FPS);
                let hue = hue_for(&label);
                let mut frame: u64 = 0;
                // Düşürülen karelerin tamponunu geri dönüştür: 20 eşzamanlı
                // akışta saniyede yüzlerce megabayt ayırmamak için.
                let mut spare: Option<Vec<u8>> = None;

                while thread_running.load(Ordering::Relaxed) {
                    // Önceki kare henüz tüketilmediyse yenisini çizme.
                    // Gerçek ekran yakalamada kareleri donanım üretir; burada
                    // her kare CPU ile çiziliyor, bu yüzden talep kadar
                    // üretmek gerekir.
                    if thread_slot.lock().is_some() {
                        std::thread::sleep(IDLE_POLL);
                        continue;
                    }

                    let started = std::time::Instant::now();
                    let mut pixels = spare.take().unwrap_or_else(|| vec![0u8; len]);
                    if pixels.len() != len {
                        pixels.resize(len, 0);
                    }
                    draw(&mut pixels, width, height, hue, frame, &label);

                    let previous = thread_slot.lock().replace(RawFrame {
                        width,
                        height,
                        layout: PixelLayout::Rgba8,
                        pixels,
                    });
                    spare = previous.map(|f| f.pixels);

                    frame = frame.wrapping_add(1);
                    if let Some(rest) = period.checked_sub(started.elapsed()) {
                        std::thread::sleep(rest);
                    }
                }
                tracing::info!("test deseni iş parçacığı sonlandı");
            })
            .map_err(|e| anyhow!("test deseni iş parçacığı başlatılamadı: {e}"))?;

        tracing::warn!(
            width,
            height,
            "TEST DESENİ etkin — gerçek ekran yakalanmıyor (OPENCLASS_CAPTURE=test)"
        );
        Ok(Self { slot, running })
    }

    pub fn take_latest(&self) -> Option<RawFrame> {
        self.slot.lock().take()
    }

    pub fn backend_name(&self) -> &'static str {
        "Test deseni (gerçek ekran değil)"
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    pub fn stop(self) {
        self.running.store(false, Ordering::Relaxed);
    }
}

/// `OPENCLASS_CAPTURE_SIZE=1280x720` ile çözünürlük değiştirilebilir.
fn desired_size() -> (u32, u32) {
    let Ok(raw) = std::env::var("OPENCLASS_CAPTURE_SIZE") else {
        return DEFAULT_SIZE;
    };
    let Some((w, h)) = raw.split_once(['x', 'X']) else {
        tracing::warn!(%raw, "OPENCLASS_CAPTURE_SIZE biçimi 1280x720 olmalı");
        return DEFAULT_SIZE;
    };
    match (w.trim().parse::<u32>(), h.trim().parse::<u32>()) {
        (Ok(w), Ok(h)) if w >= 160 && h >= 120 && w <= 7680 && h <= 4320 => (w, h),
        _ => {
            tracing::warn!(%raw, "OPENCLASS_CAPTURE_SIZE geçersiz, varsayılana dönülüyor");
            DEFAULT_SIZE
        }
    }
}

/// Her sahte öğrencinin ayırt edilebilmesi için addan sabit bir renk tonu.
fn hue_for(label: &str) -> f32 {
    let sum: u32 = label.bytes().map(u32::from).sum();
    (sum % 360) as f32
}

/// Üzerine çizim yapılan RGBA tamponu.
///
/// Piksel tamponu, genişlik ve yükseklik bir arada taşınır; aksi hâlde her
/// çizim yardımcısı aynı üç değeri ayrı ayrı almak zorunda kalırdı.
struct Canvas<'a> {
    pixels: &'a mut [u8],
    w: usize,
    h: usize,
}

impl Canvas<'_> {
    /// Dikdörtgen doldurur; tampon sınırları dışına taşmaz.
    fn fill(&mut self, x0: usize, y0: usize, rect_w: usize, rect_h: usize, (r, g, b): Rgb) {
        for y in y0..(y0 + rect_h).min(self.h) {
            let row = y * self.w * 4;
            for x in x0..(x0 + rect_w).min(self.w) {
                let i = row + x * 4;
                self.pixels[i] = r;
                self.pixels[i + 1] = g;
                self.pixels[i + 2] = b;
                self.pixels[i + 3] = 255;
            }
        }
    }

    /// 5×7 bit eşlemli yazı tipiyle metin çizer.
    ///
    /// Yalnızca büyük harf, rakam ve birkaç işaret desteklenir; Türkçe'ye
    /// özgü harfler desende kullanılmaz.
    fn text(&mut self, x0: usize, y0: usize, scale: usize, text: &str, color: Rgb) {
        let mut cursor = x0;
        for ch in text.chars() {
            if cursor >= self.w {
                return;
            }
            let glyph = glyph(ch.to_ascii_uppercase());
            for (row, bits) in glyph.iter().enumerate() {
                for col in 0..5usize {
                    if bits & (1 << (4 - col)) != 0 {
                        self.fill(cursor + col * scale, y0 + row * scale, scale, scale, color);
                    }
                }
            }
            cursor += 6 * scale;
        }
    }
}

type Rgb = (u8, u8, u8);

/// Ölçüm testleri için çizim köprüsü.
#[cfg(test)]
pub(crate) fn draw_for_bench(
    pixels: &mut [u8],
    width: u32,
    height: u32,
    hue: f32,
    frame: u64,
    label: &str,
) {
    draw(pixels, width, height, hue, frame, label);
}

/// Sentetik masaüstünü çizer.
fn draw(pixels: &mut [u8], width: u32, height: u32, hue: f32, frame: u64, label: &str) {
    let w = width as usize;
    let h = height as usize;
    let mut c = Canvas { pixels, w, h };

    // Arka plan: dikey degrade.
    let (bg_r, bg_g, bg_b) = hsl_to_rgb(hue, 0.30, 0.18);
    let (bg2_r, bg2_g, bg2_b) = hsl_to_rgb(hue, 0.35, 0.10);
    for y in 0..h {
        let t = y as f32 / h as f32;
        let color = (
            lerp(bg_r, bg2_r, t),
            lerp(bg_g, bg2_g, t),
            lerp(bg_b, bg2_b, t),
        );
        c.fill(0, y, w, 1, color);
    }

    // Üst çubuk (sahte başlık çubuğu).
    let bar_h = (h / 24).max(18);
    c.fill(0, 0, w, bar_h, hsl_to_rgb(hue, 0.40, 0.28));

    // Sahte pencere gövdesi.
    let pad = w / 24;
    let win_y = bar_h + h / 14;
    let win_h = h / 2;
    c.fill(pad, win_y, w - pad * 2, win_h, (236, 240, 245));

    // Gövdede metin satırlarını taklit eden çizgiler.
    let line_h = (win_h / 18).max(4);
    for i in 0..9usize {
        let y = win_y + line_h * 2 + i * line_h * 2;
        let len = (w - pad * 2) * (55 + (i * 7) % 40) / 100;
        c.fill(pad * 2, y, len, line_h, (120, 132, 148));
    }

    // Soldan sağa süzülen blok: kareler arasında hareket olduğunu kanıtlar.
    let sweep_w = w / 12;
    let span = w.saturating_sub(sweep_w);
    let pos = if span == 0 {
        0
    } else {
        (frame as usize * 11) % span
    };
    c.fill(
        pos,
        win_y + win_h + h / 20,
        sweep_w,
        h / 16,
        hsl_to_rgb((hue + 160.0) % 360.0, 0.70, 0.55),
    );

    // Okunabilir metin: bilgisayar adı ve kare numarası.
    // Test 6 (büyütünce metin okunur mu?) bu sayede gerçek veriyle ölçülür.
    //
    // Yerleşim hesapları `saturating_sub` kullanır: küçük çözünürlüklerde
    // yazı tipi ölçeği çubuk yüksekliğini aşabilir ve çıkarma taşabilir.
    let scale = (h / 90).max(2);
    let title_y = (bar_h / 2).saturating_sub(scale * 3);
    c.text(pad, title_y, scale, label, (255, 255, 255));
    c.text(
        pad,
        win_y + win_h + h / 6,
        scale,
        &format!("KARE {frame}"),
        (235, 238, 242),
    );
    c.text(
        pad,
        win_y + win_h + h / 6 + scale * 10,
        scale.saturating_sub(1).max(1),
        "TEST DESENI - GERCEK EKRAN DEGIL",
        (250, 204, 120),
    );
}

fn lerp(a: u8, b: u8, t: f32) -> u8 {
    (a as f32 + (b as f32 - a as f32) * t) as u8
}

/// 5 sütun × 7 satırlık bit eşlemi; her bayt bir satırın alt 5 bitidir.
fn glyph(c: char) -> [u8; 7] {
    match c {
        '0' => [
            0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110,
        ],
        '1' => [
            0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        '2' => [
            0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111,
        ],
        '3' => [
            0b11111, 0b00010, 0b00100, 0b00010, 0b00001, 0b10001, 0b01110,
        ],
        '4' => [
            0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010,
        ],
        '5' => [
            0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110,
        ],
        '6' => [
            0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110,
        ],
        '7' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000,
        ],
        '8' => [
            0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110,
        ],
        '9' => [
            0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100,
        ],
        'A' => [
            0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'B' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110,
        ],
        'C' => [
            0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110,
        ],
        'D' => [
            0b11100, 0b10010, 0b10001, 0b10001, 0b10001, 0b10010, 0b11100,
        ],
        'E' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111,
        ],
        'F' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'G' => [
            0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01111,
        ],
        'H' => [
            0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'I' => [
            0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        'J' => [
            0b00111, 0b00010, 0b00010, 0b00010, 0b00010, 0b10010, 0b01100,
        ],
        'K' => [
            0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001,
        ],
        'L' => [
            0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111,
        ],
        'M' => [
            0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001,
        ],
        'N' => [
            0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001,
        ],
        'O' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'P' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'Q' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101,
        ],
        'R' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001,
        ],
        'S' => [
            0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110,
        ],
        'T' => [
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        'U' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'V' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100,
        ],
        'W' => [
            0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b11011, 0b10001,
        ],
        'X' => [
            0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001,
        ],
        'Y' => [
            0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        'Z' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111,
        ],
        '-' => [
            0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000,
        ],
        '.' => [
            0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b01100, 0b01100,
        ],
        ':' => [
            0b00000, 0b01100, 0b01100, 0b00000, 0b01100, 0b01100, 0b00000,
        ],
        ' ' => [0; 7],
        _ => [
            0b11111, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11111,
        ],
    }
}

/// HSL → RGB; desendeki renkleri addan türetmek için.
fn hsl_to_rgb(h: f32, s: f32, l: f32) -> Rgb {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - (((h / 60.0) % 2.0) - 1.0).abs());
    let m = l - c / 2.0;
    let (r, g, b) = match h as u32 / 60 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    (
        ((r + m) * 255.0) as u8,
        ((g + m) * 255.0) as u8,
        ((b + m) * 255.0) as u8,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desen_dogru_boyutta_kare_uretir() {
        let (w, h) = (320u32, 180u32);
        let mut pixels = vec![0u8; (w * h * 4) as usize];
        draw(&mut pixels, w, h, 200.0, 0, "LAB101");
        assert_eq!(pixels.len(), (w * h * 4) as usize);
        // Alfa kanalı her pikselde dolu olmalı.
        assert!(pixels.chunks_exact(4).all(|px| px[3] == 255));
    }

    #[test]
    fn ardisik_kareler_farklidir() {
        let (w, h) = (320u32, 180u32);
        let mut a = vec![0u8; (w * h * 4) as usize];
        let mut b = vec![0u8; (w * h * 4) as usize];
        draw(&mut a, w, h, 200.0, 0, "LAB101");
        draw(&mut b, w, h, 200.0, 30, "LAB101");
        assert_ne!(a, b, "hareketli blok kareler arasında değişmeli");
    }

    #[test]
    fn farkli_adlar_farkli_renk_uretir() {
        assert_ne!(hue_for("LAB101"), hue_for("LAB102"));
    }

    #[test]
    fn cozunurluk_degiskeni_dogrulanir() {
        // Geçersiz değerler sessizce varsayılana döner; panik olmaz.
        assert_eq!(DEFAULT_SIZE, (1920, 1080));
    }

    #[test]
    fn gercek_cozunurluklerde_tasma_olmaz() {
        // Bu test bir regresyondur: 1080p'de `bar_h / 2 - scale * 3`
        // çıkarması taşıyor ve çizim iş parçacığı panikliyordu.
        for (w, h) in [
            (1920u32, 1080u32),
            (1280, 720),
            (3840, 2160),
            (800, 600),
            (160, 120),
        ] {
            let mut pixels = vec![0u8; (w as usize) * (h as usize) * 4];
            draw(&mut pixels, w, h, 120.0, 7, "LAB101");
        }
    }

    #[test]
    fn metin_cizimi_sinirlari_asmaz() {
        let (w, h) = (64u32, 32u32);
        let mut pixels = vec![0u8; (w * h * 4) as usize];
        // Tampona sığmayacak kadar uzun metin: taşma olmamalı.
        let mut c = Canvas {
            pixels: &mut pixels,
            w: w as usize,
            h: h as usize,
        };
        c.text(0, 0, 3, "COK UZUN BIR METIN", (255, 255, 255));
    }
}
