use core::num::NonZeroU8;

/// How many items a stack holds. Never zero: a stack with no items does not exist.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
pub struct Count(NonZeroU8);

impl Count {
    /// Gives `None` for a count of zero.
    #[must_use]
    pub fn new(count: u8) -> Option<Self> {
        NonZeroU8::new(count).map(Self)
    }

    #[must_use]
    pub fn get(self) -> u8 {
        self.0.get()
    }
}
