use alloc::vec::Vec;

use crate::ItemStack;

#[derive(Clone, Debug, Default, PartialEq, Hash)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
pub struct BundleContents(pub Vec<ItemStack>);
