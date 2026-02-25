use crate::compat::manifest::{CompatManifest, ExpectedOutcome, ManifestRom, load_manifest};
use crate::compat::report::{
    CompatRomResult, CompatRunReport, CompatibilityOutcome, FailureCategory,
};
use crate::compat::{CompatError, CompatTier};
use crate::runner::{RunBudget, RunnerConfig, RunnerError, run};
use gb_core::GbError;

pub fn run_tier(
    tier: CompatTier,
    manifest_path: Option<std::path::PathBuf>,
) -> Result<CompatRunReport, CompatError> {
    let path = manifest_path.unwrap_or_else(|| tier.manifest_path());
    let manifest = load_manifest(&path)?;
    Ok(run_manifest(tier, &manifest))
}

pub fn run_manifest(tier: CompatTier, manifest: &CompatManifest) -> CompatRunReport {
    let mut results = Vec::with_capacity(manifest.rom.len());

    for entry in &manifest.rom {
        results.push(run_entry(entry));
    }

    let mut pass = 0usize;
    let mut fail = 0usize;
    let mut timeout = 0usize;
    let mut unsupported_feature = 0usize;

    for result in &results {
        match result.outcome {
            CompatibilityOutcome::Pass => pass += 1,
            CompatibilityOutcome::Fail => fail += 1,
            CompatibilityOutcome::Timeout => timeout += 1,
            CompatibilityOutcome::UnsupportedFeature => unsupported_feature += 1,
        }
    }

    CompatRunReport {
        tier,
        total: results.len(),
        pass,
        fail,
        timeout,
        unsupported_feature,
        results,
    }
}

fn run_entry(entry: &ManifestRom) -> CompatRomResult {
    let expected = expected_to_outcome(entry.expected);
    let config = RunnerConfig {
        rom_path: entry.path.clone(),
        boot_rom_path: None,
        budget: RunBudget::Cycles(entry.max_cycles),
        input_script_path: None,
        save_in_path: None,
        save_out_path: None,
        audio_diagnostics: false,
        audio_channel_diagnostics: false,
    };

    let (outcome, cycles_executed, frame_hash, audio_hash, failure_category, diagnostic) =
        match run(&config) {
            Ok(report) => (
                CompatibilityOutcome::Pass,
                report.cycles_executed,
                report.frame_hash,
                report.audio_hash,
                None,
                None,
            ),
            Err(err) => {
                let (mapped_outcome, category) = map_error_outcome(&err);
                (
                    mapped_outcome,
                    0,
                    None,
                    None,
                    category,
                    Some(actionable_diagnostic(&err)),
                )
            }
        };

    let diagnostic = if expected == outcome {
        diagnostic
    } else {
        Some(match diagnostic {
            Some(message) => format!("{message}; expected {expected:?}, got {outcome:?}"),
            None => format!("expected {expected:?}, got {outcome:?}"),
        })
    };

    CompatRomResult {
        id: entry.id.clone(),
        path: entry.path.display().to_string(),
        tags: entry.tags.clone(),
        expected,
        outcome,
        cycles_executed,
        frame_hash,
        audio_hash,
        failure_category,
        diagnostic,
    }
}

fn expected_to_outcome(value: ExpectedOutcome) -> CompatibilityOutcome {
    match value {
        ExpectedOutcome::Pass => CompatibilityOutcome::Pass,
        ExpectedOutcome::Fail => CompatibilityOutcome::Fail,
        ExpectedOutcome::Timeout => CompatibilityOutcome::Timeout,
        ExpectedOutcome::UnsupportedFeature => CompatibilityOutcome::UnsupportedFeature,
    }
}

fn map_error_outcome(error: &RunnerError) -> (CompatibilityOutcome, Option<FailureCategory>) {
    match error {
        RunnerError::FrameBudgetTimeout => {
            (CompatibilityOutcome::Timeout, Some(FailureCategory::Timing))
        }
        RunnerError::Core(core) => (
            CompatibilityOutcome::Fail,
            Some(match core {
                GbError::Cpu(_) => FailureCategory::Cpu,
                GbError::Timer(_) | GbError::Clock(_) | GbError::Dma(_) => FailureCategory::Timing,
                GbError::Ppu(_) => FailureCategory::Ppu,
                GbError::Apu(_) => FailureCategory::Apu,
                GbError::Cartridge(_) => FailureCategory::MemoryMapper,
                GbError::Bus(_) | GbError::Joypad(_) | GbError::Serial(_) => FailureCategory::Io,
                GbError::Boot(_) | GbError::State(_) => FailureCategory::Unknown,
            }),
        ),
        RunnerError::Io(_) => (CompatibilityOutcome::Fail, Some(FailureCategory::Unknown)),
        RunnerError::InvalidBootRomLength(_) | RunnerError::InputScriptParse(_) => (
            CompatibilityOutcome::UnsupportedFeature,
            Some(FailureCategory::Io),
        ),
    }
}

fn actionable_diagnostic(error: &RunnerError) -> String {
    match error {
        RunnerError::Io(io_error) => format!("verify file paths and permissions: {io_error}"),
        RunnerError::Core(core) => {
            format!("core execution failed; inspect subsystem-specific tests: {core}")
        }
        RunnerError::InvalidBootRomLength(len) => {
            format!("boot ROM length must be 256 bytes, received {len}")
        }
        RunnerError::InputScriptParse(message) => {
            format!("input script is invalid; fix parse issue: {message}")
        }
        RunnerError::FrameBudgetTimeout => {
            String::from("ROM did not complete within configured cycle/frame budget")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{run_manifest, run_tier};
    use crate::compat::CompatTier;
    use crate::compat::manifest::{CompatManifest, ExpectedOutcome, ManifestRom};

    fn minimal_rom_file() -> std::path::PathBuf {
        let mut rom = vec![0u8; 0x8000];
        rom[0x0147] = 0x00;
        rom[0x0148] = 0x00;
        rom[0x0149] = 0x00;
        rom[0x0100] = 0x00;

        let path = std::env::temp_dir().join("compat_runner_minimal.gb");
        std::fs::write(&path, rom).expect("write rom");
        path
    }

    #[test]
    fn runner_produces_results_for_manifest_entries() {
        let rom_path = minimal_rom_file();
        let manifest = CompatManifest {
            rom: vec![ManifestRom {
                id: String::from("minimal"),
                path: rom_path.clone(),
                max_cycles: 512,
                expected: ExpectedOutcome::Pass,
                tags: vec![String::from("smoke")],
            }],
        };

        let report = run_manifest(CompatTier::Smoke, &manifest);
        assert_eq!(report.total, 1);
        assert_eq!(report.results.len(), 1);

        std::fs::remove_file(rom_path).expect("remove rom");
    }

    #[test]
    fn tier_runner_uses_default_manifest_path() {
        let report = run_tier(CompatTier::Smoke, None).expect("run smoke tier");
        assert!(report.total >= 1);
    }
}
