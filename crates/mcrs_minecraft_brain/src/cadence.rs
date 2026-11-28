#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Cadence {
    origin: u64,
    offset: u8,
}

impl Cadence {
    #[must_use]
    pub const fn new(first_step: u64, offset: u8) -> Self {
        Self {
            origin: first_step.saturating_sub(1),
            offset,
        }
    }

    #[must_use]
    pub const fn tick_count(&self, tick: u64) -> u64 {
        tick.saturating_sub(self.origin)
    }

    #[must_use]
    pub const fn is_full_evaluation(&self, tick: u64) -> bool {
        let count = self.tick_count(tick);
        count >= 1 && (count <= 1 || (count + self.offset as u64).is_multiple_of(2))
    }

    #[must_use]
    pub const fn next_evaluation(&self, tick: u64) -> u64 {
        let first = self.origin.saturating_add(1);
        let tick = if tick < first { first } else { tick };
        if self.is_full_evaluation(tick) {
            tick
        } else {
            tick.saturating_add(1)
        }
    }

    #[must_use]
    pub const fn nth_evaluation(&self, tick: u64, n: u32) -> u64 {
        let mut evaluation = self.next_evaluation(tick);
        let mut remaining = if n == 0 { 0 } else { n - 1 };
        while remaining > 0 && self.tick_count(evaluation) < 2 {
            evaluation = self.next_evaluation(evaluation.saturating_add(1));
            remaining -= 1;
        }
        evaluation.saturating_add(2 * remaining as u64)
    }
}
