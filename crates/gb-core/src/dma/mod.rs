use crate::Cycles;
use crate::bus::Bus;
use crate::error::DmaError;
use serde::{Deserialize, Serialize};

const DMA_BYTES: u16 = 160;
const DMA_TOTAL_CYCLES: Cycles = 640;
const CYCLES_PER_BYTE: Cycles = 4;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OamDma {
    active: bool,
    source_base: u16,
    index: u16,
    cycles_remaining: Cycles,
    cycle_accumulator: Cycles,
}

impl OamDma {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn start(&mut self, source_high: u8, bus: &mut Bus) -> Result<(), DmaError> {
        self.active = true;
        self.source_base = u16::from(source_high) << 8;
        self.index = 0;
        self.cycles_remaining = DMA_TOTAL_CYCLES;
        self.cycle_accumulator = 0;
        bus.set_dma_active(true);
        Ok(())
    }

    pub fn tick(&mut self, cycles: Cycles, bus: &mut Bus) -> Result<(), DmaError> {
        if !self.active {
            return Ok(());
        }

        self.cycle_accumulator = self.cycle_accumulator.wrapping_add(cycles);
        self.cycles_remaining = self.cycles_remaining.saturating_sub(cycles);

        while self.cycle_accumulator >= CYCLES_PER_BYTE && self.index < DMA_BYTES {
            let source_addr = self
                .source_base
                .checked_add(self.index)
                .ok_or(DmaError::SourceOverflow)?;
            let value = bus.read_dma_source(source_addr)?;
            bus.write_oam_dma(self.index, value);
            self.index = self.index.wrapping_add(1);
            self.cycle_accumulator -= CYCLES_PER_BYTE;
        }

        if self.index >= DMA_BYTES {
            self.active = false;
            self.cycles_remaining = 0;
            self.cycle_accumulator = 0;
            bus.set_dma_active(false);
        }

        Ok(())
    }

    #[must_use]
    pub fn is_active(&self) -> bool {
        self.active
    }
}
