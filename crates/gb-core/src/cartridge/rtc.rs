#![allow(clippy::cast_possible_truncation)] // RTC register values are range-limited before narrowing.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RtcState {
    pub seconds: u8,
    pub minutes: u8,
    pub hours: u8,
    pub day_counter: u16,
    pub halt: bool,
    pub day_carry: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rtc {
    live: RtcState,
    latched: RtcState,
}

impl Rtc {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn tick(&mut self, elapsed_seconds: u64) {
        self.live.tick(elapsed_seconds);
    }

    pub fn latch(&mut self) {
        self.latched = self.live;
    }

    #[must_use]
    pub fn read_register(&self, reg: u8) -> Option<u8> {
        self.latched.read_register(reg)
    }

    pub fn write_register(&mut self, reg: u8, value: u8) -> bool {
        self.live.write_register(reg, value)
    }

    #[must_use]
    pub fn export_state(&self) -> RtcState {
        self.live
    }

    pub fn import_state(&mut self, state: &RtcState) {
        self.live = *state;
        self.latched = *state;
    }
}

impl RtcState {
    fn tick(&mut self, elapsed_seconds: u64) {
        if self.halt {
            return;
        }

        let day_seconds = 86_400u64;
        let current_seconds =
            u64::from(self.hours) * 3_600 + u64::from(self.minutes) * 60 + u64::from(self.seconds);
        let total_seconds = current_seconds + elapsed_seconds;

        let day_increment = total_seconds / day_seconds;
        let seconds_in_day = total_seconds % day_seconds;

        self.hours = (seconds_in_day / 3_600) as u8;
        self.minutes = ((seconds_in_day % 3_600) / 60) as u8;
        self.seconds = (seconds_in_day % 60) as u8;

        let day_total = u64::from(self.day_counter) + day_increment;
        if day_total > 511 {
            self.day_carry = true;
        }
        self.day_counter = (day_total % 512) as u16;
    }

    fn read_register(self, reg: u8) -> Option<u8> {
        match reg {
            0x08 => Some(self.seconds % 60),
            0x09 => Some(self.minutes % 60),
            0x0A => Some(self.hours % 24),
            0x0B => Some((self.day_counter & 0x00FF) as u8),
            0x0C => {
                let day_high = ((self.day_counter >> 8) & 0x0001) as u8;
                Some(day_high | (u8::from(self.halt) << 6) | (u8::from(self.day_carry) << 7))
            }
            _ => None,
        }
    }

    fn write_register(&mut self, reg: u8, value: u8) -> bool {
        match reg {
            0x08 => {
                self.seconds = value % 60;
                true
            }
            0x09 => {
                self.minutes = value % 60;
                true
            }
            0x0A => {
                self.hours = value % 24;
                true
            }
            0x0B => {
                self.day_counter = (self.day_counter & 0x0100) | u16::from(value);
                true
            }
            0x0C => {
                self.day_counter = (self.day_counter & 0x00FF) | (u16::from(value & 0x01) << 8);
                self.halt = (value & 0x40) != 0;
                self.day_carry = (value & 0x80) != 0;
                true
            }
            _ => false,
        }
    }
}
