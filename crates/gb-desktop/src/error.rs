use std::path::{Path, PathBuf};

use gb_core::GbError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DesktopError {
    #[error("argument error: {0}")]
    Argument(String),
    #[error("io error while {context} '{path}': {source}")]
    Io {
        context: &'static str,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("boot rom must contain exactly 256 bytes, got {0}")]
    InvalidBootRomLength(usize),
    #[error("core error: {0}")]
    Core(#[from] GbError),
    #[error("window event loop error: {0}")]
    WindowEventLoop(#[source] winit::error::EventLoopError),
    #[error("window creation failed: {0}")]
    WindowCreate(#[source] winit::error::OsError),
    #[error("render backend initialization failed: {0}")]
    PixelsCreate(#[source] pixels::Error),
    #[error("render backend resize failed: {0}")]
    PixelsResize(#[source] pixels::TextureError),
    #[error("render backend presentation failed: {0}")]
    PixelsRender(#[source] pixels::Error),
    #[error("audio output device is unavailable")]
    AudioNoDevice,
    #[error("audio default output configuration failed: {0}")]
    AudioDefaultConfig(#[source] cpal::DefaultStreamConfigError),
    #[error("audio stream build failed: {0}")]
    AudioStreamBuild(#[source] cpal::BuildStreamError),
    #[error("audio stream start failed: {0}")]
    AudioStreamPlay(#[source] cpal::PlayStreamError),
    #[error("audio sample format is unsupported: {0}")]
    AudioUnsupportedSampleFormat(String),
}

impl DesktopError {
    #[must_use]
    pub fn io(context: &'static str, path: &Path, source: std::io::Error) -> Self {
        Self::Io {
            context,
            path: path.to_path_buf(),
            source,
        }
    }
}
