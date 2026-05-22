//! Minecraft-tier AoI Components. The per-player source of truth for
//! chunk subscription (mirrored into each subscribed chunk's
//! `mcrs_engine::aoi::PlayerObservers`) and the derived per-player
//! cache of other in-radius players.

use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use mcrs_engine::geometry::ColumnPos;
use rustc_hash::FxHashSet;
use smallvec::SmallVec;

/// Per-player chunk subscription set. Source of truth that
/// `aoi::update_own_pov` mirrors into each subscribed chunk's
/// `mcrs_engine::aoi::PlayerObservers` Component. Adding a `ColumnPos`
/// here adds this player to that chunk's observer set; removal mirrors
/// the same way.
#[derive(Component, Default, Debug)]
pub struct ChunkSubscriptionSet(pub FxHashSet<ColumnPos>);

/// Derived cache: the set of OTHER players inside this player's
/// tracking radius (~80 blocks, narrower than the chunk-subscription
/// radius). Computed by `aoi::update_tracked_by` from the
/// `PlayerObservers` sets of chunks neighbouring the player's current
/// chunk, filtered by precise entity distance. May be one tick stale
/// relative to the source of truth; that one-tick latency is the
/// Folia-style asymmetry baked into the schedule placement, not a
/// double buffer. Eviction is piggybacked onto the next
/// `update_tracked_by` re-derivation.
#[derive(Component, Default, Debug)]
pub struct TrackedBy(pub SmallVec<[Entity; 32]>);
