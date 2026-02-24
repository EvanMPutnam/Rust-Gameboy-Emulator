use crate::error::BootError;
use serde::{Deserialize, Serialize};
use serde_big_array::BigArray;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BootRom {
    enabled: bool,
    #[serde(with = "BigArray")]
    data: [u8; 256],
}

impl BootRom {
    #[must_use]
    pub fn from_array(data: [u8; 256]) -> Self {
        Self {
            enabled: true,
            data,
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, BootError> {
        if bytes.len() != 256 {
            return Err(BootError::InvalidLength(bytes.len()));
        }

        let mut data = [0; 256];
        data.copy_from_slice(bytes);
        Ok(Self::from_array(data))
    }

    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    #[must_use]
    pub fn read(&self, addr: u16) -> Option<u8> {
        if self.enabled && addr <= 0x00FF {
            Some(self.data[usize::from(addr)])
        } else {
            None
        }
    }
}
