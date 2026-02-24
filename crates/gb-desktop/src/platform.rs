use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use pixels::{Pixels, SurfaceTexture};
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

use crate::app::DesktopApp;
use crate::audio::AudioBackend;
use crate::error::DesktopError;
use crate::input::{HostInputEvent, HostKey};

const DEFAULT_WINDOW_WIDTH: u32 = 1_280;
const DEFAULT_WINDOW_HEIGHT: u32 = 720;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchOptions {
    pub rom_path: PathBuf,
    pub boot_rom_path: Option<PathBuf>,
    pub save_in_path: Option<PathBuf>,
    pub save_out_path: Option<PathBuf>,
    pub max_cycles: Option<u64>,
    pub mute: bool,
    pub audio_diagnostics: bool,
    pub video_diagnostics: bool,
}

pub fn run(options: LaunchOptions) -> Result<(), DesktopError> {
    let rom = fs::read(&options.rom_path)
        .map_err(|err| DesktopError::io("reading rom", &options.rom_path, err))?;
    let boot_rom = load_boot_rom(options.boot_rom_path.as_deref())?;

    let mut app = DesktopApp::from_rom_bytes(rom, boot_rom.as_ref())?;
    app.set_video_diagnostics_enabled(options.video_diagnostics);
    if let Some(path) = &options.save_in_path {
        app.load_save_from_path(path)?;
    }

    let audio_queue = app.audio_queue();
    let audio = AudioBackend::start(&audio_queue, options.mute)?;
    let event_loop = EventLoop::new().map_err(DesktopError::WindowEventLoop)?;
    let snapshot_path = snapshot_path_for_rom(&options.rom_path);
    let mut runtime = DesktopRuntime::new(options, snapshot_path, app, audio);
    event_loop
        .run_app(&mut runtime)
        .map_err(DesktopError::WindowEventLoop)?;
    runtime.finish()
}

fn load_boot_rom(path: Option<&Path>) -> Result<Option<[u8; 256]>, DesktopError> {
    let Some(path) = path else {
        return Ok(None);
    };

    let bytes = fs::read(path).map_err(|err| DesktopError::io("reading boot rom", path, err))?;
    if bytes.len() != 256 {
        return Err(DesktopError::InvalidBootRomLength(bytes.len()));
    }

    let mut boot = [0u8; 256];
    boot.copy_from_slice(&bytes);
    Ok(Some(boot))
}

#[must_use]
fn snapshot_path_for_rom(rom_path: &Path) -> PathBuf {
    rom_path.with_extension("gbsnap")
}

struct DesktopRuntime {
    options: LaunchOptions,
    snapshot_path: PathBuf,
    app: DesktopApp,
    _audio: Option<AudioBackend>,
    window: Option<Arc<Window>>,
    window_id: Option<WindowId>,
    pixels: Option<Pixels<'static>>,
    surface_size: PhysicalSize<u32>,
    last_tick: Instant,
    last_audio_diag_report: Instant,
    last_audio_diag_checkpoint: Option<AudioDiagCheckpoint>,
    pending_error: Option<DesktopError>,
}

#[derive(Debug, Clone, Copy)]
struct AudioDiagCheckpoint {
    enqueued_samples: u64,
    dequeued_samples: u64,
    callback_requested_samples: u64,
    dropped_samples: u64,
    underflow_events: u64,
    callback_calls: u64,
}

impl DesktopRuntime {
    fn new(
        options: LaunchOptions,
        snapshot_path: PathBuf,
        app: DesktopApp,
        audio: Option<AudioBackend>,
    ) -> Self {
        let now = Instant::now();
        Self {
            options,
            snapshot_path,
            app,
            _audio: audio,
            window: None,
            window_id: None,
            pixels: None,
            surface_size: PhysicalSize::new(DEFAULT_WINDOW_WIDTH, DEFAULT_WINDOW_HEIGHT),
            last_tick: now,
            last_audio_diag_report: now,
            last_audio_diag_checkpoint: None,
            pending_error: None,
        }
    }

    fn finish(self) -> Result<(), DesktopError> {
        if let Some(path) = &self.options.save_out_path {
            self.app.write_save_to_path(path)?;
        }
        if let Some(error) = self.pending_error {
            return Err(error);
        }
        Ok(())
    }

    fn set_error(&mut self, event_loop: &ActiveEventLoop, error: DesktopError) {
        if self.pending_error.is_none() {
            self.pending_error = Some(error);
        }
        event_loop.exit();
    }

    fn initialize_window(&mut self, event_loop: &ActiveEventLoop) -> Result<(), DesktopError> {
        if self.window.is_some() {
            return Ok(());
        }

        let attrs = Window::default_attributes()
            .with_title("gb-desktop")
            .with_inner_size(PhysicalSize::new(
                DEFAULT_WINDOW_WIDTH,
                DEFAULT_WINDOW_HEIGHT,
            ))
            .with_min_inner_size(PhysicalSize::new(320, 288));
        let window = Arc::new(
            event_loop
                .create_window(attrs)
                .map_err(DesktopError::WindowCreate)?,
        );
        let size = window.inner_size();
        let surface = SurfaceTexture::new(size.width.max(1), size.height.max(1), window.clone());
        let pixels = Pixels::new(size.width.max(1), size.height.max(1), surface)
            .map_err(DesktopError::PixelsCreate)?;

        self.window_id = Some(window.id());
        self.window = Some(window);
        self.pixels = Some(pixels);
        self.surface_size = size;
        self.app.resize(size.width.max(1), size.height.max(1));
        self.last_tick = Instant::now();
        Ok(())
    }

    fn handle_resize(&mut self, event_loop: &ActiveEventLoop, size: PhysicalSize<u32>) {
        let width = size.width.max(1);
        let height = size.height.max(1);
        self.surface_size = PhysicalSize::new(width, height);
        self.app.resize(width, height);

        let Some(pixels) = self.pixels.as_mut() else {
            return;
        };
        if let Err(err) = pixels.resize_surface(width, height) {
            self.set_error(event_loop, DesktopError::PixelsResize(err));
            return;
        }
        if let Err(err) = pixels.resize_buffer(width, height) {
            self.set_error(event_loop, DesktopError::PixelsResize(err));
        }
    }

    fn handle_keyboard(
        &mut self,
        event_loop: &ActiveEventLoop,
        state: ElementState,
        repeat: bool,
        physical_key: PhysicalKey,
    ) {
        let PhysicalKey::Code(code) = physical_key else {
            return;
        };

        if state == ElementState::Pressed && !repeat {
            if code == KeyCode::Escape {
                event_loop.exit();
                return;
            }
            if code == KeyCode::F5 {
                if let Err(err) = self.app.save_snapshot_to_path(&self.snapshot_path) {
                    self.set_error(event_loop, err);
                }
                return;
            }
            if code == KeyCode::F9 {
                if let Err(err) = self.app.load_snapshot_from_path(&self.snapshot_path) {
                    self.set_error(event_loop, err);
                }
                return;
            }
        }

        let Some(host_key) = map_key_code(code) else {
            return;
        };
        let mapped_event = if state == ElementState::Pressed {
            HostInputEvent::KeyDown(host_key)
        } else {
            HostInputEvent::KeyUp(host_key)
        };
        if let Err(err) = self.app.handle_input(mapped_event) {
            self.set_error(event_loop, DesktopError::Core(err));
        }
    }

    fn step_and_render(&mut self, event_loop: &ActiveEventLoop) {
        let now = Instant::now();
        let elapsed = now.saturating_duration_since(self.last_tick);
        self.last_tick = now;

        if let Err(err) = self.app.tick(elapsed) {
            self.set_error(event_loop, DesktopError::Core(err));
            return;
        }
        self.maybe_report_audio_diagnostics(now);

        if let Some(limit) = self.options.max_cycles
            && u64::from(self.app.total_cycles()) >= limit
        {
            event_loop.exit();
            return;
        }

        let render_result = {
            let Some(pixels) = self.pixels.as_mut() else {
                return;
            };
            {
                let frame = pixels.frame_mut();
                self.app.video_mut().render_rgba(
                    frame,
                    self.surface_size.width,
                    self.surface_size.height,
                );
            }
            pixels.render()
        };
        if self.options.video_diagnostics {
            self.report_video_diagnostics();
        }
        if let Err(err) = render_result {
            self.set_error(event_loop, DesktopError::PixelsRender(err));
        }
    }

    fn report_video_diagnostics(&mut self) {
        if let Some(frame) = self.app.take_ppu_frame_diagnostics() {
            eprintln!(
                "ppu diag: frame={} scanlines={}",
                frame.frame_number,
                frame.scanlines.len()
            );
            for line in &frame.scanlines {
                eprintln!(
                    "ppu line: ly={} mode={:?} lyc_match={} window_line={} bg_map=0x{:04X} window_map=0x{:04X} tile_addr={:?} sprites={} drawn={} priority_skips={} transparent_skips={}",
                    line.ly,
                    line.mode,
                    line.lyc_match,
                    line.window_line_counter,
                    line.bg_tile_map_base,
                    line.window_tile_map_base,
                    line.tile_data_addressing,
                    line.sprite_stats.sprites_on_line,
                    line.sprite_stats.opaque_pixels_drawn,
                    line.sprite_stats.priority_skips,
                    line.sprite_stats.transparent_skips,
                );
            }
        }

        if let Some(upload) = self.app.video_mut().take_upload_diagnostics() {
            eprintln!(
                "video upload: src={}x{} src_stride={} target={}x{} target_stride={} target_len={} expected_len={} viewport=({},{} {}x{}) rendered_pixels={} palette_entries={} rgba_bpp={} len_match={}",
                upload.source_width,
                upload.source_height,
                upload.source_stride_bytes,
                upload.target_width,
                upload.target_height,
                upload.target_stride_bytes,
                upload.target_len,
                upload.expected_target_len,
                upload.viewport_x,
                upload.viewport_y,
                upload.viewport_width,
                upload.viewport_height,
                upload.rendered_pixels,
                upload.palette_entries,
                upload.rgba_bytes_per_pixel,
                upload.target_len_matches,
            );
        }
    }

    fn maybe_report_audio_diagnostics(&mut self, now: Instant) {
        if !self.options.audio_diagnostics {
            return;
        }

        let interval = Duration::from_secs(1);
        if now.saturating_duration_since(self.last_audio_diag_report) < interval {
            return;
        }

        let diagnostics = self.app.audio_diagnostics();
        let queue = diagnostics.queue;
        let elapsed_ms = now
            .saturating_duration_since(self.last_audio_diag_report)
            .as_millis()
            .max(1);

        let current = AudioDiagCheckpoint {
            enqueued_samples: queue.enqueued_samples,
            dequeued_samples: queue.dequeued_samples,
            callback_requested_samples: queue.callback_requested_samples,
            dropped_samples: queue.dropped_samples,
            underflow_events: queue.underflow_events,
            callback_calls: queue.callback_calls,
        };

        let (
            enqueued_rate,
            dequeued_rate,
            callback_rate,
            dropped_delta,
            underflow_delta,
            callback_delta,
        ) = if let Some(previous) = self.last_audio_diag_checkpoint {
            (
                samples_per_second(
                    current
                        .enqueued_samples
                        .wrapping_sub(previous.enqueued_samples),
                    elapsed_ms,
                ),
                samples_per_second(
                    current
                        .dequeued_samples
                        .wrapping_sub(previous.dequeued_samples),
                    elapsed_ms,
                ),
                samples_per_second(
                    current
                        .callback_requested_samples
                        .wrapping_sub(previous.callback_requested_samples),
                    elapsed_ms,
                ),
                current
                    .dropped_samples
                    .wrapping_sub(previous.dropped_samples),
                current
                    .underflow_events
                    .wrapping_sub(previous.underflow_events),
                current.callback_calls.wrapping_sub(previous.callback_calls),
            )
        } else {
            (0, 0, 0, 0, 0, 0)
        };

        let core = diagnostics.core;
        let dc_offset = if core.total_samples == 0 {
            0
        } else {
            let total_samples = match i64::try_from(core.total_samples) {
                Ok(value) if value != 0 => value,
                Ok(_) | Err(_) => 1,
            };
            core.sample_sum / total_samples
        };
        let clipping_ppm = if core.total_samples == 0 {
            0
        } else {
            parts_per_million(core.clipped_samples, core.total_samples)
        };

        eprintln!(
            "audio diag: core(samples={},dc_offset={},clip_ppm={},amp_buckets={:?}) queue(queued={},dropped={}:+{},underflows={}:+{},callbacks={}:+{},enq_sps={},deq_sps={},req_sps={})",
            core.total_samples,
            dc_offset,
            clipping_ppm,
            core.amplitude_buckets,
            queue.queued_samples,
            queue.dropped_samples,
            dropped_delta,
            queue.underflow_events,
            underflow_delta,
            queue.callback_calls,
            callback_delta,
            enqueued_rate,
            dequeued_rate,
            callback_rate,
        );

        self.last_audio_diag_report = now;
        self.last_audio_diag_checkpoint = Some(current);
    }
}

fn parts_per_million(numerator: u64, denominator: u64) -> u64 {
    if denominator == 0 {
        return 0;
    }
    let scaled = (u128::from(numerator) * 1_000_000u128) / u128::from(denominator);
    match u64::try_from(scaled) {
        Ok(value) => value,
        Err(_) => u64::MAX,
    }
}

fn samples_per_second(samples: u64, elapsed_ms: u128) -> u64 {
    if elapsed_ms == 0 {
        return 0;
    }
    let scaled = (u128::from(samples) * 1_000u128) / elapsed_ms;
    match u64::try_from(scaled) {
        Ok(value) => value,
        Err(_) => u64::MAX,
    }
}

impl ApplicationHandler for DesktopRuntime {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if let Err(err) = self.initialize_window(event_loop) {
            self.set_error(event_loop, err);
        }
    }

    fn new_events(&mut self, event_loop: &ActiveEventLoop, _cause: winit::event::StartCause) {
        event_loop.set_control_flow(ControlFlow::Poll);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if self.window_id != Some(window_id) {
            return;
        }

        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                self.handle_resize(event_loop, size);
            }
            WindowEvent::KeyboardInput { event, .. } => {
                self.handle_keyboard(event_loop, event.state, event.repeat, event.physical_key);
            }
            WindowEvent::RedrawRequested => {
                self.step_and_render(event_loop);
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

#[must_use]
fn map_key_code(code: KeyCode) -> Option<HostKey> {
    match code {
        KeyCode::ArrowRight => Some(HostKey::Right),
        KeyCode::ArrowLeft => Some(HostKey::Left),
        KeyCode::ArrowUp => Some(HostKey::Up),
        KeyCode::ArrowDown => Some(HostKey::Down),
        KeyCode::KeyZ => Some(HostKey::Z),
        KeyCode::KeyX => Some(HostKey::X),
        KeyCode::Enter => Some(HostKey::Enter),
        KeyCode::Backspace => Some(HostKey::Backspace),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{map_key_code, snapshot_path_for_rom};
    use crate::input::HostKey;
    use std::path::Path;
    use winit::keyboard::KeyCode;

    #[test]
    fn snapshot_path_uses_rom_stem() {
        let path = snapshot_path_for_rom(Path::new("/tmp/pokemon.gb"));
        assert_eq!(path, Path::new("/tmp/pokemon.gbsnap"));
    }

    #[test]
    fn keycode_mapping_covers_required_gameplay_buttons() {
        assert_eq!(map_key_code(KeyCode::ArrowUp), Some(HostKey::Up));
        assert_eq!(map_key_code(KeyCode::KeyZ), Some(HostKey::Z));
        assert_eq!(map_key_code(KeyCode::Backspace), Some(HostKey::Backspace));
        assert_eq!(map_key_code(KeyCode::F5), None);
    }
}
