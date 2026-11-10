/// When a decision of issue tick `T` applies.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AiDelay {
    /// At the first tick `T' ≥ T + k` that starts after the decision is ready; the tick waits at
    /// `T + max_lag` for a decision that is not ready. The production mode, with `k` = 1.
    AtLeast(u8),
    /// Always at `T + k`; the tick waits for the decisions of `T' - k`. Two runs from one world
    /// are equal tick by tick, for each thread count.
    Fixed(u8),
}

impl AiDelay {
    #[must_use]
    pub const fn ticks(&self) -> u8 {
        match self {
            Self::AtLeast(ticks) | Self::Fixed(ticks) => *ticks,
        }
    }
}
