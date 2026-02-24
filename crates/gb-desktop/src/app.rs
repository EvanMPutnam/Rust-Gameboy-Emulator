use std::fs;
use std::path::Path;
use std::time::Duration;

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
pub struct CoreAudioDiagnostics {
    pub total_samples: u64,
    pub sample_sum: i64,
    pub clipped_samples: u64,
    pub amplitude_buckets: [u64; 5],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioPipelineDiagnostics {
    pub core: CoreAudioDiagnostics,
    pub queue: AudioStats,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CoreAudioAccumulator {
    total_samples: u64,
    sample_sum: i64,
    clipped_samples: u64,
    amplitude_buckets: [u64; 5],
}

impl CoreAudioAccumulator {
    fn observe(&mut self, samples: &[i16]) {
        for &sample in samples {
            self.total_samples = self.total_samples.wrapping_add(1);
            self.sample_sum = self.sample_sum.wrapping_add(i64::from(sample));
            if sample == i16::MIN || sample == i16::MAX {
                self.clipped_samples = self.clipped_samples.wrapping_add(1);
            }

            let amplitude = i32::from(sample).abs();
            let bucket = if amplitude == 0 {
                0
            } else if amplitude <= 1_023 {
                1
            } else if amplitude <= 4_095 {
                2
            } else if amplitude <= 16_383 {
                3
            } else {
                4
            };
            self.amplitude_buckets[bucket] = self.amplitude_buckets[bucket].wrapping_add(1);
        }
    }

    fn snapshot(self) -> CoreAudioDiagnostics {
        CoreAudioDiagnostics {
            total_samples: self.total_samples,
            sample_sum: self.sample_sum,
            clipped_samples: self.clipped_samples,
            amplitude_buckets: self.amplitude_buckets,
        }
    }
}

pub struct DesktopApp {
    gameboy: GameBoy,
    video: VideoPresenter,
    audio: AudioQueue,
    runloop: RunLoop,
    audio_accumulator: CoreAudioAccumulator,
    latest_ppu_diagnostics: Option<PpuFrameDiagnostics>,
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
            audio_accumulator: CoreAudioAccumulator {
                total_samples: 0,
                sample_sum: 0,
                clipped_samples: 0,
                amplitude_buckets: [0; 5],
            },
            latest_ppu_diagnostics: None,
        })
    }

    pub fn load_save_from_path(&mut self, path: &Path) -> Result<(), DesktopError> {
        let save_ram =
            fs::read(path).map_err(|err| DesktopError::io("reading save-in", path, err))?;
        if save_ram.is_empty() {
            return Ok(());
        }

        let save = CartridgeSave {
            ram: Some(save_ram),
            rtc: None,
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

        let samples = self.gameboy.take_audio_samples();
        self.audio_accumulator.observe(&samples);
        let audio_samples_enqueued = samples.len();
        self.audio.enqueue(&samples);

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

    #[must_use]
    pub fn total_cycles(&self) -> Cycles {
        self.gameboy.clock.now()
    }

    #[must_use]
    pub fn audio_queue(&self) -> AudioQueue {
        self.audio.clone()
    }

    #[must_use]
    pub fn audio_diagnostics(&self) -> AudioPipelineDiagnostics {
        AudioPipelineDiagnostics {
            core: self.audio_accumulator.snapshot(),
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
}
