use crate::cartridge::mbc::Mapper;
use crate::error::CartridgeError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct RomOnly;

impl Mapper for RomOnly {
    fn read_rom(&self, rom: &[u8], addr: u16) -> Result<u8, CartridgeError> {
        let index = usize::from(addr) % rom.len();
        Ok(rom[index])
    }

    fn write_rom(&mut self, _addr: u16, _value: u8) -> Result<(), CartridgeError> {
        Ok(())
    }

    fn read_ram(&self, ram: &[u8], addr: u16) -> Result<u8, CartridgeError> {
        if ram.is_empty() {
            return Ok(0xFF);
        }

        let index = usize::from(addr - 0xA000) % ram.len();
        Ok(ram[index])
    }

    fn write_ram(&mut self, ram: &mut [u8], addr: u16, value: u8) -> Result<(), CartridgeError> {
        if ram.is_empty() {
            return Ok(());
        }

        let index = usize::from(addr - 0xA000) % ram.len();
        ram[index] = value;
        Ok(())
    }
}
