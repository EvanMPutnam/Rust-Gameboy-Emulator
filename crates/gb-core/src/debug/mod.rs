mod breakpoint;
mod trace;
mod watchpoint;

pub use breakpoint::Breakpoints;
pub use trace::{DebugBreakReason, DebugEvent, DebugSink, VecDebugSink};
pub use watchpoint::{Watchpoint, WatchpointAccess};

#[derive(Default)]
pub struct Debugger {
    enabled: bool,
    breakpoints: Breakpoints,
    watchpoints: Vec<Watchpoint>,
    sink: Option<Box<dyn DebugSink>>,
    pending_break: Option<DebugBreakReason>,
}

impl Debugger {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.pending_break = None;
        }
    }

    #[must_use]
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn set_sink(&mut self, sink: Option<Box<dyn DebugSink>>) {
        self.sink = sink;
    }

    pub fn add_pc_breakpoint(&mut self, pc: u16) {
        self.breakpoints.add_pc(pc);
    }

    pub fn add_opcode_breakpoint(&mut self, opcode: u8) {
        self.breakpoints.add_opcode(opcode);
    }

    pub fn clear_breakpoints(&mut self) {
        self.breakpoints.clear();
    }

    pub fn add_watchpoint(&mut self, watchpoint: Watchpoint) {
        if !self.watchpoints.contains(&watchpoint) {
            self.watchpoints.push(watchpoint);
        }
    }

    pub fn clear_watchpoints(&mut self) {
        self.watchpoints.clear();
    }

    #[must_use]
    pub fn watchpoints(&self) -> &[Watchpoint] {
        &self.watchpoints
    }

    pub fn clear_break(&mut self) {
        self.pending_break = None;
    }

    #[must_use]
    pub fn take_break(&mut self) -> Option<DebugBreakReason> {
        self.pending_break.take()
    }

    #[must_use]
    pub fn has_break(&self) -> bool {
        self.pending_break.is_some()
    }

    #[must_use]
    pub fn memory_trace_enabled(&self) -> bool {
        self.enabled && (!self.watchpoints.is_empty() || self.sink.is_some())
    }

    pub fn on_instruction(&mut self, pc: u16, opcode: u8) {
        if !self.enabled {
            return;
        }

        self.emit(DebugEvent::CpuStep { pc, opcode });

        if self.breakpoints.is_pc_match(pc) {
            self.pending_break = Some(DebugBreakReason::PcBreakpoint { pc });
        } else if self.breakpoints.is_opcode_match(opcode) {
            self.pending_break = Some(DebugBreakReason::OpcodeBreakpoint { opcode, pc });
        }
    }

    pub fn on_event(&mut self, event: DebugEvent) {
        if !self.enabled {
            return;
        }

        self.emit(event);
        if self.pending_break.is_some() {
            return;
        }

        if self
            .watchpoints
            .iter()
            .any(|watchpoint| watchpoint.matches(&event))
        {
            self.pending_break = match event {
                DebugEvent::MemRead { addr, value } => {
                    Some(DebugBreakReason::WatchpointRead { addr, value })
                }
                DebugEvent::MemWrite { addr, value } => {
                    Some(DebugBreakReason::WatchpointWrite { addr, value })
                }
                DebugEvent::CpuStep { .. } | DebugEvent::InterruptServiced { .. } => None,
            };
        }
    }

    fn emit(&mut self, event: DebugEvent) {
        if let Some(sink) = &mut self.sink {
            sink.on_event(&event);
        }
    }
}
