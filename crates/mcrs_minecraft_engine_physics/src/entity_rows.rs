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
use mcrs_minecraft_physics::TileBodies;
use mcrs_minecraft_physics::TileRows;

use crate::front_grid::grid_row;
use crate::front_grid::sort_bodies;
use crate::mob_intent::MobIntent;
use crate::mob_life::Placed;
use crate::mob_moved::Crossing;
use crate::mob_moved::MobMoved;
use crate::tile_at::tile_at;

/// The rows of the server entities of a dimension world, one segment per tile.
#[derive(Resource, Debug, Default)]
pub struct EntityRows {
    pub(crate) tiles: BTreeMap<TileKey, Tile>,
    /// The tiles whose rows changed after the pass wrote their grid; the freeze writes it again.
    pub(crate) dirty: BTreeSet<TileKey>,
    /// The tiles whose back buffer the pass wrote; the freeze makes it their front.
    pub(crate) written: BTreeSet<TileKey>,
    /// The tiles whose blocks changed since the freeze. A body, a move or a write wakes no other
    /// row: the push runs in each tick, and a write wakes its target.
    pub(crate) stirred: BTreeSet<TileKey>,
    /// The stirred tiles of the last freeze: a resting row in them or next to them runs a full
    /// tick in this pass.
    pub(crate) woken: BTreeSet<TileKey>,
    /// The rows that the pass moved out of their tile, with that tile.
    pub(crate) leaving: Vec<Leaving>,
    /// The living rows whose visible state changed in the last pass.
    pub(crate) moved: Vec<MobMoved>,
    /// The rows that the last tile moves took to another tile.
    pub(crate) crossed: Vec<Crossing>,
    /// The living rows that left the world in the last pass at the end of their death.
    pub(crate) removed: Vec<Placed>,
    /// The largest half width of a body that the grid ever held.
    pub(crate) reach: f32,
}

/// The rows of one tile and the back buffer of its grid. The task of the tile writes the buffer,
/// so the bodies never pass through one thread.
#[derive(Debug, Default)]
pub(crate) struct Tile {
    pub(crate) rows: TileRows,
    pub(crate) back: TileBodies,
    /// The intents that the next pass applies to the rows of the tile before their step.
    pub(crate) intents: Vec<MobIntent>,
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
            .rows
            .insert_row(Row { state, motion, id });
        self.dirty.insert(tile);
    }

    /// Queues `intent` for the row of its target in `tile`: the next pass applies it before the
    /// step of the row. An intent whose target is not in the tile then is dropped.
    pub fn steer(&mut self, tile: TileKey, intent: MobIntent) {
        self.tiles.entry(tile).or_default().intents.push(intent);
    }

    /// Queues each of `intents` as [`Self::steer`] does, with one lookup of the tile.
    pub fn steer_all(&mut self, tile: TileKey, intents: impl IntoIterator<Item = MobIntent>) {
        self.tiles.entry(tile).or_default().intents.extend(intents);
    }

    /// Removes the row `id` from `tile` and gives it; `None` when the tile has no such row.
    pub fn despawn(&mut self, tile: TileKey, id: EntityId) -> Option<Row> {
        let row = self.tiles.get_mut(&tile)?.rows.despawn_row(id)?;
        self.dirty.insert(tile);
        Some(row)
    }

    /// The id and the state of each living row, with its tile.
    pub fn living_rows(&self) -> impl Iterator<Item = (Placed, &EntityState)> {
        self.tiles.iter().flat_map(|(&tile, entry)| {
            let rows = &entry.rows;
            rows.ids()
                .iter()
                .zip(rows.states())
                .filter(|(_, state)| state.living.is_some())
                .map(move |(&id, state)| (Placed { tile, id }, state))
        })
    }

    /// The state of the row `at`; `None` when the tile has no such row.
    #[must_use]
    pub fn state_of(&self, at: Placed) -> Option<&EntityState> {
        let rows = &self.tiles.get(&at.tile)?.rows;
        let row = rows.ids().iter().position(|&held| held == at.id)?;
        rows.states().get(row)
    }

    /// The state of the row `at`, open to a writer outside the pass; `None` when the tile has no
    /// such row. The tile writes its grid again at the next freeze.
    pub fn state_mut(&mut self, at: Placed) -> Option<&mut EntityState> {
        let row = self.tiles.get_mut(&at.tile)?.rows.row_mut(at.id)?;
        self.dirty.insert(at.tile);
        Some(row.state)
    }

    /// The rows of each tile, in tile order.
    pub fn tiles_mut(&mut self) -> impl Iterator<Item = (TileKey, &mut TileRows)> {
        self.tiles
            .iter_mut()
            .map(|(&tile, entry)| (tile, &mut entry.rows))
    }

    /// Marks that a block changed in `tile`, so the rows that rest near it wake.
    pub fn stir(&mut self, tile: TileKey) {
        self.stirred.insert(tile);
    }

    /// Moves each row that the pass moved out of its tile into the tile of its position.
    pub fn move_rows(&mut self) {
        self.crossed.clear();
        for Leaving { from, id } in mem::take(&mut self.leaving) {
            let Some(row) = self
                .tiles
                .get_mut(&from)
                .and_then(|entry| entry.rows.despawn_row(id))
            else {
                continue;
            };
            let to = tile_at(row.state.body.position);
            if row.state.living.is_some() {
                self.crossed.push(Crossing { id, from, to });
            }
            self.tiles.entry(to).or_default().rows.insert_row(row);
            self.dirty.extend([from, to]);
        }
    }

    /// The freeze: writes the grid of each dirty tile from its rows, in parallel over the tiles,
    /// makes the written back buffers the front, and keeps the stirred tiles for the wakes of the
    /// next pass.
    pub fn freeze(&mut self, pool: &TaskPool, grid: &mut BodyGrid) {
        let dirty = mem::take(&mut self.dirty);
        let mut tiles: Vec<(TileKey, &mut Tile)> = self
            .tiles
            .iter_mut()
            .filter(|(tile, _)| dirty.contains(tile))
            .map(|(&tile, entry)| (tile, entry))
            .collect();
        let per_task = tiles_per_task(pool, tiles.len());
        let reaches: Vec<f32> = pool.scope(|scope| {
            for chunk in tiles.chunks_mut(per_task) {
                scope.spawn(async move {
                    let mut reach = 0.0_f32;
                    for (tile, entry) in chunk {
                        let bodies: Vec<GridRow> = entry
                            .rows
                            .rows_mut()
                            .filter_map(|row| grid_row(row.state, row.motion, row.id))
                            .collect();
                        reach = reach.max(entry.write(*tile, &bodies));
                    }
                    reach
                });
            }
        });
        self.reach = reaches.into_iter().fold(self.reach, f32::max);
        self.written.extend(dirty);
        for tile in mem::take(&mut self.written) {
            if let Some(entry) = self.tiles.get_mut(&tile) {
                grid.swap(tile, &mut entry.back);
            }
        }
        self.woken = mem::take(&mut self.stirred);
    }
}

impl Tile {
    /// Writes `bodies` into the back buffer in the order of the grid; gives their largest half
    /// width.
    pub(crate) fn write(&mut self, tile: TileKey, bodies: &[GridRow]) -> f32 {
        sort_bodies(tile, bodies, &mut self.back);
        bodies
            .iter()
            .map(|body| body.half_width)
            .fold(0.0, f32::max)
    }
}

/// The tiles in one task of a parallel pass: four tasks per thread.
pub(crate) fn tiles_per_task(pool: &TaskPool, tiles: usize) -> usize {
    tiles
        .div_ceil(pool.thread_num().saturating_mul(4).max(1))
        .max(1)
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
