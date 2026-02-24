use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct GbConfig {
    pub force_post_boot_state: bool,
    pub use_boot_rom: bool,
    #[serde(skip)]
    pub boot_rom: Option<[u8; 256]>,
}

impl Default for GbConfig {
    fn default() -> Self {
        Self {
            force_post_boot_state: true,
            use_boot_rom: false,
            boot_rom: None,
        }
    }
}
