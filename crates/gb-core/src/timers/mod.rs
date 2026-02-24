use crate::Cycles;
use crate::bus::TimerRegs;
use crate::interrupts::{Interrupt, InterruptController};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct Timers {
    div_counter: u16,
    tima_counter: Cycles,
    tima: u8,
    tma: u8,
    tac: u8,
}

impl Timers {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn export_regs(&self) -> TimerRegs {
        let [div, _] = self.div_counter.to_be_bytes();
        TimerRegs {
            div,
            tima: self.tima,
            tma: self.tma,
            tac: self.tac & 0x07,
        }
    }

    pub fn apply_bus_regs(&mut self, regs: TimerRegs) {
        let [div, _] = self.div_counter.to_be_bytes();
        if regs.div != div {
            self.write_div();
        }

        self.tima = regs.tima;
        self.tma = regs.tma;
        self.tac = regs.tac & 0x07;
    }

    pub fn tick(&mut self, cycles: Cycles, interrupts: &mut InterruptController) {
        let [b0, b1, _, _] = cycles.to_le_bytes();
        let delta = u16::from_le_bytes([b0, b1]);
        self.div_counter = self.div_counter.wrapping_add(delta);

        if (self.tac & 0x04) == 0 {
            return;
        }

        self.tima_counter = self.tima_counter.wrapping_add(cycles);
        let threshold = match self.tac & 0x03 {
            0x00 => 1024,
            0x01 => 16,
            0x02 => 64,
            _ => 256,
        };

        while self.tima_counter >= threshold {
            self.tima_counter -= threshold;
            if self.tima == u8::MAX {
                self.tima = self.tma;
                interrupts.request(Interrupt::Timer);
            } else {
                self.tima = self.tima.wrapping_add(1);
            }
        }
    }

    fn write_div(&mut self) {
        self.div_counter = 0;
        self.tima_counter = 0;
    }
}
