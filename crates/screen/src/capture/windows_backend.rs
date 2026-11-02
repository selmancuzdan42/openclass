//! Windows ekran yakalama arka ucu.
//!
//! Önce Windows Graphics Capture (WGC) denenir; başarısız olursa DXGI
//! Desktop Duplication'a düşülür. Her iki yol da birincil ekranı yakalar.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::{anyhow, Result};
use parking_lot::Mutex;
use windows_capture::capture::{Context, GraphicsCaptureApiHandler};
use windows_capture::frame::Frame;
use windows_capture::graphics_capture_api::InternalCaptureControl;
use windows_capture::monitor::Monitor;
use windows_capture::settings::{
    ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings,
    MinimumUpdateIntervalSettings, SecondaryWindowSettings, Settings,
};

use crate::encode::{PixelLayout, RawFrame};

/// Yakalanan son karenin paylaşımlı yuvası.
///
/// Tek kare tutulur: gönderici yetişemezse ara kareler düşer, bellek sabit kalır.
type FrameSlot = Arc<Mutex<Option<RawFrame>>>;

pub struct Session {
    slot: FrameSlot,
    running: Arc<AtomicBool>,
    backend: &'static str,
    /// WGC durdurma bayrağı; DXGI yolunda kullanılmaz.
    halt: Option<Arc<AtomicBool>>,
}

impl Session {
    pub fn start_primary_monitor() -> Result<Self> {
        let slot: FrameSlot = Arc::new(Mutex::new(None));
        let running = Arc::new(AtomicBool::new(true));

        match start_wgc(Arc::clone(&slot), Arc::clone(&running)) {
            Ok(halt) => {
                tracing::info!("ekran yakalama başladı: Windows Graphics Capture");
                return Ok(Self {
                    slot,
                    running,
                    backend: "Windows Graphics Capture",
                    halt: Some(halt),
                });
            }
            Err(err) => {
                tracing::warn!(
                    error = %format!("{err:#}"),
                    "WGC başlatılamadı, DXGI Desktop Duplication deneniyor"
                );
            }
        }

        start_dxgi(Arc::clone(&slot), Arc::clone(&running))?;
        tracing::info!("ekran yakalama başladı: DXGI Desktop Duplication");
        Ok(Self {
            slot,
            running,
            backend: "DXGI Desktop Duplication",
            halt: None,
        })
    }

    pub fn take_latest(&self) -> Option<RawFrame> {
        self.slot.lock().take()
    }

    pub fn backend_name(&self) -> &'static str {
        self.backend
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    pub fn stop(self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(halt) = self.halt {
            halt.store(true, Ordering::Relaxed);
        }
    }
}

// ---------------------------------------------------------------------------
// Windows Graphics Capture
// ---------------------------------------------------------------------------

/// WGC geri çağrımlarını karşılayan tutamaç.
struct WgcHandler {
    slot: FrameSlot,
    running: Arc<AtomicBool>,
    /// `as_nopadding_buffer` için yeniden kullanılan geçici tampon;
    /// her karede yeniden ayırmamak için alanda tutulur.
    scratch: Vec<u8>,
    /// Tanılama: geri çağrımın kaç kez tetiklendiği ve kaç kez tampon
    /// alınamadığı. "Hiç kare gelmiyor" ile "kare geliyor ama okunamıyor"
    /// ayrımı bu sayaçlar olmadan yapılamıyor.
    callbacks: u64,
    buffer_failures: u64,
}

impl GraphicsCaptureApiHandler for WgcHandler {
    type Flags = (FrameSlot, Arc<AtomicBool>);
    type Error = std::io::Error;

    fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
        let (slot, running) = ctx.flags;
        Ok(Self {
            slot,
            running,
            scratch: Vec::new(),
            callbacks: 0,
            buffer_failures: 0,
        })
    }

    fn on_frame_arrived(
        &mut self,
        frame: &mut Frame,
        capture_control: InternalCaptureControl,
    ) -> Result<(), Self::Error> {
        if !self.running.load(Ordering::Relaxed) {
            capture_control.stop();
            return Ok(());
        }

        self.callbacks += 1;
        if self.callbacks == 1 {
            tracing::info!("WGC ilk kareyi bildirdi");
        }

        let buffer = match frame.buffer() {
            Ok(b) => b,
            Err(err) => {
                // Tek bir bozuk kare oturumu düşürmez. Ama sürekli
                // başarısız oluyorsa yayın sessizce ölür; ilk hata ve
                // sonrasında seyrek aralıklarla uyarılır.
                self.buffer_failures += 1;
                if self.buffer_failures == 1 || self.buffer_failures % 120 == 0 {
                    tracing::warn!(
                        error = %err,
                        hata_sayisi = self.buffer_failures,
                        "kare tamponu alınamadı — ekran verisi gönderilemiyor"
                    );
                }
                return Ok(());
            }
        };
        let width = buffer.width();
        let height = buffer.height();

        let mut scratch = std::mem::take(&mut self.scratch);
        let pixels = buffer.as_nopadding_buffer(&mut scratch).to_vec();
        self.scratch = scratch;

        *self.slot.lock() = Some(RawFrame {
            width,
            height,
            // Ayarlarda Rgba8 istendiği için dönüşüm GPU tarafında yapılır.
            layout: PixelLayout::Rgba8,
            pixels,
        });
        Ok(())
    }

    fn on_closed(&mut self) -> Result<(), Self::Error> {
        tracing::info!("WGC yakalama öğesi kapandı");
        self.running.store(false, Ordering::Relaxed);
        Ok(())
    }
}

/// WGC yakalamasını kendi iş parçacığında başlatır ve durdurma bayrağını döner.
fn start_wgc(slot: FrameSlot, running: Arc<AtomicBool>) -> Result<Arc<AtomicBool>> {
    let monitor = Monitor::primary().map_err(|e| anyhow!("birincil ekran bulunamadı: {e}"))?;

    let settings = Settings::new(
        monitor,
        // İmleç de yakalanır: öğretmenin öğrencinin nereye tıkladığını
        // görmesi dersin takibi için gerekli.
        CursorCaptureSettings::WithCursor,
        // Yakalanan ekranın etrafına sarı çerçeve çizilmez; öğrenciye
        // paylaşım durumu uygulamanın kendi arayüzünden bildirilir.
        DrawBorderSettings::WithoutBorder,
        SecondaryWindowSettings::Default,
        MinimumUpdateIntervalSettings::Default,
        DirtyRegionSettings::Default,
        ColorFormat::Rgba8,
        (slot, Arc::clone(&running)),
    );

    let control = WgcHandler::start_free_threaded(settings)
        .map_err(|e| anyhow!("WGC yakalaması başlatılamadı: {e}"))?;

    Ok(control.halt_handle())
}

// ---------------------------------------------------------------------------
// DXGI Desktop Duplication (yedek yol)
// ---------------------------------------------------------------------------

/// DXGI Desktop Duplication döngüsünü kendi iş parçacığında başlatır.
///
/// `acquire_next_frame` zaman aşımına uğraması normaldir ve ekranın
/// değişmediği anlamına gelir; hata sayılmaz.
fn start_dxgi(slot: FrameSlot, running: Arc<AtomicBool>) -> Result<()> {
    use windows_capture::dxgi_duplication_api::DxgiDuplicationApi;

    let monitor = Monitor::primary().map_err(|e| anyhow!("birincil ekran bulunamadı: {e}"))?;

    // İlk oluşturmayı burada dene: başarısızsa çağırana hemen hata dönsün,
    // "başladı" denip sessizce ölen bir iş parçacığı kalmasın.
    let mut duplication = DxgiDuplicationApi::new(monitor)
        .map_err(|e| anyhow!("DXGI Desktop Duplication başlatılamadı: {e}"))?;

    std::thread::Builder::new()
        .name("openclass-dxgi-capture".into())
        .spawn(move || {
            // İş parçacığı nasıl sonlanırsa sonlansın bayrak düşsün; aksi
            // hâlde istemci hiç kare gelmediği hâlde hata bildirmez.
            let _guard = crate::capture::RunningGuard(Arc::clone(&running));

            let mut scratch = Vec::new();
            while running.load(Ordering::Relaxed) {
                match duplication.acquire_next_frame(100) {
                    Ok(mut frame) => {
                        let buffer = match frame.buffer() {
                            Ok(b) => b,
                            Err(err) => {
                                tracing::debug!(error = %err, "DXGI kare tamponu alınamadı");
                                continue;
                            }
                        };
                        let width = buffer.width();
                        let height = buffer.height();
                        let pixels = buffer.as_nopadding_buffer(&mut scratch).to_vec();
                        *slot.lock() = Some(RawFrame {
                            width,
                            height,
                            // DXGI çıktısı BGRA8'dir.
                            layout: PixelLayout::Bgra8,
                            pixels,
                        });
                    }
                    Err(err) => {
                        // Zaman aşımı = ekran değişmedi. Diğer hatalarda
                        // masaüstü değişmiş olabilir (UAC, oturum kilidi);
                        // duplication yeniden kurulur.
                        let text = err.to_string();
                        if text.contains("Timeout") || text.contains("timeout") {
                            continue;
                        }
                        tracing::warn!(error = %text, "DXGI yakalama hatası, yeniden kuruluyor");
                        match duplication.recreate() {
                            Ok(fresh) => duplication = fresh,
                            Err(err) => {
                                tracing::error!(error = %err, "DXGI yeniden kurulamadı, yakalama durdu");
                                return;
                            }
                        }
                        std::thread::sleep(std::time::Duration::from_millis(250));
                    }
                }
            }
            tracing::info!("DXGI yakalama iş parçacığı sonlandı");
        })
        .map_err(|e| anyhow!("DXGI yakalama iş parçacığı başlatılamadı: {e}"))?;

    Ok(())
}
