use std::collections::BTreeMap;

use bevy_ecs::entity::Entity;
use bevy_ecs::resource::Resource;

use crate::player_roster::RosterRow;

/// The players of every dimension at the end of the last tick, as each world reported them, by
/// the label of the world. A world that runs commands gets the players of the others.
#[derive(Resource, Default)]
pub struct PlayerRoster(BTreeMap<Entity, Vec<RosterRow>>);

impl PlayerRoster {
    pub(crate) fn report(&mut self, dim: Entity, rows: Vec<RosterRow>) {
        self.0.insert(dim, rows);
    }

    pub(crate) fn forget(&mut self, dim: Entity) {
        self.0.remove(&dim);
    }

    /// The players of every world but `dim`.
    pub(crate) fn others(&self, dim: Entity) -> Vec<RosterRow> {
        self.0
            .iter()
            .filter(|(reported, _)| **reported != dim)
            .flat_map(|(_, rows)| rows.iter().cloned())
            .collect()
    }
}
