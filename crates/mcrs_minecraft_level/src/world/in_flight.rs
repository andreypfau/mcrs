use bevy_ecs::entity::Entity;
use bevy_ecs::resource::Resource;
use rustc_hash::FxHashMap;

use crate::session::{MoveId, PlayerSession};

/// Default tick budget before a move without a `Spawned` ack is rolled back.
/// At 20 TPS this is 5 seconds.
pub const MOVE_TIMEOUT_TICKS: u32 = 100;

/// The ids of the moves a dimension starts. The source allocates the id at move-out so it can
/// stamp its in-transit entity with the id the host echoes back on confirm or rollback.
#[derive(Resource, Debug)]
pub struct MoveIds {
    source: Entity,
    next: u64,
}

impl MoveIds {
    pub fn new(source: Entity) -> Self {
        Self { source, next: 0 }
    }

    pub fn allocate(&mut self) -> MoveId {
        self.next += 1;
        MoveId {
            source: self.source,
            seq: self.next,
        }
    }
}

pub struct InFlightEntry {
    /// Label entity of the source sub-app.
    pub source_dim: Entity,
    /// In-source-dim entity that has the `InTransit` marker.
    pub hidden_entity: Entity,
    /// Present for player moves; absent for non-player moves.
    pub session: Option<PlayerSession>,
    pub ticks_elapsed: u32,
}

#[derive(Resource)]
pub struct InFlightMoves {
    entries: FxHashMap<MoveId, InFlightEntry>,
    /// Ticks-elapsed threshold at which an entry is considered timed out.
    /// Defaults to `MOVE_TIMEOUT_TICKS`; tests set this to a small value for
    /// deterministic fast-timeout behaviour.
    pub timeout_ticks: u32,
}

impl Default for InFlightMoves {
    fn default() -> Self {
        Self {
            entries: FxHashMap::default(),
            timeout_ticks: MOVE_TIMEOUT_TICKS,
        }
    }
}

impl InFlightMoves {
    /// Stores an entry under the id the source stamped on its in-transit entity.
    pub fn insert(&mut self, id: MoveId, entry: InFlightEntry) {
        self.entries.insert(id, entry);
    }

    pub fn get(&self, id: MoveId) -> Option<&InFlightEntry> {
        self.entries.get(&id)
    }

    pub fn remove(&mut self, id: MoveId) -> Option<InFlightEntry> {
        self.entries.remove(&id)
    }

    /// Advance every entry by one tick and return the ids that reached the
    /// timeout threshold.  The returned entries are NOT removed — the caller
    /// is responsible for driving the rollback and then calling `remove`.
    pub fn tick_all(&mut self) -> Vec<MoveId> {
        for entry in self.entries.values_mut() {
            entry.ticks_elapsed = entry.ticks_elapsed.saturating_add(1);
        }
        let threshold = self.timeout_ticks;
        self.entries
            .iter()
            .filter(|(_, e)| e.ticks_elapsed >= threshold)
            .map(|(id, _)| *id)
            .collect()
    }
}
