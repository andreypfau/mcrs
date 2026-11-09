//! The tile deltas that a dimension publishes at the end of each tick: one event for each
//! change of a player, by tile and by player.

use std::mem;

use bevy_ecs::resource::Resource;
use mcrs_engine_muxer::{EntityEvent, EntityId, SubjectEvent, TileDelta, TileDeltas, TileKey};
use mcrs_minecraft_replication::PlayerData;
use rustc_hash::FxHashMap;

/// The events of the current tick, in the order they happened.
#[derive(Resource, Default, Debug)]
pub struct TileEvents {
    tick: u64,
    events: Vec<TileEvent>,
}

#[derive(Debug)]
struct TileEvent {
    tile: TileKey,
    event: SubjectEvent<PlayerData>,
}

impl TileEvents {
    pub fn push(&mut self, tile: TileKey, subject: EntityId, event: EntityEvent<PlayerData>) {
        self.events.push(TileEvent {
            tile,
            event: SubjectEvent { subject, event },
        });
    }

    /// The tile deltas of the tick, sorted by tile and by subject; the events of one subject
    /// keep their order. The next tick starts.
    pub fn take(&mut self) -> TileDeltas<PlayerData> {
        let tick = self.tick;
        self.tick = self.tick.wrapping_add(1);
        let mut slots: FxHashMap<TileKey, usize> = FxHashMap::default();
        let mut tiles: Vec<TileDelta<PlayerData>> = Vec::new();
        for TileEvent { tile, event } in mem::take(&mut self.events) {
            let slot = *slots.entry(tile).or_insert_with(|| {
                tiles.push(TileDelta {
                    tile,
                    events: Vec::new(),
                });
                tiles.len() - 1
            });
            if let Some(delta) = tiles.get_mut(slot) {
                delta.events.push(event);
            }
        }
        tiles.sort_unstable_by_key(|delta| delta.tile);
        for delta in &mut tiles {
            // A stable sort, so the events of one subject stay in the order they came.
            delta.events.sort_by_key(|event| event.subject);
        }
        TileDeltas { tick, tiles }
    }
}
