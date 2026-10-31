use crate::light_storage::LightStorage;

/// The block light of one section.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
pub struct BlockLight(pub LightStorage);

/// The sky light of one section.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
pub struct SkyLight(pub LightStorage);

/// One of the two independent light layers.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum LightLayer {
    /// Emitted by blocks.
    Block,
    /// Comes from the sky, down whole vertical runs of unoccluded column.
    Sky,
}
