use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Interrupt {
    VBlank = 0,
    LcdStat = 1,
    Timer = 2,
    Serial = 3,
    Joypad = 4,
}

impl Interrupt {
    #[must_use]
    pub fn bit(self) -> u8 {
        1u8 << (self as u8)
    }

    #[must_use]
    pub fn vector(self) -> u16 {
        match self {
            Self::VBlank => 0x40,
            Self::LcdStat => 0x48,
            Self::Timer => 0x50,
            Self::Serial => 0x58,
            Self::Joypad => 0x60,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterruptController {
    pub ie: u8,
    pub iflags: u8,
}

impl InterruptController {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn request(&mut self, interrupt: Interrupt) {
        self.iflags |= interrupt.bit();
    }

    #[must_use]
    pub fn pending(&self) -> Option<Interrupt> {
        let masked = self.iflags & self.ie & 0x1F;

        if masked & Interrupt::VBlank.bit() != 0 {
            Some(Interrupt::VBlank)
        } else if masked & Interrupt::LcdStat.bit() != 0 {
            Some(Interrupt::LcdStat)
        } else if masked & Interrupt::Timer.bit() != 0 {
            Some(Interrupt::Timer)
        } else if masked & Interrupt::Serial.bit() != 0 {
            Some(Interrupt::Serial)
        } else if masked & Interrupt::Joypad.bit() != 0 {
            Some(Interrupt::Joypad)
        } else {
            None
        }
    }

    pub fn acknowledge(&mut self, interrupt: Interrupt) {
        self.iflags &= !interrupt.bit();
    }
}
