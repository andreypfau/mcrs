use bevy_ecs::message::Message;
use bevy_ecs::resource::Resource;
use bevy_ecs::schedule::SystemSet;
use mcrs_voxel::ColumnPos;

/// Present in a dimension world whose columns wait for their light to settle before they are
/// sent. Without it a column goes out as soon as its sections land, at full sky light.
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct LightGated;

/// A column whose sections and light have landed, waiting for its light to settle.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColumnAwaitsLight(pub ColumnPos);

/// A column whose light, and the light of the eight around it, has settled.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColumnLightSettled(pub ColumnPos);

/// Between a column waiting for its light and the column going out.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ColumnLightGate;
