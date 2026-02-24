use crate::cartridge::mbc::Mapper;
use crate::error::CartridgeError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Mbc1 {
    ram_enabled: bool,
    rom_bank_low5: u8,
    bank_upper2: u8,
    banking_mode: u8,
}

impl Default for Mbc1 {
    fn default() -> Self {
        Self {
            ram_enabled: false,
            rom_bank_low5: 1,
            bank_upper2: 0,
            banking_mode: 0,
        }
    }
}

impl Mbc1 {
    fn rom_bank_count(rom: &[u8]) -> usize {
        (rom.len() / 0x4000).max(1)
    }

    fn normalize_rom_bank(bank: u8, rom: &[u8]) -> usize {
        let count = Self::rom_bank_count(rom);
        let mut selected = usize::from(bank);
        if selected == 0 {
            selected = 1;
        }

        selected % count
    }

    fn low_region_bank(&self, rom: &[u8]) -> usize {
        if self.banking_mode == 0 {
            return 0;
        }

        let raw = self.bank_upper2 << 5;
        Self::normalize_rom_bank(raw, rom)
    }

    fn high_region_bank(&self, rom: &[u8]) -> usize {
        let raw = (self.bank_upper2 << 5) | (self.rom_bank_low5 & 0x1F);
        Self::normalize_rom_bank(raw, rom)
    }

    fn ram_bank(&self, ram: &[u8]) -> usize {
        if ram.is_empty() || self.banking_mode == 0 {
            return 0;
        }

        usize::from(self.bank_upper2 & 0x03)
    }
}

impl Mapper for Mbc1 {
    fn read_rom(&self, rom: &[u8], addr: u16) -> Result<u8, CartridgeError> {
        let (bank, offset) = if addr < 0x4000 {
            (self.low_region_bank(rom), usize::from(addr))
        } else {
            (self.high_region_bank(rom), usize::from(addr - 0x4000))
        };

        let index = (bank * 0x4000 + offset) % rom.len();
        Ok(rom[index])
    }

    fn write_rom(&mut self, addr: u16, value: u8) -> Result<(), CartridgeError> {
        match addr {
            0x0000..=0x1FFF => {
                self.ram_enabled = (value & 0x0F) == 0x0A;
            }
            0x2000..=0x3FFF => {
                let mut bank = value & 0x1F;
                if bank == 0 {
                    bank = 1;
                }
                self.rom_bank_low5 = bank;
            }
            0x4000..=0x5FFF => {
                self.bank_upper2 = value & 0x03;
            }
            0x6000..=0x7FFF => {
                self.banking_mode = value & 0x01;
            }
            _ => {}
        }

        Ok(())
    }

    fn read_ram(&self, ram: &[u8], addr: u16) -> Result<u8, CartridgeError> {
        if !self.ram_enabled || ram.is_empty() {
            return Ok(0xFF);
        }

        let bank = self.ram_bank(ram);
        let offset = usize::from(addr - 0xA000);
        let index = (bank * 0x2000 + offset) % ram.len();
        Ok(ram[index])
    }

    fn write_ram(&mut self, ram: &mut [u8], addr: u16, value: u8) -> Result<(), CartridgeError> {
        if !self.ram_enabled || ram.is_empty() {
            return Ok(());
        }

        let bank = self.ram_bank(ram);
        let offset = usize::from(addr - 0xA000);
        let index = (bank * 0x2000 + offset) % ram.len();
        ram[index] = value;
        Ok(())
    }
}
