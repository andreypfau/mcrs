//! The tile deltas that a dimension publishes at the end of each tick: one event for each
//! change of a player, by tile and by player.

use std::mem;

use bevy_ecs::resource::Resource;
use bevy_tasks::ComputeTaskPool;
use mcrs_engine_muxer::{EntityEvent, EntityId, SubjectEvent, TileDelta, TileDeltas, TileKey};
use mcrs_minecraft_replication::ReplicationData;
use rustc_hash::FxHashMap;

/// Above this count of events, [`TileEvents::take`] sorts the tiles on the compute pool.
const PARALLEL_SORT: usize = 4096;

/// The events of the current tick, by tile, each tile in the order its events happened.
#[derive(Resource, Default, Debug)]
pub struct TileEvents {
    tick: u64,
    tiles: FxHashMap<TileKey, Vec<SubjectEvent<ReplicationData>>>,
    count: usize,
}

impl TileEvents {
    pub fn push(&mut self, tile: TileKey, subject: EntityId, event: EntityEvent<ReplicationData>) {
        self.tiles
            .entry(tile)
            .or_default()
            .push(SubjectEvent { subject, event });
        self.count = self.count.saturating_add(1);
    }

    /// Adds `events` to `tile` after the events that the tile holds, in their order, as one
    /// [`Self::push`] for each would.
    pub fn append(&mut self, tile: TileKey, mut events: Vec<SubjectEvent<ReplicationData>>) {
        self.count = self.count.saturating_add(events.len());
        let held = self.tiles.entry(tile).or_default();
        if held.is_empty() {
            *held = events;
        } else {
            held.append(&mut events);
        }
    }

    /// The tile deltas of the tick, sorted by tile and by subject; the events of one subject
    /// keep their order. The next tick starts.
    pub fn take(&mut self) -> TileDeltas<ReplicationData> {
        let tick = self.tick;
        self.tick = self.tick.wrapping_add(1);
        let count = mem::take(&mut self.count);
        let mut tiles: Vec<TileDelta<ReplicationData>> = self
            .tiles
            .drain()
            .map(|(tile, events)| TileDelta { tile, events })
            .collect();
        tiles.sort_unstable_by_key(|delta| delta.tile);
        // A stable sort, so the events of one subject stay in the order they came.
        let sort = |delta: &mut TileDelta<ReplicationData>| {
            delta.events.sort_by_key(|event| event.subject);
        };
        match ComputeTaskPool::try_get() {
            Some(pool) if count > PARALLEL_SORT => {
                pool.scope(|scope| {
                    for delta in &mut tiles {
                        scope.spawn(async move { sort(delta) });
                    }
                });
            }
            _ => {
                for delta in &mut tiles {
                    sort(delta);
                }
            }
        }
        TileDeltas { tick, tiles }
    }
}
