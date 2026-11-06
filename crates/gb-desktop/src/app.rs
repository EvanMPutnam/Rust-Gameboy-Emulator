use std::fs;
use std::path::Path;
use std::time::Duration;

use gb_core::apu::ApuDiagnostics;
use gb_core::ppu::PpuFrameDiagnostics;
use gb_core::{CartridgeSave, Cycles, GameBoy, GbConfig, GbError};

use crate::audio::{AudioQueue, AudioStats};
use crate::error::DesktopError;
use crate::input::{HostInputEvent, InputMap};
use crate::runloop::RunLoop;
use crate::video::VideoPresenter;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TickSummary {
    pub cycles_ran: Cycles,
    pub frames_presented: u64,
    pub audio_samples_enqueued: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioPipelineDiagnostics {
    pub core: Option<ApuDiagnostics>,
    pub queue: AudioStats,
}

pub struct DesktopApp {
    gameboy: GameBoy,
    video: VideoPresenter,
    audio: AudioQueue,
    runloop: RunLoop,
    audio_scratch: Vec<i16>,
    latest_ppu_diagnostics: Option<PpuFrameDiagnostics>,
    /// Wall time not yet passed to the cartridge RTC, which counts whole seconds.
    rtc_pending: Duration,
}

impl DesktopApp {
    pub fn from_rom_bytes(
        rom: Vec<u8>,
        boot_rom: Option<&[u8; 256]>,
    ) -> Result<Self, DesktopError> {
        let mut config = GbConfig::default();
        if let Some(boot) = boot_rom {
            config.use_boot_rom = true;
            config.force_post_boot_state = false;
            config.boot_rom = Some(*boot);
        }

        let gameboy = GameBoy::new(config, rom)?;
        Ok(Self {
            gameboy,
            video: VideoPresenter::new(),
            audio: AudioQueue::new(48_000),
            runloop: RunLoop::new(),
            audio_scratch: Vec::new(),
            latest_ppu_diagnostics: None,
            rtc_pending: Duration::ZERO,
        })
    }

    pub fn load_save_from_path(&mut self, path: &Path) -> Result<(), DesktopError> {
        let save_ram =
            fs::read(path).map_err(|err| DesktopError::io("reading save-in", path, err))?;
        if save_ram.is_empty() {
            return Ok(());
        }

        // A raw .sav holds only RAM, so keep the cartridge's current RTC state.
        let save = CartridgeSave {
            ram: Some(save_ram),
            rtc: self.gameboy.export_save()?.rtc,
        };
        self.gameboy.import_save(&save)?;
        Ok(())
    }

    pub fn write_save_to_path(&self, path: &Path) -> Result<(), DesktopError> {
        let save = self.gameboy.export_save()?;
        let payload = save.ram.unwrap_or_default();
        fs::write(path, payload).map_err(|err| DesktopError::io("writing save-out", path, err))
    }

    pub fn save_snapshot_to_path(&self, path: &Path) -> Result<(), DesktopError> {
        let snapshot = self.gameboy.export_snapshot()?;
        fs::write(path, snapshot).map_err(|err| DesktopError::io("writing snapshot", path, err))
    }

    pub fn load_snapshot_from_path(&mut self, path: &Path) -> Result<(), DesktopError> {
        let snapshot =
            fs::read(path).map_err(|err| DesktopError::io("reading snapshot", path, err))?;
        self.gameboy.import_snapshot(&snapshot)?;
        Ok(())
    }

    pub fn handle_input(&mut self, event: HostInputEvent) -> Result<(), GbError> {
        if let Some((button, pressed)) = InputMap::map_event(event) {
            self.gameboy.set_button(button, pressed)?;
        }
        Ok(())
    }

    pub fn tick(&mut self, elapsed: Duration) -> Result<TickSummary, GbError> {
        // The cartridge clock runs in real time, independent of emulation speed.
        self.rtc_pending += elapsed;
        let rtc_seconds = self.rtc_pending.as_secs();
        if rtc_seconds > 0 {
            self.gameboy.tick_rtc(rtc_seconds)?;
            self.rtc_pending -= Duration::from_secs(rtc_seconds);
        }

        let cycle_budget = self.runloop.cycle_budget(elapsed);
        let cycles_ran = if cycle_budget == 0 {
            0
        } else {
            self.gameboy.run_cycles(cycle_budget)?
        };

        let mut frames_presented = 0u64;
        while let Some(frame) = self.gameboy.take_frame() {
            self.video.present(frame);
            if let Some(diagnostics) = self.gameboy.take_ppu_frame_diagnostics() {
                self.latest_ppu_diagnostics = Some(diagnostics);
            }
            frames_presented = frames_presented.wrapping_add(1);
        }

        self.audio_scratch.clear();
        self.gameboy
            .drain_audio_samples_into(&mut self.audio_scratch);
        let audio_samples_enqueued = self.audio_scratch.len();
        self.audio.enqueue(&self.audio_scratch);

        Ok(TickSummary {
            cycles_ran,
            frames_presented,
            audio_samples_enqueued,
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.video.resize(width, height);
    }

    pub fn set_video_diagnostics_enabled(&mut self, enabled: bool) {
        self.gameboy.set_ppu_diagnostics_enabled(enabled);
        self.video.set_diagnostics_enabled(enabled);
        if !enabled {
            self.latest_ppu_diagnostics = None;
        }
    }

    pub fn set_audio_diagnostics_enabled(&mut self, enabled: bool, include_channel_stats: bool) {
        self.gameboy
            .set_apu_diagnostics_enabled(enabled, include_channel_stats);
    }

    #[must_use]
    pub fn total_cycles(&self) -> Cycles {
        self.gameboy.clock.now()
    }

    #[must_use]
    pub fn audio_queue(&self) -> AudioQueue {
        self.audio.clone()
    }

    pub fn set_emulation_speed_multiplier(&mut self, multiplier: u32) {
        self.runloop.set_speed_multiplier(multiplier);
        // Fast-forward fills the queue; drop the backlog so audio doesn't lag behind video.
        self.audio.clear();
    }

    #[must_use]
    pub fn emulation_speed_multiplier(&self) -> u32 {
        self.runloop.speed_multiplier()
    }

    pub fn set_audio_muted(&self, muted: bool) {
        self.audio.set_muted(muted);
    }

    #[must_use]
    pub fn audio_muted(&self) -> bool {
        self.audio.is_muted()
    }

    #[must_use]
    pub fn audio_diagnostics(&self) -> AudioPipelineDiagnostics {
        AudioPipelineDiagnostics {
            core: self.gameboy.apu_diagnostics(),
            queue: self.audio.stats(),
        }
    }

    #[cfg(test)]
    #[must_use]
    pub fn video(&self) -> &VideoPresenter {
        &self.video
    }

    #[must_use]
    pub fn video_mut(&mut self) -> &mut VideoPresenter {
        &mut self.video
    }

    #[must_use]
    pub fn take_ppu_frame_diagnostics(&mut self) -> Option<PpuFrameDiagnostics> {
        self.latest_ppu_diagnostics.take()
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use super::DesktopApp;
    use crate::input::{HostInputEvent, HostKey};
    use gb_core::bus::Memory;

    fn test_rom() -> Vec<u8> {
        let mut rom = vec![0u8; 0x8000];
        rom[0x0147] = 0x00;
        rom[0x0148] = 0x00;
        rom[0x0149] = 0x00;
        rom[0x0100] = 0x00;
        rom
    }

    fn temp_path(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("{label}_{nanos}"))
    }

    #[test]
    fn startup_shutdown_smoke_path_is_stable() {
        let mut app = DesktopApp::from_rom_bytes(test_rom(), None).expect("init app");
        let first = app.tick(Duration::from_millis(2)).expect("tick");
        let second = app.tick(Duration::from_millis(2)).expect("tick");

        assert!(first.cycles_ran > 0);
        assert!(second.cycles_ran > 0);
    }

    #[test]
    fn input_path_maps_to_core_buttons() {
        let mut app = DesktopApp::from_rom_bytes(test_rom(), None).expect("init app");
        app.handle_input(HostInputEvent::KeyDown(HostKey::Z))
            .expect("press");
        let _ = app.tick(Duration::from_millis(1)).expect("tick");
        app.handle_input(HostInputEvent::KeyUp(HostKey::Z))
            .expect("release");
    }

    #[test]
    fn resize_smoke_preserves_aspect_and_stays_stable() {
        let mut app = DesktopApp::from_rom_bytes(test_rom(), None).expect("init app");
        app.resize(1024, 768);
        app.resize(800, 600);

        let size = app.video().present_size();
        assert_eq!(size.width * 9, size.height * 10);
    }

    #[test]
    fn snapshot_save_and_load_round_trip() {
        let mut app = DesktopApp::from_rom_bytes(test_rom(), None).expect("init app");
        let path = temp_path("desktop_snapshot");

        app.save_snapshot_to_path(&path).expect("save snapshot");
        app.load_snapshot_from_path(&path).expect("load snapshot");

        std::fs::remove_file(path).expect("remove snapshot");
    }

    fn rtc_rom() -> Vec<u8> {
        let mut rom = test_rom();
        rom[0x0147] = 0x10; // MBC3 + timer + RAM + battery
        rom[0x0149] = 0x02; // 8 KiB RAM
        rom
    }

    fn read_rtc_seconds(app: &mut DesktopApp) -> u8 {
        let bus = &mut app.gameboy.bus;
        bus.write8(0x0000, 0x0A).expect("enable ram");
        bus.write8(0x4000, 0x08).expect("select seconds");
        bus.write8(0x6000, 0x00).expect("arm latch");
        bus.write8(0x6000, 0x01).expect("latch");
        bus.read8(0xA000).expect("read seconds")
    }

    #[test]
    fn raw_save_loads_on_rtc_cartridge() {
        let mut app = DesktopApp::from_rom_bytes(rtc_rom(), None).expect("init app");
        let path = temp_path("desktop_rtc_save");
        std::fs::write(&path, vec![0x5A; 8 * 1024]).expect("write save");

        app.load_save_from_path(&path).expect("load save");

        std::fs::remove_file(path).expect("remove save");
    }

    #[test]
    fn rtc_advances_with_wall_time() {
        let mut app = DesktopApp::from_rom_bytes(rtc_rom(), None).expect("init app");
        app.tick(Duration::from_millis(700)).expect("tick");
        assert_eq!(read_rtc_seconds(&mut app), 0);

        app.tick(Duration::from_millis(700)).expect("tick");
        assert_eq!(read_rtc_seconds(&mut app), 1);
    }

    #[test]
    fn runtime_controls_update_speed_and_mute_state() {
        let mut app = DesktopApp::from_rom_bytes(test_rom(), None).expect("init app");
        app.set_emulation_speed_multiplier(8);
        assert_eq!(app.emulation_speed_multiplier(), 8);

        app.set_audio_muted(true);
        assert!(app.audio_muted());
        app.set_audio_muted(false);
        assert!(!app.audio_muted());
    }
}
