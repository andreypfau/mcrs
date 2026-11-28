use alloc::vec::Vec;

use crate::FireworkShape;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
pub struct FireworkExplosion {
    pub shape: FireworkShape,
    pub colors: Vec<i32>,
    pub fade_colors: Vec<i32>,
    pub has_trail: bool,
    pub has_twinkle: bool,
}
