//! Engine-tier AoI Components. Universal primitives shared across
//! current and future trackers.

use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use smallvec::SmallVec;

/// Per-chunk observer set: the players whose chunk subscription set
/// includes this chunk's column. Universal AoI primitive — future
/// `MobTracker` / `ItemTracker` / `ProjectileTracker` route observer
/// lookups through the same Component. A mob-farm with 50 mobs in one
/// chunk pays one observer set, not 50.
///
/// Inline capacity 16 sized for typical VD12 occupancy plus the
/// mini-game profile. Spill is correctness-safe; pathological
/// PvP-arena clustering may spill but Tracy telemetry surfaces it.
#[derive(Component, Debug, Default)]
pub struct PlayerObservers(pub SmallVec<[Entity; 16]>);
