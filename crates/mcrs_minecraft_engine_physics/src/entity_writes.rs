use bevy_ecs::resource::Resource;
use glam::DVec3;
use mcrs_minecraft_entity_rows::Keyed;
use mcrs_minecraft_entity_rows::TileQueues;

/// The impulses of the last tick, by the tile of the target. The target adds each one to its
/// velocity at the start of the next tick.
#[derive(Resource, Debug, Default)]
pub struct EntityWrites(pub TileQueues<Keyed<DVec3>>);
