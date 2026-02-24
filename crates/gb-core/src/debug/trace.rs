#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugEvent {
    CpuStep { pc: u16, opcode: u8 },
    MemRead { addr: u16, value: u8 },
    MemWrite { addr: u16, value: u8 },
    InterruptServiced { vector: u16 },
}

pub trait DebugSink {
    fn on_event(&mut self, event: &DebugEvent);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugBreakReason {
    PcBreakpoint { pc: u16 },
    OpcodeBreakpoint { opcode: u8, pc: u16 },
    WatchpointRead { addr: u16, value: u8 },
    WatchpointWrite { addr: u16, value: u8 },
}

#[derive(Default)]
pub struct VecDebugSink {
    events: Vec<DebugEvent>,
}

impl VecDebugSink {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn events(&self) -> &[DebugEvent] {
        &self.events
    }

    #[must_use]
    pub fn take_events(&mut self) -> Vec<DebugEvent> {
        core::mem::take(&mut self.events)
    }
}

impl DebugSink for VecDebugSink {
    fn on_event(&mut self, event: &DebugEvent) {
        self.events.push(*event);
    }
}
