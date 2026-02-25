use gb_core::apu::{ApuChannelDiagnostics, ApuDiagnostics};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ChannelDiagnosticsReport {
    pub sample_count: u64,
    pub sample_sum: i64,
    pub sample_abs_sum: u64,
    pub peak_abs_sample: u16,
    pub non_zero_samples: u64,
}

impl From<ApuChannelDiagnostics> for ChannelDiagnosticsReport {
    fn from(value: ApuChannelDiagnostics) -> Self {
        Self {
            sample_count: value.sample_count,
            sample_sum: value.sample_sum,
            sample_abs_sum: value.sample_abs_sum,
            peak_abs_sample: value.peak_abs_sample,
            non_zero_samples: value.non_zero_samples,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AudioDiagnosticsReport {
    pub total_samples: u64,
    pub clipped_samples: u64,
    pub clipping_ppm: u64,
    pub sample_sum: i64,
    pub sample_abs_sum: u64,
    pub min_sample: i16,
    pub max_sample: i16,
    pub peak_abs_sample: u16,
    pub frame_sequencer_steps: u64,
    pub sample_hash: u64,
    pub channel_stats: Option<[ChannelDiagnosticsReport; 4]>,
}

impl From<ApuDiagnostics> for AudioDiagnosticsReport {
    fn from(value: ApuDiagnostics) -> Self {
        Self {
            total_samples: value.total_samples,
            clipped_samples: value.clipped_samples,
            clipping_ppm: value.clipping_ppm(),
            sample_sum: value.sample_sum,
            sample_abs_sum: value.sample_abs_sum,
            min_sample: value.min_sample,
            max_sample: value.max_sample,
            peak_abs_sample: value.peak_abs_sample,
            frame_sequencer_steps: value.frame_sequencer_steps,
            sample_hash: value.sample_hash,
            channel_stats: value
                .channel_stats
                .map(|channels| channels.map(ChannelDiagnosticsReport::from)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RunReport {
    pub cycles_executed: u64,
    pub frames_produced: u64,
    pub frame_hash: Option<u64>,
    pub audio_hash: Option<u64>,
    pub audio_diagnostics: Option<AudioDiagnosticsReport>,
}

impl RunReport {
    #[must_use]
    pub fn to_json(&self) -> String {
        match serde_json::to_string(self) {
            Ok(json) => json,
            Err(_) => String::from("{\"serialization_error\":\"failed\"}"),
        }
    }
}

#[must_use]
pub fn hash_bytes(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0001_0000_01B3);
    }
    hash
}

#[must_use]
pub fn hash_samples(samples: &[i16]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for sample in samples {
        for byte in sample.to_le_bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0000_0001_0000_01B3);
        }
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::{RunReport, hash_bytes};

    #[test]
    fn report_json_is_machine_readable() {
        let report = RunReport {
            cycles_executed: 123,
            frames_produced: 4,
            frame_hash: Some(10),
            audio_hash: None,
            audio_diagnostics: None,
        };

        assert_eq!(
            report.to_json(),
            "{\"cycles_executed\":123,\"frames_produced\":4,\"frame_hash\":10,\"audio_hash\":null,\"audio_diagnostics\":null}"
        );
    }

    #[test]
    fn hash_function_is_deterministic() {
        let payload = [1, 2, 3, 4];
        assert_eq!(hash_bytes(&payload), hash_bytes(&payload));
    }
}
