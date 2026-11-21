use bevy_ecs::resource::Resource;
use glam::DVec3;
use mcrs_minecraft_entity_rows::Keyed;
use mcrs_minecraft_entity_rows::TileQueues;

#[derive(Resource, Debug, Default)]
pub struct RowWrites(pub TileQueues<Keyed<DVec3>>);
