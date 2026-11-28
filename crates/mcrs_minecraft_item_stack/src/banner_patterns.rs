use alloc::vec::Vec;

use crate::BannerLayer;

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
pub struct BannerPatterns(pub Vec<BannerLayer>);
