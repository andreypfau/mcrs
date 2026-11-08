/// The ticks of the full evaluations of one mob. The tick count of the mob is 1 in its first AI
/// step; a tick is a full evaluation when the tick count is 1 or less, or when the tick count plus
/// the phase offset of the birth key is even.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Cadence {
    origin: u64,
    offset: u8,
}

impl Cadence {
    /// `first_step` is the world tick of the first AI step of the mob after its load or summon;
    /// `offset` is the phase offset from 0 to 3.
    #[must_use]
    pub const fn new(first_step: u64, offset: u8) -> Self {
        Self {
            origin: first_step.saturating_sub(1),
            offset,
        }
    }

    /// The tick count of the mob in the world tick `tick`.
    #[must_use]
    pub const fn tick_count(&self, tick: u64) -> u64 {
        tick.saturating_sub(self.origin)
    }

    #[must_use]
    pub const fn is_full_evaluation(&self, tick: u64) -> bool {
        let count = self.tick_count(tick);
        count >= 1 && (count <= 1 || (count + self.offset as u64).is_multiple_of(2))
    }

    /// The first full evaluation at `tick` or later.
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

    /// The full evaluation number `n` from `tick` on, where `tick` itself counts as number 1 when
    /// it is a full evaluation. `n` of 0 gives the same as 1.
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
