use bevy_ecs::prelude::Component;

/// Archetype filter for `Added<ChunkLoaded>` readers: `Added` is change detection,
/// so on its own it walks every loaded section rather than the ones that landed.
/// Held for the tick a section lands in and the whole tick after it.
#[derive(Component, Default)]
#[component(storage = "SparseSet")]
pub struct ChunkFresh;

#[derive(Component)]
#[component(storage = "SparseSet")]
#[require(ChunkFresh)]
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
