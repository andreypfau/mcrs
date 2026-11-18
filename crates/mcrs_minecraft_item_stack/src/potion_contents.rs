use alloc::string::String;
use alloc::vec::Vec;

use crate::EffectInstance;
use crate::PotionId;

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
pub struct PotionContents {
    pub potion: Option<PotionId>,
    pub custom_color: Option<i32>,
    pub custom_effects: Vec<EffectInstance>,
    pub custom_name: Option<String>,
}
