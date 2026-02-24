use gb_core::Cycles;
use std::time::Duration;

const CPU_HZ: u128 = 4_194_304;
const NANOS_PER_SECOND: u128 = 1_000_000_000;

#[derive(Debug, Clone, Copy)]
pub struct RunLoop {
    pending_cycle_units: u128,
    max_cycles_per_tick: Cycles,
}

impl Default for RunLoop {
    fn default() -> Self {
        Self {
            pending_cycle_units: 0,
            max_cycles_per_tick: 70_224,
        }
    }
}

impl RunLoop {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn cycle_budget(&mut self, elapsed: Duration) -> Cycles {
        let elapsed_nanos = elapsed.as_nanos();
        self.pending_cycle_units = self
            .pending_cycle_units
            .wrapping_add(elapsed_nanos.saturating_mul(CPU_HZ));

        let produced_cycles = self.pending_cycle_units / NANOS_PER_SECOND;
        let capped_cycles = produced_cycles.min(u128::from(self.max_cycles_per_tick));
        self.pending_cycle_units = self
            .pending_cycle_units
            .saturating_sub(capped_cycles.saturating_mul(NANOS_PER_SECOND));

        u32::try_from(capped_cycles).unwrap_or(self.max_cycles_per_tick)
    }
}

#[cfg(test)]
mod tests {
    use super::RunLoop;
    use std::time::Duration;

    #[test]
    fn accumulates_fractional_cycle_budget_deterministically() {
        let mut loop_a = RunLoop::new();
        let mut loop_b = RunLoop::new();

        let first_a = loop_a.cycle_budget(Duration::from_micros(500));
        let second_a = loop_a.cycle_budget(Duration::from_micros(500));

        let full_b = loop_b.cycle_budget(Duration::from_micros(1_000));

        assert_eq!(first_a + second_a, full_b);
    }

    #[test]
    fn long_stalls_are_capped_per_tick() {
        let mut runloop = RunLoop::new();
        let budget = runloop.cycle_budget(Duration::from_secs(1));
        assert_eq!(budget, 70_224);
    }
}
