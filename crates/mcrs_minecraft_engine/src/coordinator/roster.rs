use std::collections::BTreeMap;

use bevy_ecs::resource::Resource;
use mcrs_engine_cell::CellId;
use mcrs_minecraft_engine_world::session::PlayerSession;

use crate::player_roster::RosterRow;

/// The players of every cell at the end of the last tick, as each cell reported them. A cell
/// that runs commands gets the players of the others, those of the other cells of its own
/// dimension too.
#[derive(Resource, Default)]
pub struct PlayerRoster(BTreeMap<CellId, Vec<RosterRow>>);

impl PlayerRoster {
    pub(crate) fn report(&mut self, cell: CellId, rows: Vec<RosterRow>) {
        self.0.insert(cell, rows);
    }

    pub(crate) fn forget(&mut self, cell: CellId) {
        self.0.remove(&cell);
    }

    /// Moves the row of the player of `session` from the cell `from` to the cell `to`, which
    /// holds the player from the next tick on.
    pub(crate) fn move_row(&mut self, session: PlayerSession, from: CellId, to: CellId) {
        let Some(rows) = self.0.get_mut(&from) else {
            return;
        };
        let Some(at) = rows.iter().position(|row| row.session == session) else {
            return;
        };
        let row = rows.remove(at);
        self.0.entry(to).or_default().push(row);
    }

    /// The players of every cell but `cell`.
    pub(crate) fn others(&self, cell: CellId) -> Vec<RosterRow> {
        self.0
            .iter()
            .filter(|(reported, _)| **reported != cell)
            .flat_map(|(_, rows)| rows.iter().cloned())
            .collect()
    }
}
