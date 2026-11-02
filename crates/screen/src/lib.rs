//! Ekran yakalama ve aktarıma hazırlama.
//!
//! Hem öğrenci ajanı (kendi ekranını öğretmene gönderir) hem öğretmen konsolu
//! (kendi ekranını öğrencilere yayınlar) aynı boru hattını kullanır:
//!
//! ```text
//! yakalama  →  ölçekleme  →  değişen bölge tespiti  →  JPEG
//! (capture)    (encode)      (tiles)
//! ```
//!
//! Bu crate iki uygulama arasında paylaşılır; aksi hâlde aynı kod iki yerde
//! durur ve ölçüm sonuçlarıyla ayarlanmış ödünleşimler (kutu filtresi eşiği,
//! karo boyutu, anahtar kare aralığı) ikiye ayrılırdı.

pub mod buffer;
pub mod capture;
pub mod encode;
pub mod tiles;

pub use buffer::{ScreenBuffer, StoredUpdate};
pub use capture::{is_supported, CaptureSession, CAPTURE_OVERRIDE_VAR};
pub use encode::{PixelLayout, RawFrame};
pub use tiles::{EncodedRect, ScreenUpdate, TileEncoder, TILE};
