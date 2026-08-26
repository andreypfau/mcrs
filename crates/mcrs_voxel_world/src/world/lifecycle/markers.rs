use bevy_ecs::prelude::Component;

#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct ChunkLoaded;

#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct ChunkGenerating;

#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct ChunkLoading;

#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct ChunkUnloading;

#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct ChunkUnloaded;
