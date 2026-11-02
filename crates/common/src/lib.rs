//! OpenClass Teacher ve Student uygulamalarının ortak altyapısı.
//!
//! Burada iş mantığı yoktur; yalnızca iki uygulamanın da ihtiyaç duyduğu
//! platform yardımcıları bulunur.

pub mod logging;
pub mod machine;
pub mod net;
pub mod paths;
pub mod secrets;
pub mod tls;

/// Uygulama sürümü (Cargo.toml'dan).
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
