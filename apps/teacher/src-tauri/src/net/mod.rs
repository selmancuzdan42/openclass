//! Ağ katmanı.
//!
//! Keşif, bağlantı ve ekran yayını [`docs/ARCHITECTURE.md`] uyarınca ayrı
//! katmanlardır: keşif çalışmasa da bağlantı kurulabilir, bağlantı varken
//! ekran yayını olmayabilir.

pub mod discovery;
pub mod server;
