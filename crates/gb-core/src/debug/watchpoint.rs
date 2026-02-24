use crate::debug::trace::DebugEvent;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchpointAccess {
    Read,
    Write,
    ReadWrite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Watchpoint {
    pub addr: u16,
    pub access: WatchpointAccess,
}

impl Watchpoint {
    #[must_use]
    pub fn read(addr: u16) -> Self {
        Self {
            addr,
            access: WatchpointAccess::Read,
        }
    }

    #[must_use]
    pub fn write(addr: u16) -> Self {
        Self {
            addr,
            access: WatchpointAccess::Write,
        }
    }

    #[must_use]
    pub fn read_write(addr: u16) -> Self {
        Self {
            addr,
            access: WatchpointAccess::ReadWrite,
        }
    }

    #[must_use]
    pub fn matches(self, event: &DebugEvent) -> bool {
        match event {
            DebugEvent::MemRead { addr, .. } => {
                *addr == self.addr
                    && matches!(
                        self.access,
                        WatchpointAccess::Read | WatchpointAccess::ReadWrite
                    )
            }
            DebugEvent::MemWrite { addr, .. } => {
                *addr == self.addr
                    && matches!(
                        self.access,
                        WatchpointAccess::Write | WatchpointAccess::ReadWrite
                    )
            }
            DebugEvent::CpuStep { .. } | DebugEvent::InterruptServiced { .. } => false,
        }
    }
}
