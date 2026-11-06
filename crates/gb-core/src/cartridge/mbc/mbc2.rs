use crate::cartridge::mbc::Mapper;
use crate::error::CartridgeError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Mbc2 {
    ram_enabled: bool,
    rom_bank: u8,
}

impl Default for Mbc2 {
    fn default() -> Self {
        Self {
            ram_enabled: false,
            rom_bank: 1,
        }
    }
}

impl Mbc2 {
    fn rom_bank_count(rom: &[u8]) -> usize {
        (rom.len() / 0x4000).max(1)
    }

    fn normalize_rom_bank(bank: u8, rom: &[u8]) -> usize {
        let count = Self::rom_bank_count(rom);
        let mut selected = usize::from(bank & 0x0F);
        if selected == 0 {
            selected = 1;
        }
        selected % count
    }
}

impl Mapper for Mbc2 {
    fn read_rom(&self, rom: &[u8], addr: u16) -> Result<u8, CartridgeError> {
        let (bank, offset) = if addr < 0x4000 {
            (0, usize::from(addr))
        } else {
            (
                Self::normalize_rom_bank(self.rom_bank, rom),
                usize::from(addr - 0x4000),
            )
        };
        let index = (bank * 0x4000 + offset) % rom.len();
        Ok(rom[index])
    }

    fn write_rom(&mut self, addr: u16, value: u8) -> Result<(), CartridgeError> {
        match addr {
            // Address bit 8 picks the register anywhere in 0x0000-0x3FFF.
            0x0000..=0x3FFF if (addr & 0x0100) == 0 => {
                self.ram_enabled = (value & 0x0F) == 0x0A;
            }
            0x0000..=0x3FFF => {
                let mut bank = value & 0x0F;
                if bank == 0 {
                    bank = 1;
                }
                self.rom_bank = bank;
            }
            _ => {}
        }
        Ok(())
    }

    fn read_ram(&self, ram: &[u8], addr: u16) -> Result<u8, CartridgeError> {
        if !self.ram_enabled || ram.is_empty() {
            return Ok(0xFF);
        }

        let index = usize::from(addr - 0xA000) % ram.len();
        Ok(0xF0 | (ram[index] & 0x0F))
    }

    fn write_ram(&mut self, ram: &mut [u8], addr: u16, value: u8) -> Result<(), CartridgeError> {
        if !self.ram_enabled || ram.is_empty() {
            return Ok(());
        }

        let index = usize::from(addr - 0xA000) % ram.len();
        ram[index] = value & 0x0F;
        Ok(())
    }
}
