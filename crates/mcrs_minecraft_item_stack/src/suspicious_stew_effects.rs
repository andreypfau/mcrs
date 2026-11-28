use alloc::vec::Vec;

use crate::StewEffect;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
pub struct SuspiciousStewEffects(pub Vec<StewEffect>);
