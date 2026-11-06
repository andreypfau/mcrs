use bevy_ecs::entity::Entity;
use bevy_ecs::message::Message;
use bevy_ecs::resource::Resource;
use bevy_ecs::schedule::SystemSet;
use mcrs_voxel::ColumnPos;

/// Present in a dimension world whose columns wait for their light to settle before they are
/// sent. Without it a column goes out as soon as its sections land, at full sky light.
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct LightGated;

/// A section whose light was written, and which of its two layers was written.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct SectionRelit {
    pub section: Entity,
    pub block: bool,
    pub sky: bool,
}

/// A column whose light the dimension waits on.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColumnAwaitsLight(pub ColumnPos);

/// A column whose light is final: the column and the eight around it are loaded and lit, and the
/// work that reached them before the light stage took the column in is done.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColumnLightSettled(pub ColumnPos);

/// Between a column waiting for its light and the column going out.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ColumnLightGate;
