use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::mem;

use bevy_ecs::resource::Resource;
use bevy_tasks::TaskPool;
use mcrs_engine_muxer::EntityId;
use mcrs_engine_muxer::TileKey;
use mcrs_minecraft_physics::BodyGrid;
use mcrs_minecraft_physics::EntityState;
use mcrs_minecraft_physics::GridRow;
use mcrs_minecraft_physics::MotionRow;
use mcrs_minecraft_physics::Row;
use mcrs_minecraft_physics::TileRows;

use crate::front_grid::grid_row;
use crate::front_grid::sort_bodies;
use crate::tile_at::tile_at;

/// The rows of the server entities of a dimension world, one segment per tile.
#[derive(Resource, Debug, Default)]
pub struct EntityRows {
    pub(crate) tiles: BTreeMap<TileKey, TileRows>,
    /// The tiles whose rows changed after the pass wrote their grid; the freeze writes it again.
    pub(crate) dirty: BTreeSet<TileKey>,
    /// The tiles whose blocks changed since the freeze. A body, a move or a write wakes no other
    /// row: the push runs in each tick, and a write wakes its target.
    pub(crate) stirred: BTreeSet<TileKey>,
    /// The stirred tiles of the last freeze: a resting row in them or next to them runs a full
    /// tick in this pass.
    pub(crate) woken: BTreeSet<TileKey>,
    /// The rows that the pass moved out of their tile, with that tile.
    pub(crate) leaving: Vec<Leaving>,
    /// The largest half width of a body that the grid ever held.
    pub(crate) reach: f32,
}

/// A row that left the tile `from` in the pass.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Leaving {
    pub(crate) from: TileKey,
    pub(crate) id: EntityId,
}

impl EntityRows {
    /// Adds a row with `id` in the tile of its position.
    pub fn spawn(&mut self, id: EntityId, state: EntityState, motion: MotionRow) {
        let tile = tile_at(state.body.position);
        self.tiles
            .entry(tile)
            .or_default()
            .insert_row(Row { state, motion, id });
        self.dirty.insert(tile);
    }

    /// The rows of each tile, in tile order.
    pub fn tiles_mut(&mut self) -> impl Iterator<Item = (TileKey, &mut TileRows)> {
        self.tiles.iter_mut().map(|(&tile, rows)| (tile, rows))
    }

    /// Marks that a block changed in `tile`, so the rows that rest near it wake.
    pub fn stir(&mut self, tile: TileKey) {
        self.stirred.insert(tile);
    }

    /// Moves each row that the pass moved out of its tile into the tile of its position.
    pub fn move_rows(&mut self) {
        for Leaving { from, id } in mem::take(&mut self.leaving) {
            let Some(row) = self
                .tiles
                .get_mut(&from)
                .and_then(|rows| rows.despawn_row(id))
            else {
                continue;
            };
            let to = tile_at(row.state.body.position);
            self.tiles.entry(to).or_default().insert_row(row);
            self.dirty.extend([from, to]);
        }
    }

    /// The freeze: writes the grid of each dirty tile from its rows, in parallel over the tiles,
    /// makes the back buffers the front, and keeps the stirred tiles for the wakes of the next
    /// pass.
    pub fn freeze(&mut self, pool: &TaskPool, grid: &mut BodyGrid) {
        let dirty = mem::take(&mut self.dirty);
        let rebuilt: Vec<(TileKey, Vec<GridRow>)> = pool.scope(|scope| {
            for (&tile, rows) in &mut self.tiles {
                if dirty.contains(&tile) {
                    scope.spawn(async move { (tile, bodies(tile, rows)) });
                }
            }
        });
        for (tile, bodies) in rebuilt {
            self.install(grid, tile, bodies);
        }
        for &tile in &dirty {
            if !self.tiles.contains_key(&tile) {
                self.install(grid, tile, Vec::new());
            }
        }
        grid.swap();
        self.woken = mem::take(&mut self.stirred);
    }

    pub(crate) fn install(&mut self, grid: &mut BodyGrid, tile: TileKey, bodies: Vec<GridRow>) {
        for body in &bodies {
            self.reach = self.reach.max(body.half_width);
        }
        *grid.back_mut(tile) = bodies;
    }
}

/// The bodies of the rows of `tile`, in the order of its grid.
fn bodies(tile: TileKey, rows: &mut TileRows) -> Vec<GridRow> {
    let mut bodies: Vec<GridRow> = rows
        .rows_mut()
        .filter_map(|row| grid_row(row.state, row.motion, row.id))
        .collect();
    sort_bodies(tile, &mut bodies);
    bodies
}

/// True when a resting row of `tile` runs a full tick: something changed in the tile or in a tile
/// next to it.
pub(crate) fn wakes(woken: &BTreeSet<TileKey>, tile: TileKey) -> bool {
    (-1..=1).any(|z| {
        (-1..=1).any(|x| {
            woken.contains(&TileKey::new(
                tile.x.saturating_add(x),
                0,
                tile.z.saturating_add(z),
            ))
        })
    })
}
