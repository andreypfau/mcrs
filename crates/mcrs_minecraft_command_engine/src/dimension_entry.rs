use bevy_ecs::entity::Entity;
use mcrs_minecraft_dimension::Dimension;
use mcrs_minecraft_resource::ResourceKeyBuf;

#[derive(Clone, Debug)]
pub struct DimensionEntry {
    pub label: Entity,
    pub key: ResourceKeyBuf<Dimension>,
    pub coordinate_scale: f64,
}
