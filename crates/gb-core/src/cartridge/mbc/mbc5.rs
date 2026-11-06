use crate::cartridge::mbc::Mapper;
use crate::error::CartridgeError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Mbc5 {
    ram_enabled: bool,
    rom_bank_low: u8,
    rom_bank_high: u8,
    ram_bank: u8,
}

impl Default for Mbc5 {
    fn default() -> Self {
        Self {
            ram_enabled: false,
            rom_bank_low: 1,
            rom_bank_high: 0,
            ram_bank: 0,
        }
    }
}

impl Mbc5 {
    fn rom_bank_count(rom: &[u8]) -> usize {
        (rom.len() / 0x4000).max(1)
    }

    fn selected_rom_bank(&self, rom: &[u8]) -> usize {
        let bank = usize::from(self.rom_bank_low) | (usize::from(self.rom_bank_high & 0x01) << 8);
        bank % Self::rom_bank_count(rom)
    }

    fn selected_ram_bank(&self, ram: &[u8]) -> usize {
        if ram.is_empty() {
            return 0;
        }
        let banks = (ram.len() / 0x2000).max(1);
        usize::from(self.ram_bank & 0x0F) % banks
    }
}

impl Mapper for Mbc5 {
    fn read_rom(&self, rom: &[u8], addr: u16) -> Result<u8, CartridgeError> {
        let (bank, offset) = if addr < 0x4000 {
            (0, usize::from(addr))
        } else {
            (self.selected_rom_bank(rom), usize::from(addr - 0x4000))
        };

        let index = (bank * 0x4000 + offset) % rom.len();
        Ok(rom[index])
    }

    fn write_rom(&mut self, addr: u16, value: u8) -> Result<(), CartridgeError> {
        match addr {
            0x0000..=0x1FFF => {
                self.ram_enabled = (value & 0x0F) == 0x0A;
            }
            0x2000..=0x2FFF => {
                self.rom_bank_low = value;
            }
            0x3000..=0x3FFF => {
                self.rom_bank_high = value & 0x01;
            }
            0x4000..=0x5FFF => {
                self.ram_bank = value & 0x0F;
            }
            _ => {}
        }
        Ok(())
    }

    fn read_ram(&self, ram: &[u8], addr: u16) -> Result<u8, CartridgeError> {
        if !self.ram_enabled || ram.is_empty() {
            return Ok(0xFF);
        }

        let bank = self.selected_ram_bank(ram);
        let offset = usize::from(addr - 0xA000);
        let index = (bank * 0x2000 + offset) % ram.len();
        Ok(ram[index])
    }

    fn write_ram(&mut self, ram: &mut [u8], addr: u16, value: u8) -> Result<(), CartridgeError> {
        if !self.ram_enabled || ram.is_empty() {
            return Ok(());
        }

        let bank = self.selected_ram_bank(ram);
        let offset = usize::from(addr - 0xA000);
        let index = (bank * 0x2000 + offset) % ram.len();
        ram[index] = value;
        Ok(())
    }
}
