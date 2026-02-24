use crate::interrupts::{Interrupt, InterruptController};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GbButton {
    Right,
    Left,
    Up,
    Down,
    A,
    B,
    Select,
    Start,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Joypad {
    select_dpad: bool,
    select_buttons: bool,
    pressed_mask: u8,
}

impl Joypad {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn press(&mut self, button: GbButton, interrupts: &mut InterruptController) {
        let before = self.selected_lines();
        self.pressed_mask |= button_mask(button);
        let after = self.selected_lines();

        if (after & !before) != 0 {
            interrupts.request(Interrupt::Joypad);
        }
    }

    pub fn release(&mut self, button: GbButton) {
        self.pressed_mask &= !button_mask(button);
    }

    #[must_use]
    pub fn read_p1(&self) -> u8 {
        let mut value = 0xC0 | 0x0F;
        if !self.select_dpad {
            value |= 0x10;
        }
        if !self.select_buttons {
            value |= 0x20;
        }

        value & !self.selected_lines()
    }

    pub fn write_p1(&mut self, value: u8) {
        self.select_dpad = (value & 0x10) == 0;
        self.select_buttons = (value & 0x20) == 0;
    }

    fn selected_lines(self) -> u8 {
        let mut low = 0;
        if self.select_dpad {
            low |= self.pressed_mask & 0x0F;
        }
        if self.select_buttons {
            low |= (self.pressed_mask >> 4) & 0x0F;
        }
        low
    }
}

const fn button_mask(button: GbButton) -> u8 {
    match button {
        GbButton::Right => 1 << 0,
        GbButton::Left => 1 << 1,
        GbButton::Up => 1 << 2,
        GbButton::Down => 1 << 3,
        GbButton::A => 1 << 4,
        GbButton::B => 1 << 5,
        GbButton::Select => 1 << 6,
        GbButton::Start => 1 << 7,
    }
}
