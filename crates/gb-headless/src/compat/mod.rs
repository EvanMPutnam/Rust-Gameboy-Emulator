pub mod manifest;
pub mod report;
pub mod runner;

use std::error::Error;
use std::fmt::{Display, Formatter};
use std::path::PathBuf;

use serde::Serialize;

use crate::runner::RunnerError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CompatTier {
    Smoke,
    Core,
    Full,
}

impl CompatTier {
    pub fn from_str(value: &str) -> Result<Self, CompatError> {
        match value {
            "smoke" => Ok(Self::Smoke),
            "core" => Ok(Self::Core),
            "full" => Ok(Self::Full),
            other => Err(CompatError::InvalidTier(other.to_string())),
        }
    }

    #[must_use]
    pub fn manifest_path(self) -> PathBuf {
        let base =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata/compat/manifests");
        match self {
            Self::Smoke => base.join("smoke.toml"),
            Self::Core => base.join("core.toml"),
            Self::Full => base.join("full.toml"),
        }
    }
}

#[derive(Debug)]
pub enum CompatError {
    Io(std::io::Error),
    ManifestParse(toml::de::Error),
    Json(serde_json::Error),
    InvalidTier(String),
    Runner(RunnerError),
}

impl Display for CompatError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "io error: {err}"),
            Self::ManifestParse(err) => write!(f, "manifest parse error: {err}"),
            Self::Json(err) => write!(f, "json serialization error: {err}"),
            Self::InvalidTier(tier) => write!(f, "invalid compatibility tier '{tier}'"),
            Self::Runner(err) => write!(f, "runner error: {err}"),
        }
    }
}

impl Error for CompatError {}

impl From<std::io::Error> for CompatError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<toml::de::Error> for CompatError {
    fn from(value: toml::de::Error) -> Self {
        Self::ManifestParse(value)
    }
}

impl From<serde_json::Error> for CompatError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

impl From<RunnerError> for CompatError {
    fn from(value: RunnerError) -> Self {
        Self::Runner(value)
    }
}
