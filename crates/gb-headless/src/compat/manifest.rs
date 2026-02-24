use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::compat::CompatError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExpectedOutcome {
    Pass,
    Fail,
    Timeout,
    UnsupportedFeature,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestRom {
    pub id: String,
    pub path: PathBuf,
    pub max_cycles: u32,
    pub expected: ExpectedOutcome,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompatManifest {
    pub rom: Vec<ManifestRom>,
}

pub fn load_manifest(path: &Path) -> Result<CompatManifest, CompatError> {
    let contents = fs::read_to_string(path)?;
    let mut manifest: CompatManifest = toml::from_str(&contents)?;

    let base_dir = path.parent().unwrap_or_else(|| Path::new("."));
    for entry in &mut manifest.rom {
        if entry.path.is_relative() {
            entry.path = base_dir.join(&entry.path);
        }
    }

    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::load_manifest;

    fn temp_path(prefix: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("{prefix}_{nanos}.toml"))
    }

    #[test]
    fn parses_manifest_and_resolves_relative_paths() {
        let path = temp_path("compat_manifest");
        let rom_file = path.with_extension("gb");
        std::fs::write(&rom_file, [0u8; 0]).expect("write rom placeholder");

        let manifest = format!(
            "[[rom]]\nid = \"smoke\"\npath = \"{}\"\nmax_cycles = 100\nexpected = \"pass\"\ntags = [\"cpu\"]\n",
            rom_file.file_name().expect("filename").to_string_lossy()
        );
        std::fs::write(&path, manifest).expect("write manifest");

        let parsed = load_manifest(&path).expect("parse manifest");
        assert_eq!(parsed.rom.len(), 1);
        assert!(parsed.rom[0].path.is_absolute());

        std::fs::remove_file(path).expect("remove manifest");
        std::fs::remove_file(rom_file).expect("remove rom");
    }
}
