use crate::cartridge::mbc::Mapper;
use crate::cartridge::rtc::{Rtc, RtcState};
use crate::error::CartridgeError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Mbc3 {
    ram_enabled: bool,
    rom_bank: u8,
    ram_or_rtc_select: u8,
    latch_armed: bool,
    rtc: Rtc,
    has_rtc: bool,
}

impl Mbc3 {
    #[must_use]
    pub fn with_rtc(has_rtc: bool) -> Self {
        Self {
            has_rtc,
            rom_bank: 1,
            ..Self::default()
        }
    }

    fn rom_bank_count(rom: &[u8]) -> usize {
        (rom.len() / 0x4000).max(1)
    }

    fn selected_rom_bank(&self, rom: &[u8]) -> usize {
        let mut bank = usize::from(self.rom_bank & 0x7F);
        if bank == 0 {
            bank = 1;
        }
        bank % Self::rom_bank_count(rom)
    }

    fn selected_ram_bank(&self, ram: &[u8]) -> usize {
        if ram.is_empty() {
            return 0;
        }
        let banks = (ram.len() / 0x2000).max(1);
        usize::from(self.ram_or_rtc_select & 0x03) % banks
    }
}

impl Mapper for Mbc3 {
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
            0x2000..=0x3FFF => {
                self.rom_bank = value & 0x7F;
                if self.rom_bank == 0 {
                    self.rom_bank = 1;
                }
            }
            0x4000..=0x5FFF => {
                self.ram_or_rtc_select = value & 0x0F;
            }
            0x6000..=0x7FFF => {
                if self.latch_armed && value == 1 {
                    self.rtc.latch();
                }
                self.latch_armed = value == 0;
            }
            _ => {}
        }
        Ok(())
    }

    fn read_ram(&self, ram: &[u8], addr: u16) -> Result<u8, CartridgeError> {
        if !self.ram_enabled {
            return Ok(0xFF);
        }

        match self.ram_or_rtc_select {
            0x00..=0x03 => {
                if ram.is_empty() {
                    Ok(0xFF)
                } else {
                    let bank = self.selected_ram_bank(ram);
                    let offset = usize::from(addr - 0xA000);
                    let index = (bank * 0x2000 + offset) % ram.len();
                    Ok(ram[index])
                }
            }
            0x08..=0x0C if self.has_rtc => Ok(self
                .rtc
                .read_register(self.ram_or_rtc_select)
                .unwrap_or(0xFF)),
            _ => Ok(0xFF),
        }
    }

    fn write_ram(&mut self, ram: &mut [u8], addr: u16, value: u8) -> Result<(), CartridgeError> {
        if !self.ram_enabled {
            return Ok(());
        }

        match self.ram_or_rtc_select {
            0x00..=0x03 => {
                if !ram.is_empty() {
                    let bank = self.selected_ram_bank(ram);
                    let offset = usize::from(addr - 0xA000);
                    let index = (bank * 0x2000 + offset) % ram.len();
                    ram[index] = value;
                }
            }
            0x08..=0x0C if self.has_rtc => {
                let _ = self.rtc.write_register(self.ram_or_rtc_select, value);
            }
            _ => {}
        }

        Ok(())
    }

    fn save_rtc(&self) -> Option<RtcState> {
        if self.has_rtc {
            Some(self.rtc.export_state())
        } else {
            None
        }
    }

    fn load_rtc(&mut self, state: RtcState) -> Result<(), CartridgeError> {
        if self.has_rtc {
            self.rtc.import_state(&state);
            Ok(())
        } else {
            Err(CartridgeError::MissingRtcState)
        }
    }

    fn tick_rtc(&mut self, elapsed_seconds: u64) {
        if self.has_rtc {
            self.rtc.tick(elapsed_seconds);
        }
    }

    fn requires_rtc_state(&self) -> bool {
        self.has_rtc
    }
}
