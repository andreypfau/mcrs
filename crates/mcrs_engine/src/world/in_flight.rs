use bevy_ecs::entity::Entity;
use bevy_ecs::resource::Resource;
use rustc_hash::FxHashMap;

use crate::session::{MoveId, PlayerSession};

/// Default tick budget before a move without a `Spawned` ack is rolled back.
/// At 20 TPS this is 5 seconds.
pub const MOVE_TIMEOUT_TICKS: u32 = 100;

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
    next_id: u64,
    /// Ticks-elapsed threshold at which an entry is considered timed out.
    /// Defaults to `MOVE_TIMEOUT_TICKS`; tests set this to a small value for
    /// deterministic fast-timeout behaviour.
    pub timeout_ticks: u32,
}

impl Default for InFlightMoves {
    fn default() -> Self {
        Self {
            entries: FxHashMap::default(),
            next_id: 0,
            timeout_ticks: MOVE_TIMEOUT_TICKS,
        }
    }
}

impl InFlightMoves {
    /// Allocate a fresh `MoveId` (never 0) and store the entry.
    pub fn alloc(&mut self, entry: InFlightEntry) -> MoveId {
        self.next_id = self.next_id.checked_add(1).expect("MoveId counter exhausted");
        let id = MoveId(self.next_id);
        self.entries.insert(id, entry);
        id
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
