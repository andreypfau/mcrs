use crate::world::entity::player::HostAnchor;
use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::world::World;
use smallvec::SmallVec;

/// Derived cache: the set of OTHER players inside this player's
/// tracking radius (~80 blocks, narrower than the view). Computed by
/// `aoi::update_tracked_by` from the `PlayerObservers` sets of columns
/// neighbouring the player's current column, filtered by precise entity
/// distance. May be one tick stale relative to the source of truth; that
/// one-tick latency is the Folia-style asymmetry baked into the schedule
/// placement, not a double buffer. Eviction is piggybacked onto the next
/// `update_tracked_by` re-derivation.
#[derive(Component, Default, Debug)]
pub struct TrackedBy(pub SmallVec<[Entity; 32]>);

impl TrackedBy {
    pub fn anchors(world: &World, tracked: Entity) -> SmallVec<[Entity; 8]> {
        world
            .get::<TrackedBy>(tracked)
            .into_iter()
            .flat_map(|tracked| tracked.0.iter().copied())
            .filter_map(|viewer| world.get::<HostAnchor>(viewer).map(|anchor| anchor.0))
            .collect()
    }
}
