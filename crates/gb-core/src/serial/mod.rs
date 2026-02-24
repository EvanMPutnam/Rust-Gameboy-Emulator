use crate::Cycles;
use crate::interrupts::{Interrupt, InterruptController};
use serde::{Deserialize, Serialize};

const TRANSFER_CYCLES: Cycles = 4_096;
const START_BIT: u8 = 0x80;
const INTERNAL_CLOCK_BIT: u8 = 0x01;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Serial {
    sb: u8,
    sc: u8,
    transfer_active: bool,
    cycles_until_complete: Cycles,
    received_byte: u8,
}

impl Default for Serial {
    fn default() -> Self {
        Self {
            sb: 0,
            sc: 0x7E,
            transfer_active: false,
            cycles_until_complete: 0,
            received_byte: 0xFF,
        }
    }
}

impl Serial {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn read_sb(&self) -> u8 {
        self.sb
    }

    pub fn write_sb(&mut self, value: u8) {
        self.sb = value;
    }

    #[must_use]
    pub fn read_sc(&self) -> u8 {
        self.sc | 0x7E
    }

    pub fn write_sc(&mut self, value: u8) {
        self.sc = (value & (START_BIT | INTERNAL_CLOCK_BIT)) | 0x7E;
        if (self.sc & START_BIT) != 0 && (self.sc & INTERNAL_CLOCK_BIT) != 0 {
            self.transfer_active = true;
            self.cycles_until_complete = TRANSFER_CYCLES;
        }
    }

    pub fn tick(&mut self, cycles: Cycles, interrupts: &mut InterruptController) {
        if !self.transfer_active {
            return;
        }

        if cycles >= self.cycles_until_complete {
            self.transfer_active = false;
            self.cycles_until_complete = 0;
            self.sb = self.received_byte;
            self.sc = (self.sc & !START_BIT) | 0x7E;
            interrupts.request(Interrupt::Serial);
        } else {
            self.cycles_until_complete -= cycles;
        }
    }

    #[must_use]
    pub fn is_transfer_active(&self) -> bool {
        self.transfer_active
    }
}
