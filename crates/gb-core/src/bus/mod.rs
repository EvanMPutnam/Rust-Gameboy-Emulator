use crate::Cycles;
use crate::boot::BootRom;
use crate::cartridge::Cartridge;
use crate::debug::{DebugEvent, Watchpoint};
use crate::error::BusError;
use crate::interrupts::InterruptController;
use crate::joypad::Joypad;
use crate::memory::{HRAM_SIZE, OAM_SIZE, VRAM_SIZE, WRAM_SIZE};
use crate::serial::Serial;
use crate::{Address, Byte, Word};
use memory_map::{
    ECHO_RAM_END, ECHO_RAM_START, EXT_RAM_END, EXT_RAM_START, HRAM_END, HRAM_START, IE_ADDR,
    IO_END, IO_START, OAM_END, OAM_START, ROM_BANK_0_END, ROM_BANK_0_START, ROM_BANK_N_END,
    ROM_BANK_N_START, UNUSABLE_END, UNUSABLE_START, VRAM_END, VRAM_START, WRAM_END, WRAM_START,
};
use serde::{Deserialize, Serialize};
use serde_big_array::BigArray;
use std::cell::RefCell;

pub mod memory_map;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimerRegs {
    pub div: u8,
    pub tima: u8,
    pub tma: u8,
    pub tac: u8,
}

pub trait Memory {
    fn read8(&self, addr: Address) -> Result<Byte, BusError>;
    fn write8(&mut self, addr: Address, val: Byte) -> Result<(), BusError>;

    fn read16(&self, addr: Address) -> Result<Word, BusError> {
        let lo = self.read8(addr)?;
        let hi = self.read8(addr.wrapping_add(1))?;
        Ok(u16::from(lo) | (u16::from(hi) << 8))
    }

    fn write16(&mut self, addr: Address, val: Word) -> Result<(), BusError> {
        let [lo, hi] = val.to_le_bytes();
        self.write8(addr, lo)?;
        self.write8(addr.wrapping_add(1), hi)?;
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
pub struct Bus {
    pub cartridge: Cartridge,
    #[serde(with = "BigArray")]
    pub vram: [u8; VRAM_SIZE],
    #[serde(with = "BigArray")]
    pub wram: [u8; WRAM_SIZE],
    #[serde(with = "BigArray")]
    pub oam: [u8; OAM_SIZE],
    #[serde(with = "BigArray")]
    pub hram: [u8; HRAM_SIZE],
    #[serde(with = "BigArray")]
    pub io: [u8; 0x80],
    pub ie: u8,
    pub joypad: Joypad,
    pub serial: Serial,
    boot_rom: Option<BootRom>,
    vram_blocked: bool,
    oam_blocked: bool,
    dma_active: bool,
    dma_start_request: Option<u8>,
    #[serde(skip)]
    apu_write_mask: u128,
    #[serde(skip)]
    debug: DebugTraceState,
}

#[derive(Default)]
struct DebugTraceState {
    enabled: bool,
    watchpoints: Vec<Watchpoint>,
    events: RefCell<Vec<DebugEvent>>,
}

impl Bus {
    #[must_use]
    pub fn new(cartridge: Cartridge) -> Self {
        Self {
            cartridge,
            vram: [0; VRAM_SIZE],
            wram: [0; WRAM_SIZE],
            oam: [0; OAM_SIZE],
            hram: [0; HRAM_SIZE],
            io: [0; 0x80],
            ie: 0,
            joypad: Joypad::new(),
            serial: Serial::new(),
            boot_rom: None,
            vram_blocked: false,
            oam_blocked: false,
            dma_active: false,
            dma_start_request: None,
            apu_write_mask: 0,
            debug: DebugTraceState::default(),
        }
    }

    #[must_use]
    pub fn read_if(&self) -> u8 {
        self.io[0x0F] & 0x1F
    }

    pub fn write_if(&mut self, value: u8) {
        self.io[0x0F] = value & 0x1F;
    }

    #[must_use]
    pub fn read_ie(&self) -> u8 {
        self.ie
    }

    pub fn write_ie(&mut self, value: u8) {
        self.ie = value;
    }

    #[must_use]
    pub fn timer_regs(&self) -> TimerRegs {
        TimerRegs {
            div: self.io[0x04],
            tima: self.io[0x05],
            tma: self.io[0x06],
            tac: self.io[0x07] & 0x07,
        }
    }

    pub fn set_timer_regs(&mut self, regs: TimerRegs) {
        self.io[0x04] = regs.div;
        self.io[0x05] = regs.tima;
        self.io[0x06] = regs.tma;
        self.io[0x07] = regs.tac & 0x07;
    }

    pub fn set_ppu_access(&mut self, vram_blocked: bool, oam_blocked: bool) {
        self.vram_blocked = vram_blocked;
        self.oam_blocked = oam_blocked;
    }

    pub fn install_boot_rom(&mut self, data: [u8; 256]) {
        self.boot_rom = Some(BootRom::from_array(data));
        self.io[0x50] = 0;
    }

    pub fn apply_post_boot_state(&mut self) {
        self.io.fill(0);
        self.hram.fill(0);

        self.joypad.write_p1(0xCF);

        self.io[0x0F] = 0xE1;

        self.io[0x10] = 0x80;
        self.io[0x11] = 0xBF;
        self.io[0x12] = 0xF3;
        self.io[0x14] = 0xBF;
        self.io[0x16] = 0x3F;
        self.io[0x17] = 0x00;
        self.io[0x19] = 0xBF;
        self.io[0x1A] = 0x7F;
        self.io[0x1B] = 0xFF;
        self.io[0x1C] = 0x9F;
        self.io[0x1E] = 0xBF;
        self.io[0x20] = 0xFF;
        self.io[0x21] = 0x00;
        self.io[0x22] = 0x00;
        self.io[0x23] = 0xBF;
        self.io[0x24] = 0x77;
        self.io[0x25] = 0xF3;
        self.io[0x26] = 0xF1;

        self.io[0x40] = 0x91;
        self.io[0x41] = 0x85;
        self.io[0x42] = 0x00;
        self.io[0x43] = 0x00;
        self.io[0x44] = 0x00;
        self.io[0x45] = 0x00;
        self.io[0x47] = 0xFC;
        self.io[0x48] = 0xFF;
        self.io[0x49] = 0xFF;
        self.io[0x4A] = 0x00;
        self.io[0x4B] = 0x00;
        self.io[0x50] = 0x01;

        self.ie = 0x00;
    }

    #[must_use]
    pub fn is_boot_rom_enabled(&self) -> bool {
        self.boot_rom.as_ref().is_some_and(BootRom::is_enabled)
    }

    pub fn set_dma_active(&mut self, active: bool) {
        self.dma_active = active;
    }

    #[must_use]
    pub fn take_dma_start_request(&mut self) -> Option<u8> {
        self.dma_start_request.take()
    }

    #[must_use]
    pub fn take_apu_write_mask(&mut self) -> u128 {
        let mask = self.apu_write_mask;
        self.apu_write_mask = 0;
        mask
    }

    pub fn tick_serial(&mut self, cycles: Cycles, interrupts: &mut InterruptController) {
        self.serial.tick(cycles, interrupts);
    }

    pub fn read_dma_source(&self, addr: u16) -> Result<u8, BusError> {
        let value = self.read8_raw(addr, true)?;
        self.trace_event(DebugEvent::MemRead { addr, value });
        Ok(value)
    }

    pub fn write_oam_dma(&mut self, index: u16, value: u8) {
        if index < 0x00A0 {
            self.oam[usize::from(index)] = value;
        }
    }

    pub fn configure_debug_tracing(&mut self, enabled: bool, watchpoints: &[Watchpoint]) {
        self.debug.enabled = enabled;
        self.debug.watchpoints.clear();
        self.debug.watchpoints.extend_from_slice(watchpoints);
        if !enabled {
            self.debug.events.borrow_mut().clear();
        }
    }

    #[must_use]
    pub fn take_debug_events(&self) -> Vec<DebugEvent> {
        if !self.debug.enabled {
            return Vec::new();
        }

        let mut events = self.debug.events.borrow_mut();
        core::mem::take(&mut *events)
    }

    pub fn peek8(&self, addr: Address) -> Result<Byte, BusError> {
        self.read8_raw(addr, false)
    }

    fn cpu_access_blocked_by_dma(addr: Address) -> bool {
        !matches!(addr, HRAM_START..=HRAM_END)
    }

    fn read8_raw(&self, addr: Address, ignore_ppu_block: bool) -> Result<Byte, BusError> {
        match addr {
            ROM_BANK_0_START..=ROM_BANK_0_END | ROM_BANK_N_START..=ROM_BANK_N_END => {
                if let Some(boot) = self.boot_rom
                    && let Some(value) = boot.read(addr)
                {
                    return Ok(value);
                }
                self.cartridge.read_rom(addr).map_err(Into::into)
            }
            VRAM_START..=VRAM_END => {
                if !ignore_ppu_block && self.vram_blocked {
                    Ok(0xFF)
                } else {
                    Ok(self.vram[usize::from(addr - VRAM_START)])
                }
            }
            EXT_RAM_START..=EXT_RAM_END => self.cartridge.read_ram(addr).map_err(Into::into),
            WRAM_START..=WRAM_END => Ok(self.wram[usize::from(addr - WRAM_START)]),
            ECHO_RAM_START..=ECHO_RAM_END => Ok(self.wram[usize::from(addr - ECHO_RAM_START)]),
            OAM_START..=OAM_END => {
                if !ignore_ppu_block && self.oam_blocked {
                    Ok(0xFF)
                } else {
                    Ok(self.oam[usize::from(addr - OAM_START)])
                }
            }
            UNUSABLE_START..=UNUSABLE_END => Ok(0xFF),
            IO_START..=IO_END => match addr {
                0xFF00 => Ok(self.joypad.read_p1()),
                0xFF01 => Ok(self.serial.read_sb()),
                0xFF02 => Ok(self.serial.read_sc()),
                _ => Ok(self.io[usize::from(addr - IO_START)]),
            },
            HRAM_START..=HRAM_END => Ok(self.hram[usize::from(addr - HRAM_START)]),
            IE_ADDR => Ok(self.ie),
        }
    }

    fn trace_event(&self, event: DebugEvent) {
        if !self.debug.enabled {
            return;
        }

        let should_record = self.debug.watchpoints.is_empty()
            || self
                .debug
                .watchpoints
                .iter()
                .any(|watchpoint| watchpoint.matches(&event))
            || matches!(
                event,
                DebugEvent::CpuStep { .. } | DebugEvent::InterruptServiced { .. }
            );

        if should_record {
            self.debug.events.borrow_mut().push(event);
        }
    }
}

impl Memory for Bus {
    fn read8(&self, addr: Address) -> Result<Byte, BusError> {
        if self.dma_active && Self::cpu_access_blocked_by_dma(addr) {
            self.trace_event(DebugEvent::MemRead { addr, value: 0xFF });
            return Ok(0xFF);
        }

        let value = self.read8_raw(addr, false)?;
        self.trace_event(DebugEvent::MemRead { addr, value });
        Ok(value)
    }

    fn write8(&mut self, addr: Address, val: Byte) -> Result<(), BusError> {
        if self.dma_active && Self::cpu_access_blocked_by_dma(addr) {
            return Ok(());
        }

        self.trace_event(DebugEvent::MemWrite { addr, value: val });

        match addr {
            ROM_BANK_0_START..=ROM_BANK_0_END | ROM_BANK_N_START..=ROM_BANK_N_END => {
                self.cartridge.write_rom(addr, val)?;
            }
            VRAM_START..=VRAM_END => {
                if !self.vram_blocked {
                    self.vram[usize::from(addr - VRAM_START)] = val;
                }
            }
            EXT_RAM_START..=EXT_RAM_END => {
                self.cartridge.write_ram(addr, val)?;
            }
            WRAM_START..=WRAM_END => {
                self.wram[usize::from(addr - WRAM_START)] = val;
            }
            ECHO_RAM_START..=ECHO_RAM_END => {
                self.wram[usize::from(addr - ECHO_RAM_START)] = val;
            }
            OAM_START..=OAM_END => {
                if !self.oam_blocked {
                    self.oam[usize::from(addr - OAM_START)] = val;
                }
            }
            UNUSABLE_START..=UNUSABLE_END => {}
            IO_START..=IO_END => match addr {
                0xFF00 => {
                    self.joypad.write_p1(val);
                }
                0xFF01 => {
                    self.serial.write_sb(val);
                }
                0xFF02 => {
                    self.serial.write_sc(val);
                }
                0xFF04 => {
                    self.io[0x04] = 0;
                }
                0xFF41 => {
                    let writable = val & 0xF8;
                    let read_only = self.io[0x41] & 0x07;
                    self.io[0x41] = writable | read_only;
                }
                0xFF44 => {
                    self.io[0x44] = 0;
                }
                0xFF46 => {
                    self.io[0x46] = val;
                    self.dma_start_request = Some(val);
                }
                0xFF50 => {
                    self.io[0x50] = val;
                    if val != 0
                        && let Some(boot) = &mut self.boot_rom
                    {
                        boot.disable();
                    }
                }
                _ => {
                    let index = usize::from(addr - IO_START);
                    self.io[index] = val;
                    if (0x10..=0x26).contains(&index) || (0x30..=0x3F).contains(&index) {
                        self.apu_write_mask |= 1u128 << index;
                    }
                }
            },
            HRAM_START..=HRAM_END => {
                self.hram[usize::from(addr - HRAM_START)] = val;
            }
            IE_ADDR => {
                self.ie = val;
            }
        }

        Ok(())
    }
}
