use gb_core::ppu::{SCREEN_HEIGHT, SCREEN_WIDTH};

const PALETTE: [[u8; 3]; 4] = [[224, 248, 208], [136, 192, 112], [52, 104, 86], [8, 24, 32]];
const SCREEN_WIDTH_U32: u32 = 160;
const SCREEN_HEIGHT_U32: u32 = 144;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PresentSize {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Viewport {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VideoUploadDiagnostics {
    pub source_width: u32,
    pub source_height: u32,
    pub source_stride_bytes: usize,
    pub target_width: u32,
    pub target_height: u32,
    pub target_stride_bytes: usize,
    pub target_len: usize,
    pub expected_target_len: usize,
    pub viewport_x: u32,
    pub viewport_y: u32,
    pub viewport_width: u32,
    pub viewport_height: u32,
    pub rendered_pixels: u32,
    pub palette_entries: u8,
    pub rgba_bytes_per_pixel: u8,
    pub target_len_matches: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct UploadRecordInput {
    surface_width: u32,
    surface_height: u32,
    target_len: usize,
    expected_target_len: usize,
    viewport: Viewport,
    rendered_pixels: u32,
    target_len_matches: bool,
}

#[derive(Debug, Clone)]
pub struct VideoPresenter {
    last_frame: Option<[u8; SCREEN_WIDTH * SCREEN_HEIGHT]>,
    present_size: PresentSize,
    diagnostics_enabled: bool,
    last_upload_diagnostics: Option<VideoUploadDiagnostics>,
}

impl Default for VideoPresenter {
    fn default() -> Self {
        Self {
            last_frame: None,
            present_size: PresentSize {
                width: SCREEN_WIDTH_U32,
                height: SCREEN_HEIGHT_U32,
            },
            diagnostics_enabled: false,
            last_upload_diagnostics: None,
        }
    }
}

impl VideoPresenter {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[allow(
        clippy::large_types_passed_by_value,
        reason = "Frames are transferred from core to frontend once per produced frame."
    )]
    pub fn present(&mut self, frame: [u8; SCREEN_WIDTH * SCREEN_HEIGHT]) {
        self.last_frame = Some(frame);
    }

    #[cfg(test)]
    #[must_use]
    pub fn present_size(&self) -> PresentSize {
        self.present_size
    }

    pub fn set_diagnostics_enabled(&mut self, enabled: bool) {
        self.diagnostics_enabled = enabled;
        if !enabled {
            self.last_upload_diagnostics = None;
        }
    }

    #[must_use]
    pub fn take_upload_diagnostics(&mut self) -> Option<VideoUploadDiagnostics> {
        self.last_upload_diagnostics.take()
    }

    pub fn resize(&mut self, window_width: u32, window_height: u32) {
        self.present_size = compute_present_size(window_width, window_height);
    }

    pub fn render_rgba(&mut self, target: &mut [u8], surface_width: u32, surface_height: u32) {
        let Some(surface_width_usize) = usize::try_from(surface_width).ok() else {
            return;
        };
        let Some(surface_height_usize) = usize::try_from(surface_height).ok() else {
            return;
        };
        let expected_len = surface_width_usize
            .checked_mul(surface_height_usize)
            .and_then(|pixels| pixels.checked_mul(4));
        let expected_target_len = expected_len.unwrap_or(0);
        if expected_len != Some(target.len()) {
            self.record_upload_diagnostics(UploadRecordInput {
                surface_width,
                surface_height,
                target_len: target.len(),
                expected_target_len,
                viewport: Viewport {
                    x: 0,
                    y: 0,
                    width: 0,
                    height: 0,
                },
                rendered_pixels: 0,
                target_len_matches: false,
            });
            return;
        }

        let Some(frame) = self.last_frame.as_ref() else {
            fill_black(target);
            return;
        };

        let viewport = build_viewport(surface_width, surface_height, self.present_size);
        if viewport.width == 0 || viewport.height == 0 {
            fill_black(target);
            self.record_upload_diagnostics(UploadRecordInput {
                surface_width,
                surface_height,
                target_len: target.len(),
                expected_target_len,
                viewport,
                rendered_pixels: 0,
                target_len_matches: true,
            });
            return;
        }

        let Some(viewport_width) = usize::try_from(viewport.width).ok() else {
            return;
        };
        let Some(viewport_height) = usize::try_from(viewport.height).ok() else {
            return;
        };
        let Some(viewport_x) = usize::try_from(viewport.x).ok() else {
            return;
        };
        let Some(viewport_y) = usize::try_from(viewport.y).ok() else {
            return;
        };

        let viewport_covers_surface = viewport.width == surface_width
            && viewport.height == surface_height
            && viewport.x == 0
            && viewport.y == 0;
        if !viewport_covers_surface {
            fill_black(target);
        }

        let row_stride = surface_width_usize.saturating_mul(4);
        for y in 0..viewport_height {
            let src_y = y * SCREEN_HEIGHT / viewport_height;
            let src_row = src_y * SCREEN_WIDTH;
            let dst_row = (viewport_y + y)
                .saturating_mul(row_stride)
                .saturating_add(viewport_x.saturating_mul(4));
            for x in 0..viewport_width {
                let src_x = x * SCREEN_WIDTH / viewport_width;
                let shade = frame[src_row + src_x];
                let [r, g, b] = shade_to_rgb(shade);

                let base = dst_row + x * 4;
                target[base] = r;
                target[base + 1] = g;
                target[base + 2] = b;
                target[base + 3] = u8::MAX;
            }
        }

        let rendered_pixels = viewport.width.saturating_mul(viewport.height);
        self.record_upload_diagnostics(UploadRecordInput {
            surface_width,
            surface_height,
            target_len: target.len(),
            expected_target_len,
            viewport,
            rendered_pixels,
            target_len_matches: true,
        });
    }

    fn record_upload_diagnostics(&mut self, input: UploadRecordInput) {
        if !self.diagnostics_enabled {
            return;
        }

        self.last_upload_diagnostics = Some(VideoUploadDiagnostics {
            source_width: SCREEN_WIDTH_U32,
            source_height: SCREEN_HEIGHT_U32,
            source_stride_bytes: SCREEN_WIDTH,
            target_width: input.surface_width,
            target_height: input.surface_height,
            target_stride_bytes: usize::try_from(input.surface_width)
                .ok()
                .and_then(|width| width.checked_mul(4))
                .unwrap_or(0),
            target_len: input.target_len,
            expected_target_len: input.expected_target_len,
            viewport_x: input.viewport.x,
            viewport_y: input.viewport.y,
            viewport_width: input.viewport.width,
            viewport_height: input.viewport.height,
            rendered_pixels: input.rendered_pixels,
            palette_entries: 4,
            rgba_bytes_per_pixel: 4,
            target_len_matches: input.target_len_matches,
        });
    }
}

fn fill_black(target: &mut [u8]) {
    for chunk in target.chunks_exact_mut(4) {
        chunk[0] = 0;
        chunk[1] = 0;
        chunk[2] = 0;
        chunk[3] = u8::MAX;
    }
}

#[must_use]
fn shade_to_rgb(shade: u8) -> [u8; 3] {
    PALETTE
        .get(usize::from(shade))
        .copied()
        .unwrap_or(PALETTE[0])
}

#[must_use]
fn compute_present_size(window_width: u32, window_height: u32) -> PresentSize {
    let width = window_width.max(1);
    let height = window_height.max(1);

    if width >= SCREEN_WIDTH_U32 && height >= SCREEN_HEIGHT_U32 {
        let scale = (width / SCREEN_WIDTH_U32).min(height / SCREEN_HEIGHT_U32);
        if scale > 0 {
            return PresentSize {
                width: SCREEN_WIDTH_U32.saturating_mul(scale),
                height: SCREEN_HEIGHT_U32.saturating_mul(scale),
            };
        }
    }

    let fit_to_width = width.saturating_mul(9) <= height.saturating_mul(10);
    if fit_to_width {
        let adjusted_width = width - (width % 10);
        let fallback_width = adjusted_width.max(1);
        return PresentSize {
            width: fallback_width,
            height: (fallback_width.saturating_mul(9) / 10).max(1),
        };
    }

    let adjusted_height = height - (height % 9);
    let fallback_height = adjusted_height.max(1);
    PresentSize {
        width: (fallback_height.saturating_mul(10) / 9).max(1),
        height: fallback_height,
    }
}

#[must_use]
fn build_viewport(surface_width: u32, surface_height: u32, present_size: PresentSize) -> Viewport {
    let width = present_size.width.min(surface_width);
    let height = present_size.height.min(surface_height);
    let x = surface_width.saturating_sub(width) / 2;
    let y = surface_height.saturating_sub(height) / 2;

    Viewport {
        x,
        y,
        width,
        height,
    }
}

#[cfg(test)]
mod tests {
    use super::{VideoPresenter, compute_present_size};

    #[test]
    fn resize_preserves_integer_scale_when_window_is_large_enough() {
        let mut video = VideoPresenter::new();
        video.resize(1920, 1080);
        let size = video.present_size();

        assert_eq!(size.width % 160, 0);
        assert_eq!(size.height % 144, 0);
        assert_eq!(size.width * 9, size.height * 10);
    }

    #[test]
    fn resize_falls_back_to_aspect_preserving_size_for_tiny_windows() {
        let size = compute_present_size(100, 60);
        assert!(size.width <= 100);
        assert!(size.height <= 60);
        assert_eq!(size.width * 9, size.height * 10);
    }

    #[test]
    #[allow(
        clippy::large_stack_arrays,
        reason = "DMG frame tests intentionally construct a full 160x144 source frame."
    )]
    fn render_maps_dmg_shades_to_rgba_pixels() {
        let mut video = VideoPresenter::new();
        let mut frame = [0u8; 160 * 144];
        frame[0] = 3;
        frame[1] = 2;
        frame[2] = 1;
        frame[3] = 0;
        video.present(frame);
        video.resize(160, 144);

        let mut buffer = vec![0u8; 160 * 144 * 4];
        video.render_rgba(&mut buffer, 160, 144);

        assert_eq!(&buffer[0..4], &[8, 24, 32, 255]);
        assert_eq!(&buffer[4..8], &[52, 104, 86, 255]);
        assert_eq!(&buffer[8..12], &[136, 192, 112, 255]);
        assert_eq!(&buffer[12..16], &[224, 248, 208, 255]);
    }

    #[test]
    #[allow(
        clippy::large_stack_arrays,
        reason = "DMG frame tests intentionally construct a full 160x144 source frame."
    )]
    fn render_emits_upload_diagnostics_with_stride_and_viewport() {
        let mut video = VideoPresenter::new();
        video.set_diagnostics_enabled(true);
        video.present([0u8; 160 * 144]);
        video.resize(320, 288);

        let mut buffer = vec![0u8; 320 * 288 * 4];
        video.render_rgba(&mut buffer, 320, 288);
        let diagnostics = video
            .take_upload_diagnostics()
            .expect("upload diagnostics should be captured");

        assert_eq!(diagnostics.source_width, 160);
        assert_eq!(diagnostics.source_height, 144);
        assert_eq!(diagnostics.target_width, 320);
        assert_eq!(diagnostics.target_height, 288);
        assert_eq!(diagnostics.target_stride_bytes, 1_280);
        assert_eq!(diagnostics.viewport_width, 320);
        assert_eq!(diagnostics.viewport_height, 288);
        assert!(diagnostics.target_len_matches);
        assert_eq!(diagnostics.rendered_pixels, 320 * 288);
    }
}
