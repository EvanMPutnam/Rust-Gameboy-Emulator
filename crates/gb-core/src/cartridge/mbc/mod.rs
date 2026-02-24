use crate::cartridge::rtc::RtcState;
use crate::error::CartridgeError;

pub mod mbc1;
pub mod mbc2;
pub mod mbc3;
pub mod mbc5;
pub mod rom_only;

pub trait Mapper {
    fn read_rom(&self, rom: &[u8], addr: u16) -> Result<u8, CartridgeError>;
    fn write_rom(&mut self, addr: u16, value: u8) -> Result<(), CartridgeError>;
    fn read_ram(&self, ram: &[u8], addr: u16) -> Result<u8, CartridgeError>;
    fn write_ram(&mut self, ram: &mut [u8], addr: u16, value: u8) -> Result<(), CartridgeError>;

    fn save_ram(&self, ram: &[u8]) -> Option<Vec<u8>> {
        if ram.is_empty() {
            None
        } else {
            Some(ram.to_vec())
        }
    }

    fn load_ram(&mut self, ram: &mut [u8], data: &[u8]) -> Result<(), CartridgeError> {
        if ram.len() != data.len() {
            return Err(CartridgeError::SaveRamSizeMismatch {
                expected: ram.len(),
                actual: data.len(),
            });
        }

        ram.copy_from_slice(data);
        Ok(())
    }

    fn save_rtc(&self) -> Option<RtcState> {
        None
    }

    fn load_rtc(&mut self, _state: RtcState) -> Result<(), CartridgeError> {
        Ok(())
    }

    fn tick_rtc(&mut self, _elapsed_seconds: u64) {}

    fn requires_rtc_state(&self) -> bool {
        false
    }
}
