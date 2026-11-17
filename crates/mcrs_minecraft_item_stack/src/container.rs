use alloc::vec::Vec;

use crate::ItemStack;

#[derive(Clone, Debug, Default, PartialEq, Hash)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
pub struct Container {
    slots: Vec<Option<ItemStack>>,
}

impl Container {
    pub const MAX_SLOTS: usize = 256;

    #[must_use]
    pub fn new(mut slots: Vec<Option<ItemStack>>) -> Option<Self> {
        while slots.last().is_some_and(Option::is_none) {
            slots.pop();
        }
        (slots.len() <= Self::MAX_SLOTS).then_some(Self { slots })
    }

    #[must_use]
    pub fn slots(&self) -> &[Option<ItemStack>] {
        &self.slots
    }
}
