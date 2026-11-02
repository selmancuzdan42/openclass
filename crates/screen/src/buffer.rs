//! Alınan ekran güncellemelerinin tamponu.
//!
//! Hem öğretmen (öğrenci ekranlarını alır) hem öğrenci (öğretmen ekranını
//! alır) aynı tamponu kullanır.
//!
//! # Neden burada görüntü işlenmiyor?
//!
//! Öğrenciler ekranın yalnızca değişen bölgelerini gönderir. Bu bölgeleri
//! tam bir görüntüde birleştirmek gerekir, ama bunu Rust'ta yapmak 20 cihaz
//! için kare başına JPEG çözme + yeniden kodlama demektir — öğretmen
//! makinesini gereksiz yere yorar.
//!
//! Bunun yerine bölgeler olduğu gibi saklanır ve arayüze ham olarak verilir;
//! birleştirmeyi tarayıcı `<canvas>` üzerinde yapar. Tarayıcı JPEG çözmeyi
//! zaten donanım hızlandırmalı yapar, Rust tarafı hiç görüntü işlemez.
//!
//! # Tutarlılık
//!
//! Arayüz bir güncellemeyi kaçırırsa tuvali bozulur. Buna karşı:
//!
//! * Son **anahtar kare** ve ondan sonraki değişiklikler saklanır.
//! * Arayüz elindeki son sıra numarasını bildirir; eksik parçalar varsa
//!   anahtar kareden itibaren her şey verilir.
//! * Saklanan veri sınırı aşarsa birikim atılır ve öğrenciden yeni bir
//!   anahtar kare istenir.

use std::collections::VecDeque;
use std::time::Instant;

use openclass_protocol::control::ScreenRect;

/// Cihaz başına saklanacak azami veri.
///
/// Anahtar kare 1080p'de ~100 KB; buna 3 saniyelik değişiklik birikimi
/// eklenir. Sınır aşılırsa yeni anahtar kare istenir.
const MAX_STORED_BYTES: usize = 4 * 1024 * 1024;

/// Saklanan tek bir güncelleme.
pub struct StoredUpdate {
    pub seq: u64,
    pub keyframe: bool,
    pub rects: Vec<ScreenRect>,
    pub bytes: usize,
}

/// Bir kaynağın ekran durumu.
pub struct ScreenBuffer {
    pub width: u32,
    pub height: u32,
    pub latest_seq: u64,
    pub received_at: Instant,
    /// Son anahtar kare; yoksa henüz tam görüntü yok.
    keyframe: Option<StoredUpdate>,
    /// Anahtar kareden sonraki değişiklikler.
    deltas: VecDeque<StoredUpdate>,
    stored_bytes: usize,
    /// Birikim taştı; öğrenciden anahtar kare istenmeli.
    needs_keyframe: bool,
}

impl ScreenBuffer {
    pub fn new() -> Self {
        Self {
            width: 0,
            height: 0,
            latest_seq: 0,
            received_at: Instant::now(),
            keyframe: None,
            deltas: VecDeque::new(),
            stored_bytes: 0,
            needs_keyframe: false,
        }
    }

    /// Yeni güncellemeyi ekler.
    pub fn push(&mut self, update: StoredUpdate, width: u32, height: u32) {
        if width != self.width || height != self.height {
            // Çözünürlük değişti: eski birikim anlamsız.
            self.keyframe = None;
            self.deltas.clear();
            self.stored_bytes = 0;
            self.width = width;
            self.height = height;
        }

        self.latest_seq = update.seq;
        self.received_at = Instant::now();

        if update.keyframe {
            self.deltas.clear();
            self.stored_bytes = update.bytes;
            self.keyframe = Some(update);
            self.needs_keyframe = false;
            return;
        }

        if self.keyframe.is_none() {
            // Anahtar kare olmadan gelen değişiklik çizilemez.
            self.needs_keyframe = true;
            return;
        }

        self.stored_bytes += update.bytes;
        self.deltas.push_back(update);

        while self.stored_bytes > MAX_STORED_BYTES {
            match self.deltas.pop_front() {
                Some(dropped) => {
                    self.stored_bytes = self.stored_bytes.saturating_sub(dropped.bytes);
                    // Aradan parça düştüğü için birikim artık tutarlı değil.
                    self.needs_keyframe = true;
                }
                None => break,
            }
        }
    }

    /// Öğrenciden yeni bir anahtar kare istenmeli mi?
    pub fn needs_keyframe(&self) -> bool {
        self.needs_keyframe
    }

    pub fn clear_keyframe_request(&mut self) {
        self.needs_keyframe = false;
    }

    /// Arayüzün elindeki `since` sırasından sonrasını ikili olarak paketler.
    ///
    /// Arayüz geride kaldıysa veya hiç görüntüsü yoksa anahtar kareden
    /// itibaren her şey verilir. Hiç veri yoksa `None` döner.
    pub fn encode_since(&self, since: u64) -> Option<Vec<u8>> {
        let keyframe = self.keyframe.as_ref()?;

        if since >= self.latest_seq {
            // Arayüz zaten güncel.
            return None;
        }

        // Birikim yalnızca baştan parça düşmediyse kesintisizdir; düştüyse
        // `needs_keyframe` zaten işaretlenmiştir. Arayüz anahtar kareyi
        // görmemişse de artımlı gönderilemez.
        let incremental = !self.needs_keyframe && since >= keyframe.seq;
        let send_keyframe = !incremental;

        let mut out = Vec::with_capacity(self.stored_bytes + 256);
        out.extend_from_slice(b"OCU1");
        out.push(u8::from(send_keyframe));
        out.extend_from_slice(&(self.width as u16).to_le_bytes());
        out.extend_from_slice(&(self.height as u16).to_le_bytes());

        let selected: Vec<&StoredUpdate> = if send_keyframe {
            std::iter::once(keyframe)
                .chain(self.deltas.iter())
                .collect()
        } else {
            self.deltas.iter().filter(|d| d.seq > since).collect()
        };

        if selected.is_empty() {
            return None;
        }

        out.extend_from_slice(&(selected.len() as u32).to_le_bytes());
        for update in selected {
            out.extend_from_slice(&update.seq.to_le_bytes());
            out.push(u8::from(update.keyframe));
            out.extend_from_slice(&(update.rects.len() as u16).to_le_bytes());
            for rect in &update.rects {
                out.extend_from_slice(&(rect.x as u16).to_le_bytes());
                out.extend_from_slice(&(rect.y as u16).to_le_bytes());
                out.extend_from_slice(&(rect.width as u16).to_le_bytes());
                out.extend_from_slice(&(rect.height as u16).to_le_bytes());
                out.extend_from_slice(&(rect.data.len() as u32).to_le_bytes());
                out.extend_from_slice(&rect.data);
            }
        }
        Some(out)
    }
}

impl Default for ScreenBuffer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(len: usize) -> ScreenRect {
        ScreenRect {
            x: 0,
            y: 0,
            width: 64,
            height: 64,
            data: vec![0xAB; len],
        }
    }

    fn update(seq: u64, keyframe: bool, len: usize) -> StoredUpdate {
        StoredUpdate {
            seq,
            keyframe,
            rects: vec![rect(len)],
            bytes: len,
        }
    }

    #[test]
    fn anahtar_kare_olmadan_cizim_yapilamaz() {
        let mut s = ScreenBuffer::new();
        s.push(update(1, false, 100), 640, 480);
        assert!(s.needs_keyframe());
        assert!(s.encode_since(0).is_none());
    }

    #[test]
    fn anahtar_kare_sonrasi_paket_uretilir() {
        let mut s = ScreenBuffer::new();
        s.push(update(1, true, 500), 640, 480);
        let blob = s.encode_since(0).unwrap();
        assert_eq!(&blob[..4], b"OCU1");
        assert_eq!(
            blob[4], 1,
            "arayüzde görüntü yokken anahtar kare gönderilmeli"
        );
    }

    #[test]
    fn anahtar_kare_birikimi_sifirlar() {
        let mut s = ScreenBuffer::new();
        s.push(update(1, true, 500), 640, 480);
        s.push(update(2, false, 100), 640, 480);
        s.push(update(3, true, 500), 640, 480);
        // Yeni anahtar kare geldiğinde eski değişiklikler atılır.
        assert_eq!(s.latest_seq, 3);
        assert!(!s.needs_keyframe());
    }

    #[test]
    fn cozunurluk_degisince_birikim_atilir() {
        let mut s = ScreenBuffer::new();
        s.push(update(1, true, 500), 640, 480);
        s.push(update(2, false, 100), 1280, 720);
        assert_eq!((s.width, s.height), (1280, 720));
        assert!(s.needs_keyframe(), "yeni çözünürlükte anahtar kare gerekir");
    }

    #[test]
    fn sinir_asilinca_anahtar_kare_istenir() {
        let mut s = ScreenBuffer::new();
        s.push(update(1, true, 1024), 640, 480);
        for seq in 2..200 {
            s.push(update(seq, false, 64 * 1024), 640, 480);
        }
        assert!(s.needs_keyframe(), "birikim taşınca anahtar kare istenmeli");
    }

    #[test]
    fn istek_sonrasi_bayrak_temizlenir() {
        let mut s = ScreenBuffer::new();
        s.push(update(1, false, 100), 640, 480);
        assert!(s.needs_keyframe());
        s.clear_keyframe_request();
        assert!(!s.needs_keyframe());
    }
}
