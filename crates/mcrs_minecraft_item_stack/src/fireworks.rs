use alloc::vec::Vec;

use crate::FireworkExplosion;

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
pub struct Fireworks {
    pub flight_duration: u8,
    pub explosions: Vec<FireworkExplosion>,
}

impl Fireworks {
    pub const MAX_EXPLOSIONS: usize = 256;
}
