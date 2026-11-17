use std::mem;

use bevy_ecs::resource::Resource;
use bevy_tasks::ComputeTaskPool;
use mcrs_engine_muxer::{EntityEvent, EntityId, SubjectEvent, TileDelta, TileDeltas, TileKey};
use mcrs_minecraft_replication::ReplicationData;
use rustc_hash::FxHashMap;

const PARALLEL_SORT: usize = 4096;

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

    pub fn append(&mut self, tile: TileKey, mut events: Vec<SubjectEvent<ReplicationData>>) {
        self.count = self.count.saturating_add(events.len());
        let held = self.tiles.entry(tile).or_default();
        if held.is_empty() {
            *held = events;
        } else {
            held.append(&mut events);
        }
    }

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
