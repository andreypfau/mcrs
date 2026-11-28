use core::num::NonZeroU8;

use crate::MaxStackSize;

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
    pub fn capacity(max: Option<&MaxStackSize>) -> u8 {
        max.map_or(1, |max| {
            u8::try_from(max.0.clamp(1, i32::from(Self::MAX))).unwrap_or(1)
        })
    }

    #[must_use]
    pub fn get(self) -> u8 {
        self.0.get()
    }
}
