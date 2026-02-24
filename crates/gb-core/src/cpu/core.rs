use crate::Cycles;
use crate::bus::Memory;
use crate::cpu::decode::decode;
use crate::cpu::execute;
use crate::cpu::registers::{Flags, Registers};
use crate::error::CpuError;
use crate::interrupts::InterruptController;
use serde::{Deserialize, Serialize};

#[derive(Default, Serialize, Deserialize)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "CPU latch state mirrors discrete hardware control flags (IME, delayed IME, HALT, HALT bug)."
)]
pub struct Cpu {
    regs: Registers,
    ime: bool,
    ime_scheduled: bool,
    halted: bool,
    halt_bug: bool,
    last_interrupt_vector: Option<u16>,
}

impl Cpu {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn registers(&self) -> &Registers {
        &self.regs
    }

    #[must_use]
    pub fn registers_mut(&mut self) -> &mut Registers {
        &mut self.regs
    }

    pub fn set_ime(&mut self, enabled: bool) {
        self.ime = enabled;
        self.ime_scheduled = false;
    }

    #[must_use]
    pub fn ime(&self) -> bool {
        self.ime
    }

    #[must_use]
    pub fn is_halted(&self) -> bool {
        self.halted
    }

    pub fn apply_post_boot_state(&mut self) {
        self.regs.a = 0x01;
        self.regs.f = Flags::from_bits_truncate(0xB0);
        self.regs.b = 0x00;
        self.regs.c = 0x13;
        self.regs.d = 0x00;
        self.regs.e = 0xD8;
        self.regs.h = 0x01;
        self.regs.l = 0x4D;
        self.regs.sp = 0xFFFE;
        self.regs.pc = 0x0100;
        self.ime = false;
        self.ime_scheduled = false;
        self.halted = false;
        self.halt_bug = false;
    }

    pub fn step(
        &mut self,
        mem: &mut dyn Memory,
        interrupts: &mut InterruptController,
    ) -> Result<Cycles, CpuError> {
        self.last_interrupt_vector = None;
        let enable_ime_after_step = self.ime_scheduled;

        if let Some(cycles) = self.service_interrupt(mem, interrupts)? {
            return Ok(cycles);
        }

        if self.halted {
            if interrupts.pending().is_some() {
                self.halted = false;
            } else {
                return Ok(4);
            }
        }

        let opcode = if self.halt_bug {
            self.halt_bug = false;
            mem.read8(self.regs.pc)?
        } else {
            self.fetch8(mem)?
        };

        if opcode == 0x76 && !self.ime && interrupts.pending().is_some() {
            self.halt_bug = true;
            return Ok(4);
        }

        let instruction = decode(opcode);
        let cycles = execute::execute(self, mem, instruction)?;

        if enable_ime_after_step {
            self.ime = true;
            self.ime_scheduled = false;
        }

        Ok(cycles)
    }

    fn service_interrupt(
        &mut self,
        mem: &mut dyn Memory,
        interrupts: &mut InterruptController,
    ) -> Result<Option<Cycles>, CpuError> {
        let Some(interrupt) = interrupts.pending() else {
            return Ok(None);
        };
        if !self.ime {
            return Ok(None);
        }
        interrupts.acknowledge(interrupt);
        mem.write8(0xFF0F, interrupts.iflags)?;

        self.halted = false;
        self.ime = false;
        self.push_word(mem, self.regs.pc)?;
        let vector = interrupt.vector();
        self.regs.pc = vector;
        self.last_interrupt_vector = Some(vector);

        Ok(Some(20))
    }

    #[must_use]
    pub fn take_last_serviced_interrupt_vector(&mut self) -> Option<u16> {
        self.last_interrupt_vector.take()
    }

    pub(crate) fn fetch8(&mut self, mem: &mut dyn Memory) -> Result<u8, CpuError> {
        let value = mem.read8(self.regs.pc)?;
        self.regs.pc = self.regs.pc.wrapping_add(1);
        Ok(value)
    }

    pub(crate) fn fetch16(&mut self, mem: &mut dyn Memory) -> Result<u16, CpuError> {
        let lo = self.fetch8(mem)?;
        let hi = self.fetch8(mem)?;
        Ok(u16::from(lo) | (u16::from(hi) << 8))
    }

    pub(crate) fn push_word(&mut self, mem: &mut dyn Memory, value: u16) -> Result<(), CpuError> {
        let [lo, hi] = value.to_le_bytes();
        self.regs.sp = self.regs.sp.wrapping_sub(1);
        mem.write8(self.regs.sp, hi)?;
        self.regs.sp = self.regs.sp.wrapping_sub(1);
        mem.write8(self.regs.sp, lo)?;
        Ok(())
    }

    pub(crate) fn pop_word(&mut self, mem: &mut dyn Memory) -> Result<u16, CpuError> {
        let lo = mem.read8(self.regs.sp)?;
        self.regs.sp = self.regs.sp.wrapping_add(1);
        let hi = mem.read8(self.regs.sp)?;
        self.regs.sp = self.regs.sp.wrapping_add(1);
        Ok(u16::from(lo) | (u16::from(hi) << 8))
    }

    pub(crate) fn jump_relative(&mut self, offset: i8) {
        self.regs.pc = self.regs.pc.wrapping_add_signed(i16::from(offset));
    }

    pub(crate) fn schedule_ime_enable(&mut self) {
        self.ime_scheduled = true;
    }

    pub(crate) fn disable_ime(&mut self) {
        self.ime = false;
        self.ime_scheduled = false;
    }

    pub(crate) fn set_halted(&mut self) {
        self.halted = true;
    }
}
