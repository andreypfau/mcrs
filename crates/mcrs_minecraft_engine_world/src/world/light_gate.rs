use bevy_ecs::entity::Entity;
use bevy_ecs::message::Message;
use bevy_ecs::resource::Resource;
use bevy_ecs::schedule::SystemSet;
use mcrs_voxel::ColumnPos;

#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct LightGated;

#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct SectionRelit {
    pub section: Entity,
    pub block: bool,
    pub sky: bool,
}

#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColumnAwaitsLight(pub ColumnPos);

#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColumnLightSettled(pub ColumnPos);

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ColumnLightGate;
