use core::num::NonZeroU8;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
pub struct Count(NonZeroU8);

impl Count {
    pub const MAX: u8 = 99;

    pub(crate) const ONE: Self = Self(NonZeroU8::MIN);

    #[must_use]
    pub fn new(count: u8) -> Option<Self> {
        NonZeroU8::new(count)
            .filter(|count| count.get() <= Self::MAX)
            .map(Self)
    }

    #[must_use]
    pub fn get(self) -> u8 {
        self.0.get()
    }
}
