//! Windows dışı platformlar için yakalama arka ucu.
//!
//! Bu arka uç **hiçbir kare üretmez**. Amacı, geliştirme sırasında
//! uygulamanın Linux/macOS üzerinde derlenip çalışabilmesidir; ekran
//! paylaşımı özelliği bu platformlarda yoktur ve sahte görüntü üretilmez.

use anyhow::{bail, Result};

use crate::encode::RawFrame;

pub struct Session;

impl Session {
    pub fn start_primary_monitor() -> Result<Self> {
        // Hata mesajı ne yapılacağını da söyler: yalnızca "yakalanamadı"
        // demek kullanıcıyı çıkmaz sokağa bırakır.
        bail!(
            "Ekran yakalama yalnızca Windows'ta desteklenir; bu sistemde \
             gerçek ekran alınamaz. Kare hattını burada denemek için \
             uygulamayı {}=test ile başlatın — sentetik bir test deseni \
             yayınlanır ve arayüzde \"test deseni\" olarak etiketlenir.",
            crate::capture::CAPTURE_OVERRIDE_VAR
        )
    }

    pub fn take_latest(&self) -> Option<RawFrame> {
        None
    }

    pub fn backend_name(&self) -> &'static str {
        "desteklenmiyor"
    }

    pub fn is_running(&self) -> bool {
        false
    }

    pub fn stop(self) {}
}
