use mcrs_minecraft_text::ItemHover;
use mcrs_minecraft_trim::TrimMaterial;
use mcrs_minecraft_trim::TrimPattern;

use crate::RegistryValue;
use crate::TrimMaterialId;
use crate::TrimPatternId;

#[derive(Clone, Debug, PartialEq, Hash)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
pub struct Trim {
    pub material: RegistryValue<TrimMaterialId, TrimMaterial<ItemHover>>,
    pub pattern: RegistryValue<TrimPatternId, TrimPattern<ItemHover>>,
}
