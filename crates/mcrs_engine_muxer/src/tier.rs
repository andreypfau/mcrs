/// A rate of updates. Tier 0 is the nearest and the fastest, tier 3 the farthest and the slowest.
/// The codec keeps one delta base for each subject and tier, so all viewers of a tier receive the
/// same bytes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Tier(u8);

impl Tier {
    pub const NEAREST: Self = Self(0);
    pub const FARTHEST: Self = Self(3);
    pub const ALL: [Self; 4] = [Self(0), Self(1), Self(2), Self(3)];

    /// The ticks between two moves of a subject in this tier.
    const INTERVALS: [u64; 4] = [2, 4, 10, 20];

    /// The position of the tier in [`Self::ALL`], to index the state of each tier.
    #[must_use]
    pub fn index(self) -> usize {
        usize::from(self.0)
    }

    /// The ticks between two moves of a subject in this tier.
    #[must_use]
    pub(crate) fn interval(self) -> u64 {
        Self::INTERVALS.get(self.index()).copied().unwrap_or(1)
    }

    /// The tier `steps` tiers slower, or the farthest tier.
    #[must_use]
    pub(crate) fn lowered(self, steps: u8) -> Self {
        Self(self.0.saturating_add(steps).min(Self::FARTHEST.0))
    }

    /// The tier one tier faster, or the nearest tier.
    #[must_use]
    pub(crate) fn raised(self) -> Self {
        Self(self.0.saturating_sub(1))
    }
}
