use crate::light_storage::LightStorage;

#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
pub struct BlockLight(pub LightStorage);

#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
pub struct SkyLight(pub LightStorage);

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum LightLayer {
    Block,
    Sky,
}
