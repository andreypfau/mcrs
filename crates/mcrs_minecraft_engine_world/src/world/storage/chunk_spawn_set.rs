use bevy_ecs::schedule::SystemSet;

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ChunkSpawnSet;
