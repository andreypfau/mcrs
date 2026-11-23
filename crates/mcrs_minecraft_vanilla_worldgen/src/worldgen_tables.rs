use bevy_ecs::prelude::Resource;
use mcrs_minecraft_worldgen_generator::world_tables::WorldTables;

#[derive(Resource)]
pub struct WorldgenTables(pub WorldTables);
