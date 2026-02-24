use crate::error::CartridgeError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CartridgeHeader {
    pub title: String,
    pub cartridge_type: u8,
    pub rom_size_code: u8,
    pub ram_size_code: u8,
    pub cgb_flag: u8,
}

impl CartridgeHeader {
    pub fn parse(rom: &[u8]) -> Result<Self, CartridgeError> {
        if rom.len() < 0x150 {
            return Err(CartridgeError::RomTooSmall);
        }

        let raw_title = &rom[0x0134..=0x0143];
        let title_end = raw_title
            .iter()
            .position(|value| *value == 0)
            .unwrap_or(raw_title.len());
        let title = String::from_utf8_lossy(&raw_title[..title_end])
            .trim()
            .to_string();

        Ok(Self {
            title,
            cartridge_type: rom[0x0147],
            rom_size_code: rom[0x0148],
            ram_size_code: rom[0x0149],
            cgb_flag: rom[0x0143],
        })
    }
}
