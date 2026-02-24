use crate::cartridge::header::CartridgeHeader;
use crate::cartridge::mbc::Mapper;
use crate::cartridge::mbc::mbc1::Mbc1;
use crate::cartridge::mbc::mbc2::Mbc2;
use crate::cartridge::mbc::mbc3::Mbc3;
use crate::cartridge::mbc::mbc5::Mbc5;
use crate::cartridge::mbc::rom_only::RomOnly;
use crate::cartridge::rtc::RtcState;
use crate::cartridge::save::CartridgeSave;
use crate::error::CartridgeError;
use serde::{Deserialize, Serialize};

pub mod header;
pub mod mbc;
pub mod rtc;
pub mod save;

#[derive(Serialize, Deserialize)]
enum MapperImpl {
    RomOnly(RomOnly),
    Mbc1(Mbc1),
    Mbc2(Mbc2),
    Mbc3(Mbc3),
    Mbc5(Mbc5),
}

impl MapperImpl {
    fn read_rom(&self, rom: &[u8], addr: u16) -> Result<u8, CartridgeError> {
        match self {
            Self::RomOnly(mapper) => mapper.read_rom(rom, addr),
            Self::Mbc1(mapper) => mapper.read_rom(rom, addr),
            Self::Mbc2(mapper) => mapper.read_rom(rom, addr),
            Self::Mbc3(mapper) => mapper.read_rom(rom, addr),
            Self::Mbc5(mapper) => mapper.read_rom(rom, addr),
        }
    }

    fn write_rom(&mut self, addr: u16, value: u8) -> Result<(), CartridgeError> {
        match self {
            Self::RomOnly(mapper) => mapper.write_rom(addr, value),
            Self::Mbc1(mapper) => mapper.write_rom(addr, value),
            Self::Mbc2(mapper) => mapper.write_rom(addr, value),
            Self::Mbc3(mapper) => mapper.write_rom(addr, value),
            Self::Mbc5(mapper) => mapper.write_rom(addr, value),
        }
    }

    fn read_ram(&self, ram: &[u8], addr: u16) -> Result<u8, CartridgeError> {
        match self {
            Self::RomOnly(mapper) => mapper.read_ram(ram, addr),
            Self::Mbc1(mapper) => mapper.read_ram(ram, addr),
            Self::Mbc2(mapper) => mapper.read_ram(ram, addr),
            Self::Mbc3(mapper) => mapper.read_ram(ram, addr),
            Self::Mbc5(mapper) => mapper.read_ram(ram, addr),
        }
    }

    fn write_ram(&mut self, ram: &mut [u8], addr: u16, value: u8) -> Result<(), CartridgeError> {
        match self {
            Self::RomOnly(mapper) => mapper.write_ram(ram, addr, value),
            Self::Mbc1(mapper) => mapper.write_ram(ram, addr, value),
            Self::Mbc2(mapper) => mapper.write_ram(ram, addr, value),
            Self::Mbc3(mapper) => mapper.write_ram(ram, addr, value),
            Self::Mbc5(mapper) => mapper.write_ram(ram, addr, value),
        }
    }

    fn save_ram(&self, ram: &[u8]) -> Option<Vec<u8>> {
        match self {
            Self::RomOnly(mapper) => mapper.save_ram(ram),
            Self::Mbc1(mapper) => mapper.save_ram(ram),
            Self::Mbc2(mapper) => mapper.save_ram(ram),
            Self::Mbc3(mapper) => mapper.save_ram(ram),
            Self::Mbc5(mapper) => mapper.save_ram(ram),
        }
    }

    fn load_ram(&mut self, ram: &mut [u8], data: &[u8]) -> Result<(), CartridgeError> {
        match self {
            Self::RomOnly(mapper) => mapper.load_ram(ram, data),
            Self::Mbc1(mapper) => mapper.load_ram(ram, data),
            Self::Mbc2(mapper) => mapper.load_ram(ram, data),
            Self::Mbc3(mapper) => mapper.load_ram(ram, data),
            Self::Mbc5(mapper) => mapper.load_ram(ram, data),
        }
    }

    fn save_rtc(&self) -> Option<RtcState> {
        match self {
            Self::RomOnly(mapper) => mapper.save_rtc(),
            Self::Mbc1(mapper) => mapper.save_rtc(),
            Self::Mbc2(mapper) => mapper.save_rtc(),
            Self::Mbc3(mapper) => mapper.save_rtc(),
            Self::Mbc5(mapper) => mapper.save_rtc(),
        }
    }

    fn load_rtc(&mut self, state: RtcState) -> Result<(), CartridgeError> {
        match self {
            Self::RomOnly(mapper) => mapper.load_rtc(state),
            Self::Mbc1(mapper) => mapper.load_rtc(state),
            Self::Mbc2(mapper) => mapper.load_rtc(state),
            Self::Mbc3(mapper) => mapper.load_rtc(state),
            Self::Mbc5(mapper) => mapper.load_rtc(state),
        }
    }

    fn tick_rtc(&mut self, elapsed_seconds: u64) {
        match self {
            Self::RomOnly(mapper) => mapper.tick_rtc(elapsed_seconds),
            Self::Mbc1(mapper) => mapper.tick_rtc(elapsed_seconds),
            Self::Mbc2(mapper) => mapper.tick_rtc(elapsed_seconds),
            Self::Mbc3(mapper) => mapper.tick_rtc(elapsed_seconds),
            Self::Mbc5(mapper) => mapper.tick_rtc(elapsed_seconds),
        }
    }

    fn requires_rtc_state(&self) -> bool {
        match self {
            Self::RomOnly(mapper) => mapper.requires_rtc_state(),
            Self::Mbc1(mapper) => mapper.requires_rtc_state(),
            Self::Mbc2(mapper) => mapper.requires_rtc_state(),
            Self::Mbc3(mapper) => mapper.requires_rtc_state(),
            Self::Mbc5(mapper) => mapper.requires_rtc_state(),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct Cartridge {
    pub header: CartridgeHeader,
    rom: Vec<u8>,
    ram: Vec<u8>,
    mapper: MapperImpl,
}

impl Cartridge {
    pub fn from_rom_bytes(bytes: Vec<u8>) -> Result<Self, CartridgeError> {
        let header = CartridgeHeader::parse(&bytes)?;
        let mapper = match header.cartridge_type {
            0x00 => MapperImpl::RomOnly(RomOnly),
            0x01..=0x03 => MapperImpl::Mbc1(Mbc1::default()),
            0x05 | 0x06 => MapperImpl::Mbc2(Mbc2::default()),
            0x0F | 0x10 => MapperImpl::Mbc3(Mbc3::with_rtc(true)),
            0x11..=0x13 => MapperImpl::Mbc3(Mbc3::with_rtc(false)),
            0x19..=0x1E => MapperImpl::Mbc5(Mbc5::default()),
            other => return Err(CartridgeError::UnsupportedCartridgeType(other)),
        };

        let expected_banks = decode_rom_banks(header.rom_size_code)?;
        let expected_rom_len = expected_banks * 0x4000;
        let mut rom = bytes;
        if rom.len() < expected_rom_len {
            rom.resize(expected_rom_len, 0xFF);
        }

        let ram_len = if matches!(header.cartridge_type, 0x05 | 0x06) {
            512
        } else {
            decode_ram_len(header.ram_size_code)?
        };

        Ok(Self {
            header,
            rom,
            ram: vec![0; ram_len],
            mapper,
        })
    }

    pub fn read_rom(&self, addr: u16) -> Result<u8, CartridgeError> {
        self.mapper.read_rom(&self.rom, addr)
    }

    pub fn write_rom(&mut self, addr: u16, value: u8) -> Result<(), CartridgeError> {
        self.mapper.write_rom(addr, value)
    }

    pub fn read_ram(&self, addr: u16) -> Result<u8, CartridgeError> {
        self.mapper.read_ram(&self.ram, addr)
    }

    pub fn write_ram(&mut self, addr: u16, value: u8) -> Result<(), CartridgeError> {
        self.mapper.write_ram(&mut self.ram, addr, value)
    }

    #[must_use]
    pub fn export_save(&self) -> CartridgeSave {
        CartridgeSave {
            ram: self.mapper.save_ram(&self.ram),
            rtc: self.mapper.save_rtc(),
        }
    }

    pub fn import_save(&mut self, save: &CartridgeSave) -> Result<(), CartridgeError> {
        if let Some(ram) = &save.ram {
            self.mapper.load_ram(&mut self.ram, ram)?;
        }

        if self.mapper.requires_rtc_state() {
            let state = save.rtc.ok_or(CartridgeError::MissingRtcState)?;
            self.mapper.load_rtc(state)?;
        } else if let Some(state) = save.rtc {
            self.mapper.load_rtc(state)?;
        }

        Ok(())
    }

    pub fn tick_rtc(&mut self, elapsed_seconds: u64) -> Result<(), CartridgeError> {
        self.mapper.tick_rtc(elapsed_seconds);
        Ok(())
    }
}

fn decode_rom_banks(code: u8) -> Result<usize, CartridgeError> {
    match code {
        0x00 => Ok(2),
        0x01 => Ok(4),
        0x02 => Ok(8),
        0x03 => Ok(16),
        0x04 => Ok(32),
        0x05 => Ok(64),
        0x06 => Ok(128),
        0x07 => Ok(256),
        0x08 => Ok(512),
        other => Err(CartridgeError::UnsupportedRomSizeCode(other)),
    }
}

fn decode_ram_len(code: u8) -> Result<usize, CartridgeError> {
    match code {
        0x00 => Ok(0),
        0x01 => Ok(2 * 1024),
        0x02 => Ok(8 * 1024),
        0x03 => Ok(32 * 1024),
        0x04 => Ok(128 * 1024),
        0x05 => Ok(64 * 1024),
        other => Err(CartridgeError::UnsupportedRamSizeCode(other)),
    }
}
