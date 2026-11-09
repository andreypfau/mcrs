use std::mem;

use bevy_ecs::resource::Resource;
use glam::DVec3;
use mcrs_minecraft_physics::Keyed;
use mcrs_minecraft_physics::Rest;
use mcrs_minecraft_physics::TileQueues;

use crate::entity_rows::EntityRows;

/// The impulses of the last tick, by the tile of the target. The target adds each one to its
/// velocity at the start of the next tick.
#[derive(Resource, Debug, Default)]
pub struct EntityWrites(pub TileQueues<Keyed<DVec3>>);

impl EntityRows {
    /// The merge point: each tile applies the writes to its rows in the order of the target key,
    /// the writer key and the sequence, so the f64 sums do not depend on the tasks that made the
    /// writes. A write whose target left the tile is dropped.
    pub fn apply_writes(&mut self, writes: &mut EntityWrites) {
        let mut queues = mem::take(&mut writes.0);
        for (&tile, rows) in &mut self.tiles {
            let entries = queues.take_in_order(tile);
            if entries.is_empty() {
                continue;
            }
            for entry in entries {
                let Some(row) = rows.rows.row_mut(entry.target_id) else {
                    continue;
                };
                row.state.body.velocity += entry.effect;
                row.state.rest = Rest::Awake;
            }
            self.dirty.insert(tile);
        }
    }
}
