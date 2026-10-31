use bevy_ecs::entity::Entity;
use mcrs_minecraft_dimension::Dimension;
use mcrs_minecraft_resource::ResourceKeyBuf;

/// A dimension that runs, as a command sees it.
#[derive(Clone, Debug)]
pub struct DimensionEntry {
    /// The host entity that labels the sub-app of the dimension.
    pub label: Entity,
    /// The identifier of the dimension.
    pub key: ResourceKeyBuf<Dimension>,
    /// The scale of horizontal coordinates in the dimension against the overworld.
    pub coordinate_scale: f64,
}
