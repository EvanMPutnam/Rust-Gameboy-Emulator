#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunReport {
    pub cycles_executed: u64,
    pub frames_produced: u64,
    pub frame_hash: Option<u64>,
    pub audio_hash: Option<u64>,
}

impl RunReport {
    #[must_use]
    pub fn to_json(&self) -> String {
        format!(
            "{{\"cycles_executed\":{},\"frames_produced\":{},\"frame_hash\":{},\"audio_hash\":{}}}",
            self.cycles_executed,
            self.frames_produced,
            optional_u64_json(self.frame_hash),
            optional_u64_json(self.audio_hash),
        )
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

fn optional_u64_json(value: Option<u64>) -> String {
    value.map_or_else(|| String::from("null"), |v| v.to_string())
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
        };

        assert_eq!(
            report.to_json(),
            "{\"cycles_executed\":123,\"frames_produced\":4,\"frame_hash\":10,\"audio_hash\":null}"
        );
    }

    #[test]
    fn hash_function_is_deterministic() {
        let payload = [1, 2, 3, 4];
        assert_eq!(hash_bytes(&payload), hash_bytes(&payload));
    }
}
