use serde::Serialize;

use crate::compat::{CompatError, CompatTier};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CompatibilityOutcome {
    Pass,
    Fail,
    Timeout,
    UnsupportedFeature,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureCategory {
    Cpu,
    Timing,
    Ppu,
    Apu,
    MemoryMapper,
    Io,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CompatRomResult {
    pub id: String,
    pub path: String,
    pub tags: Vec<String>,
    pub expected: CompatibilityOutcome,
    pub outcome: CompatibilityOutcome,
    pub cycles_executed: u64,
    pub frame_hash: Option<u64>,
    pub audio_hash: Option<u64>,
    pub failure_category: Option<FailureCategory>,
    pub diagnostic: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CompatRunReport {
    pub tier: CompatTier,
    pub total: usize,
    pub pass: usize,
    pub fail: usize,
    pub timeout: usize,
    pub unsupported_feature: usize,
    pub results: Vec<CompatRomResult>,
}

impl CompatRunReport {
    pub fn to_json(&self) -> Result<String, CompatError> {
        Ok(serde_json::to_string(self)?)
    }
}

#[cfg(test)]
mod tests {
    use super::{CompatRomResult, CompatRunReport, CompatibilityOutcome};
    use crate::compat::CompatTier;

    #[test]
    fn report_serializes_to_json() {
        let report = CompatRunReport {
            tier: CompatTier::Smoke,
            total: 1,
            pass: 1,
            fail: 0,
            timeout: 0,
            unsupported_feature: 0,
            results: vec![CompatRomResult {
                id: String::from("test"),
                path: String::from("test.gb"),
                tags: vec![String::from("cpu")],
                expected: CompatibilityOutcome::Pass,
                outcome: CompatibilityOutcome::Pass,
                cycles_executed: 1024,
                frame_hash: None,
                audio_hash: None,
                failure_category: None,
                diagnostic: None,
            }],
        };

        let json = report.to_json().expect("to json");
        assert!(json.contains("\"tier\":\"smoke\""));
    }
}
