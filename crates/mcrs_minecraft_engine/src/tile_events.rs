//! The tile deltas that a dimension publishes at the end of each tick: one event for each
//! change of a player, by tile and by player.

use std::mem;

use bevy_ecs::resource::Resource;
use mcrs_engine_muxer::{EntityEvent, EntityId, SubjectEvent, TileDelta, TileDeltas, TileKey};
use mcrs_minecraft_replication::PlayerData;

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
        let mut events = mem::take(&mut self.events);
        // A stable sort, so the events of one subject in one tile stay in the order they came.
        events.sort_by_key(|event| (event.tile, event.event.subject));
        let mut tiles: Vec<TileDelta<PlayerData>> = Vec::new();
        for TileEvent { tile, event } in events {
            match tiles.last_mut() {
                Some(delta) if delta.tile == tile => delta.events.push(event),
                _ => tiles.push(TileDelta {
                    tile,
                    events: vec![event],
                }),
            }
        }
        TileDeltas { tick, tiles }
    }
}
