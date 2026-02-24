use crate::Cycles;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventKind {
    TimerTick,
    InterruptSample,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScheduledEvent {
    pub at: Cycles,
    pub kind: EventKind,
}
