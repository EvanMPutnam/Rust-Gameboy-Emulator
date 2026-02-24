#![allow(clippy::missing_errors_doc)] // Public API docs will be finalized after phased bring-up stabilizes.

pub mod apu;
pub mod boot;
pub mod bus;
pub mod cartridge;
pub mod config;
pub mod cpu;
pub mod debug;
pub mod dma;
pub mod error;
pub mod gameboy;
pub mod interrupts;
pub mod joypad;
pub mod memory;
pub mod ppu;
pub mod scheduler;
pub mod serial;
pub mod state;
pub mod timers;

pub use crate::cartridge::save::CartridgeSave;
pub use crate::config::GbConfig;
pub use crate::error::{
    ApuError, BootError, BusError, CartridgeError, CpuError, DmaError, GbError, JoypadError,
    PpuError, SerialError, StateError,
};
pub use crate::gameboy::GameBoy;
pub use crate::joypad::GbButton;

pub type Cycles = u32;
pub type Address = u16;
pub type Byte = u8;
pub type Word = u16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TickResult {
    Continue,
    FrameComplete,
}
