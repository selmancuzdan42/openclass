//! Değişen bölge (karo) tespiti ve kodlaması.
//!
//! # Neden tüm kare gönderilmiyor?
//!
//! Sınıf ekranı — slayt, kod editörü, tarayıcı — kareden kareye neredeyse hiç
//! değişmez. Tam kareyi her seferinde kodlayıp göndermek, ölçümde 1080p'de
//! kare başına 104 KB demekti; 25 FPS'te 2,6 MB/s. Oysa öğrenci bir satır
//! yazarken ekranın yalnızca birkaç yüz pikselkaresi değişir.
//!
//! Referans ürünler (NetSupport dâhil) bu yüzden video kodeki değil, **değişen
//! bölge aktarımı** kullanır. Bu modül aynı yaklaşımı uygular:
//!
//! 1. Kare sabit boyutlu karolara bölünür ve her karonun özeti alınır.
//! 2. Özeti değişen karolar işaretlenir.
//! 3. Komşu değişen karolar dikdörtgenlere birleştirilir (JPEG başlık yükünü
//!    azaltmak için; karo başına ayrı JPEG göndermek pahalı olurdu).
//! 4. Her dikdörtgen ayrı ayrı JPEG'lenir.
//!
//! Hiçbir karo değişmediyse paket hiç üretilmez.
//!
//! # Anahtar kare
//!
//! Belirli aralıklarla tüm kare gönderilir. Bunun üç nedeni var: sonradan
//! bağlanan istemcinin görüntüyü toparlaması, paket kaybında kendini
//! düzeltme, ve özet çakışması yüzünden atlanmış olabilecek bir değişikliğin
//! en geç bir sonraki anahtar karede kapanması.

use anyhow::Result;
use jpeg_encoder::{ColorType, Encoder};

use crate::encode::{scale_frame, PixelLayout, RawFrame};

/// Değişiklik tespitinin çözünürlüğü (piksel).
///
/// Küçük karo = daha isabetli tespit, ama daha çok özet hesabı. 64, sınıf
/// içeriğinde imleç ve metin satırı boyutlarına iyi oturuyor.
pub const TILE: u32 = 64;

/// Değişen alan karenin bu oranını aşarsa tüm kare gönderilir.
///
/// Çok sayıda dağınık dikdörtgen göndermek, tek bir tam kareden pahalı hâle
/// gelebilir: her dikdörtgenin kendi JPEG başlığı vardır.
const KEYFRAME_RATIO: f32 = 0.6;

/// Kodlanmış bir ekran bölgesi.
pub struct EncodedRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub jpeg: Vec<u8>,
}

/// Bir kareye ait değişiklik kümesi.
pub struct ScreenUpdate {
    pub width: u32,
    pub height: u32,
    /// Bölgeler tüm kareyi kapsıyor mu?
    pub keyframe: bool,
    pub rects: Vec<EncodedRect>,
    /// Gönderilecek toplam sıkıştırılmış bayt (verim sınırı için).
    pub bytes: usize,
}

/// Kareler arası durumu taşıyan kodlayıcı.
pub struct TileEncoder {
    tile: u32,
    width: u32,
    height: u32,
    cols: u32,
    rows: u32,
    /// Bir önceki karenin karo özetleri.
    hashes: Vec<u64>,
    primed: bool,
}

impl TileEncoder {
    pub fn new() -> Self {
        Self {
            tile: TILE,
            width: 0,
            height: 0,
            cols: 0,
            rows: 0,
            hashes: Vec::new(),
            primed: false,
        }
    }

    /// Bir sonraki karenin anahtar kare olmasını zorlar.
    ///
    /// Kalite veya çözünürlük değiştiğinde çağrılır: eski özetler artık
    /// karşılaştırılabilir değildir.
    pub fn invalidate(&mut self) {
        self.primed = false;
    }

    /// Kareyi ölçekler, değişen bölgeleri bulur ve kodlar.
    ///
    /// Hiçbir şey değişmediyse `None` döner.
    pub fn encode(
        &mut self,
        frame: &RawFrame,
        max_width: u32,
        max_height: u32,
        quality: u8,
        force_keyframe: bool,
    ) -> Result<Option<ScreenUpdate>> {
        let scaled = scale_frame(frame, max_width, max_height)?;

        let dims_changed = scaled.width != self.width || scaled.height != self.height;
        if dims_changed {
            self.reshape(scaled.width, scaled.height);
        }

        let mut keyframe = force_keyframe || dims_changed || !self.primed;

        // Karo özetlerini hesapla ve değişenleri işaretle.
        let mut changed = vec![false; (self.cols * self.rows) as usize];
        let mut changed_count = 0usize;

        for row in 0..self.rows {
            for col in 0..self.cols {
                let idx = (row * self.cols + col) as usize;
                let h = hash_tile(&scaled, self.tile, col, row);
                if keyframe || self.hashes[idx] != h {
                    changed[idx] = true;
                    changed_count += 1;
                }
                self.hashes[idx] = h;
            }
        }

        if changed_count == 0 {
            return Ok(None);
        }

        // Değişen alan büyükse tek parça göndermek daha ucuz.
        let ratio = changed_count as f32 / changed.len() as f32;
        if ratio >= KEYFRAME_RATIO {
            keyframe = true;
        }

        let regions = if keyframe {
            vec![(0, 0, scaled.width, scaled.height)]
        } else {
            merge_rects(
                &changed,
                self.cols,
                self.rows,
                self.tile,
                scaled.width,
                scaled.height,
            )
        };

        let color = match scaled.layout {
            PixelLayout::Rgba8 => ColorType::Rgba,
            PixelLayout::Bgra8 => ColorType::Bgra,
        };

        let mut rects = Vec::with_capacity(regions.len());
        let mut bytes = 0usize;
        let mut crop = Vec::new();

        for (x, y, w, h) in regions {
            crop_region(&scaled, x, y, w, h, &mut crop);
            let mut jpeg = Vec::with_capacity(w as usize * h as usize / 4);
            Encoder::new(&mut jpeg, quality.clamp(1, 100))
                .encode(&crop, w as u16, h as u16, color)
                .map_err(|err| anyhow::anyhow!("JPEG kodlaması başarısız: {err}"))?;
            bytes += jpeg.len();
            rects.push(EncodedRect {
                x,
                y,
                width: w,
                height: h,
                jpeg,
            });
        }

        self.primed = true;
        Ok(Some(ScreenUpdate {
            width: scaled.width,
            height: scaled.height,
            keyframe,
            rects,
            bytes,
        }))
    }

    fn reshape(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        self.cols = width.div_ceil(self.tile);
        self.rows = height.div_ceil(self.tile);
        self.hashes = vec![0; (self.cols * self.rows) as usize];
        self.primed = false;
    }
}

impl Default for TileEncoder {
    fn default() -> Self {
        Self::new()
    }
}

/// Bir karonun hızlı özeti (FNV-1a benzeri, 8 baytlık adımlarla).
///
/// Kriptografik değildir. Çakışma olasılığı düşüktür ve çakışma hâlinde en
/// kötü sonuç, o karonun bir sonraki anahtar kareye kadar eski kalmasıdır.
fn hash_tile(scaled: &crate::encode::Scaled, tile: u32, col: u32, row: u32) -> u64 {
    let x0 = col * tile;
    let y0 = row * tile;
    let x1 = (x0 + tile).min(scaled.width);
    let y1 = (y0 + tile).min(scaled.height);
    let stride = scaled.width as usize * 4;

    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for y in y0..y1 {
        let start = y as usize * stride + x0 as usize * 4;
        let end = y as usize * stride + x1 as usize * 4;
        let row_bytes = &scaled.pixels[start..end];

        let mut chunks = row_bytes.chunks_exact(8);
        for chunk in &mut chunks {
            let v = u64::from_le_bytes(chunk.try_into().expect("8 baytlık dilim"));
            h ^= v;
            h = h.wrapping_mul(0x100_0000_01b3);
        }
        for &b in chunks.remainder() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x100_0000_01b3);
        }
    }
    h
}

/// Değişen karoları en az sayıda dikdörtgene birleştirir.
///
/// Açgözlü algoritma: her değişen karodan başlayıp önce sağa, sonra tüm
/// sütun aralığı değişmiş olduğu sürece aşağıya genişler. Optimal değildir
/// ama tipik değişim desenlerinde (imleç, metin satırı, pencere) iyi sonuç
/// verir ve ucuzdur.
fn merge_rects(
    changed: &[bool],
    cols: u32,
    rows: u32,
    tile: u32,
    width: u32,
    height: u32,
) -> Vec<(u32, u32, u32, u32)> {
    let mut used = vec![false; changed.len()];
    let mut out = Vec::new();
    let at = |r: u32, c: u32| (r * cols + c) as usize;

    for row in 0..rows {
        for col in 0..cols {
            if !changed[at(row, col)] || used[at(row, col)] {
                continue;
            }

            // Sağa genişle.
            let mut last_col = col;
            while last_col + 1 < cols
                && changed[at(row, last_col + 1)]
                && !used[at(row, last_col + 1)]
            {
                last_col += 1;
            }

            // Tüm sütun aralığı değişmiş olduğu sürece aşağı genişle.
            let mut last_row = row;
            'grow: while last_row + 1 < rows {
                for c in col..=last_col {
                    if !changed[at(last_row + 1, c)] || used[at(last_row + 1, c)] {
                        break 'grow;
                    }
                }
                last_row += 1;
            }

            for r in row..=last_row {
                for c in col..=last_col {
                    used[at(r, c)] = true;
                }
            }

            let x = col * tile;
            let y = row * tile;
            let w = ((last_col + 1) * tile).min(width) - x;
            let h = ((last_row + 1) * tile).min(height) - y;
            out.push((x, y, w, h));
        }
    }
    out
}

/// Bölgeyi bitişik bir tampona kopyalar (JPEG kodlayıcı bitişik veri ister).
fn crop_region(scaled: &crate::encode::Scaled, x: u32, y: u32, w: u32, h: u32, out: &mut Vec<u8>) {
    let stride = scaled.width as usize * 4;
    let row_len = w as usize * 4;
    out.clear();
    out.reserve(row_len * h as usize);
    for row in y..(y + h) {
        let start = row as usize * stride + x as usize * 4;
        out.extend_from_slice(&scaled.pixels[start..start + row_len]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Düz renkli kare üretir.
    fn kare(w: u32, h: u32, renk: [u8; 4]) -> RawFrame {
        RawFrame {
            width: w,
            height: h,
            layout: PixelLayout::Rgba8,
            pixels: renk
                .iter()
                .copied()
                .cycle()
                .take((w * h * 4) as usize)
                .collect(),
        }
    }

    /// Belirtilen dikdörtgeni başka renge boyar.
    fn boya(frame: &mut RawFrame, x: u32, y: u32, w: u32, h: u32, renk: [u8; 4]) {
        let stride = frame.width as usize * 4;
        for row in y..(y + h) {
            for col in x..(x + w) {
                let i = row as usize * stride + col as usize * 4;
                frame.pixels[i..i + 4].copy_from_slice(&renk);
            }
        }
    }

    #[test]
    fn ilk_kare_anahtar_karedir() {
        let mut enc = TileEncoder::new();
        let f = kare(256, 256, [10, 20, 30, 255]);
        let u = enc.encode(&f, 1024, 1024, 70, false).unwrap().unwrap();
        assert!(u.keyframe);
        assert_eq!(u.rects.len(), 1);
        assert_eq!((u.rects[0].width, u.rects[0].height), (256, 256));
    }

    #[test]
    fn degismeyen_kare_hic_gonderilmez() {
        let mut enc = TileEncoder::new();
        let f = kare(256, 256, [10, 20, 30, 255]);
        enc.encode(&f, 1024, 1024, 70, false).unwrap().unwrap();
        assert!(
            enc.encode(&f, 1024, 1024, 70, false).unwrap().is_none(),
            "değişmeyen kare için paket üretilmemeli"
        );
    }

    #[test]
    fn tek_karo_degisince_tek_kucuk_bolge_gonderilir() {
        let mut enc = TileEncoder::new();
        let mut f = kare(256, 256, [10, 20, 30, 255]);
        enc.encode(&f, 1024, 1024, 70, false).unwrap();

        boya(&mut f, 0, 0, 10, 10, [200, 0, 0, 255]);
        let u = enc.encode(&f, 1024, 1024, 70, false).unwrap().unwrap();

        assert!(!u.keyframe);
        assert_eq!(u.rects.len(), 1);
        assert_eq!((u.rects[0].x, u.rects[0].y), (0, 0));
        assert_eq!((u.rects[0].width, u.rects[0].height), (TILE, TILE));
    }

    #[test]
    fn komsu_karolar_tek_dikdortgende_birlesir() {
        let mut enc = TileEncoder::new();
        let mut f = kare(256, 256, [10, 20, 30, 255]);
        enc.encode(&f, 1024, 1024, 70, false).unwrap();

        // Yan yana iki karoyu kapsayan bir şerit boya.
        boya(&mut f, 0, 0, 128, 10, [200, 0, 0, 255]);
        let u = enc.encode(&f, 1024, 1024, 70, false).unwrap().unwrap();

        assert_eq!(u.rects.len(), 1, "iki komşu karo tek dikdörtgen olmalı");
        assert_eq!(u.rects[0].width, 128);
    }

    #[test]
    fn ayrik_degisiklikler_ayri_bolgeler_uretir() {
        let mut enc = TileEncoder::new();
        let mut f = kare(256, 256, [10, 20, 30, 255]);
        enc.encode(&f, 1024, 1024, 70, false).unwrap();

        boya(&mut f, 0, 0, 10, 10, [200, 0, 0, 255]);
        boya(&mut f, 192, 192, 10, 10, [0, 200, 0, 255]);
        let u = enc.encode(&f, 1024, 1024, 70, false).unwrap().unwrap();

        assert_eq!(u.rects.len(), 2);
    }

    #[test]
    fn genis_degisiklik_anahtar_kareye_doner() {
        let mut enc = TileEncoder::new();
        let mut f = kare(256, 256, [10, 20, 30, 255]);
        enc.encode(&f, 1024, 1024, 70, false).unwrap();

        // Karenin tamamını değiştir.
        boya(&mut f, 0, 0, 256, 256, [200, 0, 0, 255]);
        let u = enc.encode(&f, 1024, 1024, 70, false).unwrap().unwrap();

        assert!(u.keyframe, "çoğunluk değiştiğinde tam kare gönderilmeli");
        assert_eq!(u.rects.len(), 1);
    }

    #[test]
    fn zorlanan_anahtar_kare_tum_kareyi_gonderir() {
        let mut enc = TileEncoder::new();
        let f = kare(256, 256, [10, 20, 30, 255]);
        enc.encode(&f, 1024, 1024, 70, false).unwrap();

        let u = enc.encode(&f, 1024, 1024, 70, true).unwrap().unwrap();
        assert!(u.keyframe);
    }

    #[test]
    fn cozunurluk_degisince_anahtar_kare_uretilir() {
        let mut enc = TileEncoder::new();
        let a = kare(256, 256, [10, 20, 30, 255]);
        enc.encode(&a, 1024, 1024, 70, false).unwrap();

        let b = kare(128, 128, [10, 20, 30, 255]);
        let u = enc.encode(&b, 1024, 1024, 70, false).unwrap().unwrap();
        assert!(u.keyframe);
        assert_eq!((u.width, u.height), (128, 128));
    }

    /// Gerçekçi bir masaüstü karesi (düz renk JPEG'i aşırı iyi sıkıştırır,
    /// karşılaştırmayı anlamsız kılardı).
    fn gercekci_kare(w: u32, h: u32) -> RawFrame {
        let mut pixels = vec![0u8; (w * h * 4) as usize];
        crate::capture::test_pattern_for_bench(&mut pixels, w, h, 210.0, 3, "LAB101");
        RawFrame {
            width: w,
            height: h,
            layout: PixelLayout::Rgba8,
            pixels,
        }
    }

    #[test]
    fn kucuk_degisiklik_tam_kareden_cok_daha_ucuzdur() {
        // Bu testin amacı yaklaşımın gerekçesini sayıyla korumak:
        // tek satır yazmak, tüm kareyi göndermekten kat kat ucuz olmalı.
        let mut enc = TileEncoder::new();
        let mut f = gercekci_kare(1280, 720);

        let tam = enc.encode(&f, 1920, 1080, 70, false).unwrap().unwrap();
        assert!(tam.keyframe);

        // Bir metin satırı kadar alanı değiştir.
        boya(&mut f, 100, 300, 400, 18, [230, 230, 230, 255]);
        let kismi = enc.encode(&f, 1920, 1080, 70, false).unwrap().unwrap();

        assert!(!kismi.keyframe);
        assert!(
            kismi.bytes * 8 < tam.bytes,
            "kısmi güncelleme {} bayt, tam kare {} bayt — beklenen kazanç yok",
            kismi.bytes,
            tam.bytes
        );
    }
}

/// Değişen bölge aktarımının kazancını ölçer.
///
/// CI'da koşmaz; elle çalıştırılır:
///
/// ```text
/// cargo test --profile testlab -p openclass-student -- --ignored --nocapture olcum
/// ```
#[cfg(test)]
mod olcum {
    use super::*;
    use std::time::Instant;

    fn gercekci_kare(w: u32, h: u32, kare_no: u64) -> RawFrame {
        let mut pixels = vec![0u8; (w * h * 4) as usize];
        crate::capture::test_pattern_for_bench(&mut pixels, w, h, 210.0, kare_no, "LAB101");
        RawFrame {
            width: w,
            height: h,
            layout: PixelLayout::Rgba8,
            pixels,
        }
    }

    fn boya(frame: &mut RawFrame, x: u32, y: u32, w: u32, h: u32, renk: [u8; 4]) {
        let stride = frame.width as usize * 4;
        for row in y..(y + h) {
            for col in x..(x + w) {
                let i = row as usize * stride + col as usize * 4;
                frame.pixels[i..i + 4].copy_from_slice(&renk);
            }
        }
    }

    fn rapor(ad: &str, bayt: usize, ms: f64, fps: u32) {
        println!(
            "{ad:<38} {:>7.1} KB/kare | {:>8.1} KB/s @{fps}fps | {:>5.2} ms | ~%{:.1} CPU",
            bayt as f64 / 1024.0,
            bayt as f64 * fps as f64 / 1024.0,
            ms,
            ms * fps as f64 / 10.0
        );
    }

    #[test]
    #[ignore = "ölçüm; elle çalıştırılır"]
    fn olcum_degisen_bolge_kazanci() {
        println!("\nKaynak: 1920x1080, odaklı profil (1080p q75)\n");

        // 1) Anahtar kare — tam kare maliyeti.
        let mut enc = TileEncoder::new();
        let temel = gercekci_kare(1920, 1080, 1);
        let basla = Instant::now();
        let tam = enc.encode(&temel, 1920, 1080, 75, true).unwrap().unwrap();
        rapor(
            "Anahtar kare (tam)",
            tam.bytes,
            basla.elapsed().as_secs_f64() * 1000.0,
            25,
        );

        // 2) Durağan ekran — hiç paket çıkmamalı.
        let basla = Instant::now();
        let bos = enc.encode(&temel, 1920, 1080, 75, false).unwrap();
        println!(
            "{:<38} {:>7} | {:>8} | {:>5.2} ms",
            "Durağan ekran",
            if bos.is_none() {
                "paket yok"
            } else {
                "PAKET VAR"
            },
            "0.0 KB/s",
            basla.elapsed().as_secs_f64() * 1000.0
        );

        // 3) Metin satırı yazma — tipik ders içeriği.
        let mut yazi = temel.clone_for_test();
        boya(&mut yazi, 120, 420, 520, 20, [235, 235, 235, 255]);
        let basla = Instant::now();
        let kismi = enc.encode(&yazi, 1920, 1080, 75, false).unwrap().unwrap();
        rapor(
            "Metin satırı yazma",
            kismi.bytes,
            basla.elapsed().as_secs_f64() * 1000.0,
            25,
        );

        // 4) Pencere kaydırma — ekranın dörtte biri değişiyor.
        let mut pencere = yazi.clone_for_test();
        boya(&mut pencere, 0, 540, 960, 540, [90, 110, 140, 255]);
        let basla = Instant::now();
        let orta = enc
            .encode(&pencere, 1920, 1080, 75, false)
            .unwrap()
            .unwrap();
        rapor(
            "Pencere kaydırma (1/4 ekran)",
            orta.bytes,
            basla.elapsed().as_secs_f64() * 1000.0,
            25,
        );

        // 5) Tam hareketli video — en kötü durum.
        let hareketli = gercekci_kare(1920, 1080, 90);
        let basla = Instant::now();
        let kotu = enc
            .encode(&hareketli, 1920, 1080, 75, false)
            .unwrap()
            .unwrap();
        rapor(
            "Tam hareket (en kötü)",
            kotu.bytes,
            basla.elapsed().as_secs_f64() * 1000.0,
            25,
        );

        println!();
    }
}
