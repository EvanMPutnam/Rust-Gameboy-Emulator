use crate::Cycles;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Clock {
    now: Cycles,
}

impl Clock {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn tick(&mut self, delta: Cycles) {
        self.now = self.now.wrapping_add(delta);
    }

    #[must_use]
    pub fn now(&self) -> Cycles {
        self.now
    }
}
