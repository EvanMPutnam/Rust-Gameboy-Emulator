use bitflags::bitflags;
use serde::{Deserialize, Serialize};

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    pub struct Flags: u8 {
        const Z = 0b1000_0000;
        const N = 0b0100_0000;
        const H = 0b0010_0000;
        const C = 0b0001_0000;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Registers {
    pub a: u8,
    pub f: Flags,
    pub b: u8,
    pub c: u8,
    pub d: u8,
    pub e: u8,
    pub h: u8,
    pub l: u8,
    pub sp: u16,
    pub pc: u16,
}

impl Default for Registers {
    fn default() -> Self {
        Self {
            a: 0,
            f: Flags::empty(),
            b: 0,
            c: 0,
            d: 0,
            e: 0,
            h: 0,
            l: 0,
            sp: 0xFFFE,
            pc: 0x0100,
        }
    }
}

impl Registers {
    #[must_use]
    pub fn af(&self) -> u16 {
        (u16::from(self.a) << 8) | u16::from(self.f.bits())
    }

    pub fn set_af(&mut self, value: u16) {
        let [lo, hi] = value.to_le_bytes();
        self.a = hi;
        self.f = Flags::from_bits_truncate(lo & 0xF0);
    }

    #[must_use]
    pub fn bc(&self) -> u16 {
        (u16::from(self.b) << 8) | u16::from(self.c)
    }

    pub fn set_bc(&mut self, value: u16) {
        let [lo, hi] = value.to_le_bytes();
        self.b = hi;
        self.c = lo;
    }

    #[must_use]
    pub fn de(&self) -> u16 {
        (u16::from(self.d) << 8) | u16::from(self.e)
    }

    pub fn set_de(&mut self, value: u16) {
        let [lo, hi] = value.to_le_bytes();
        self.d = hi;
        self.e = lo;
    }

    #[must_use]
    pub fn hl(&self) -> u16 {
        (u16::from(self.h) << 8) | u16::from(self.l)
    }

    pub fn set_hl(&mut self, value: u16) {
        let [lo, hi] = value.to_le_bytes();
        self.h = hi;
        self.l = lo;
    }

    pub fn set_flag(&mut self, flag: Flags, enabled: bool) {
        self.f.set(flag, enabled);
    }

    #[must_use]
    pub fn flag(&self, flag: Flags) -> bool {
        self.f.contains(flag)
    }
}
