use crate::InlineTrimMaterial;
use crate::InlineTrimPattern;
use crate::RegistryValue;
use crate::TrimMaterialId;
use crate::TrimPatternId;

#[derive(Clone, Debug, PartialEq, Hash)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
pub struct Trim {
    pub material: RegistryValue<TrimMaterialId, InlineTrimMaterial>,
    pub pattern: RegistryValue<TrimPatternId, InlineTrimPattern>,
}
