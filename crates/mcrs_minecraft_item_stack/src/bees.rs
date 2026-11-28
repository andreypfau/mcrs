use alloc::vec::Vec;

use crate::BeeOccupant;

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
pub struct Bees(pub Vec<BeeOccupant>);
