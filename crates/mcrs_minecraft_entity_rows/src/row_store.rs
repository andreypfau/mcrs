use std::cmp::Reverse;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::mem;

use glam::DVec3;
use mcrs_minecraft_entity_key::EntityId;
use mcrs_minecraft_entity_key::TileKey;
use mcrs_minecraft_physics::EntityState;
use mcrs_minecraft_physics::GridRow;
use mcrs_minecraft_physics::MotionRow;
use mcrs_minecraft_physics::Rest;
use rustc_hash::FxHashMap;

use crate::BodyGrid;
use crate::Changes;
use crate::Crossing;
use crate::Executor;
use crate::Keyed;
use crate::MobIntent;
use crate::MobRows;
use crate::Placed;
use crate::Row;
use crate::TileBodies;
use crate::TileQueues;
use crate::TileRows;
use crate::front_grid::grid_row;
use crate::front_grid::sort_bodies;

/// The rows of the server entities of a dimension world, one segment per tile, with the mob
/// column `M` beside them.
#[derive(Debug)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::resource::Resource))]
pub struct RowStore<M> {
    pub(crate) tiles: BTreeMap<TileKey, Tile<M>>,
    /// The tile of each living row.
    pub(crate) living: FxHashMap<EntityId, TileKey>,
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
    /// The living rows whose visible state changed in the last pass, with their tile in it.
    pub(crate) moved: Vec<Placed>,
    pub(crate) changes: Changes,
    /// The largest half width of a body that the grid ever held.
    pub(crate) reach: f32,
}

/// The rows of one tile and the back buffer of its grid. The task of the tile writes the buffer,
/// so the bodies never pass through one thread.
#[derive(Debug)]
pub(crate) struct Tile<M> {
    pub(crate) rows: TileRows<M>,
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

impl<M> Default for RowStore<M> {
    fn default() -> Self {
        Self {
            tiles: BTreeMap::new(),
            living: FxHashMap::default(),
            dirty: BTreeSet::new(),
            written: BTreeSet::new(),
            stirred: BTreeSet::new(),
            woken: BTreeSet::new(),
            leaving: Vec::new(),
            moved: Vec::new(),
            changes: Changes::default(),
            reach: 0.0,
        }
    }
}

impl<M> Default for Tile<M> {
    fn default() -> Self {
        Self {
            rows: TileRows::default(),
            back: TileBodies::default(),
            intents: Vec::new(),
        }
    }
}

impl<M> RowStore<M> {
    /// Adds a row with `id` and no mob column in the tile of its position.
    pub fn spawn(&mut self, id: EntityId, state: EntityState, motion: MotionRow) {
        let tile = TileKey::at(state.body.position);
        if state.living.is_some() {
            self.living.insert(id, tile);
            self.changes.spawned.push(Placed { tile, id });
        }
        self.tiles.entry(tile).or_default().rows.insert_row(Row {
            state,
            motion,
            id,
            mob: None,
        });
        self.dirty.insert(tile);
    }

    /// Gives the row `at` the mob column `mob`, in place of the one it had; drops `mob` when the
    /// tile has no such row.
    pub fn set_mob(&mut self, at: Placed, mob: M) {
        if let Some(entry) = self.tiles.get_mut(&at.tile) {
            entry.rows.set_mob(at.id, mob);
        }
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
    pub fn despawn(&mut self, tile: TileKey, id: EntityId) -> Option<Row<M>> {
        let row = self.tiles.get_mut(&tile)?.rows.despawn_row(id)?;
        if row.state.living.is_some() {
            self.living.remove(&id);
            self.changes.removed.push(Placed { tile, id });
        }
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

    /// The tile of the living row `id`; `None` when the world has no such row.
    #[must_use]
    pub fn tile_of(&self, id: EntityId) -> Option<TileKey> {
        self.living.get(&id).copied()
    }

    /// The state of the row `at`; `None` when the tile has no such row.
    #[must_use]
    pub fn state_of(&self, at: Placed) -> Option<&EntityState> {
        let rows = &self.tiles.get(&at.tile)?.rows;
        let row = rows.row_of(at.id)?;
        rows.states().get(row)
    }

    /// The state of the row `at`, open to a writer outside the pass; `None` when the tile has no
    /// such row. The tile writes its grid again at the next freeze.
    pub fn state_mut(&mut self, at: Placed) -> Option<&mut EntityState> {
        let row = self.tiles.get_mut(&at.tile)?.rows.row_mut(at.id)?;
        self.dirty.insert(at.tile);
        Some(row.state)
    }

    /// The count of the tiles that hold rows or held them.
    #[must_use]
    pub fn tile_count(&self) -> usize {
        self.tiles.len()
    }

    /// The rows of `tile`; `None` when the world never had a row in it.
    #[must_use]
    pub fn tile(&self, tile: TileKey) -> Option<&TileRows<M>> {
        self.tiles.get(&tile).map(|entry| &entry.rows)
    }

    /// The rows of each tile, in tile order.
    pub fn tiles(&self) -> impl Iterator<Item = (TileKey, &TileRows<M>)> {
        self.tiles.iter().map(|(&tile, entry)| (tile, &entry.rows))
    }

    /// The mob columns of each tile that holds one, in tile order. They change no row, so no
    /// grid needs a new write.
    pub fn mob_tiles_mut(&mut self) -> impl Iterator<Item = (TileKey, MobRows<'_, M>)> {
        self.tiles
            .iter_mut()
            .filter(|(_, entry)| entry.rows.mob_count() > 0)
            .map(|(&tile, entry)| (tile, entry.rows.mobs_mut()))
    }

    /// The living rows that came into the world, changed tile or left it.
    #[must_use]
    pub fn changes(&self) -> &Changes {
        &self.changes
    }

    /// The living rows whose visible state changed in the last pass, in tile order, each with
    /// its tile in the pass. A row that crossed since is in [`Self::tile_of`]; a row that left the
    /// world is in no tile.
    #[must_use]
    pub fn moved(&self) -> &[Placed] {
        &self.moved
    }

    /// Marks that a block changed in `tile`, so the rows that rest near it wake.
    pub fn stir(&mut self, tile: TileKey) {
        self.stirred.insert(tile);
    }

    /// The merge point: each tile applies the impulses of `writes` to its rows in the order of
    /// the target key, the writer key and the sequence, so the f64 sums do not depend on the
    /// tasks that made the writes. A write whose target left the tile is dropped.
    pub fn apply_writes(&mut self, writes: &mut TileQueues<Keyed<DVec3>>) {
        let mut queues = mem::take(writes);
        for (&tile, entry) in &mut self.tiles {
            let entries = queues.take_in_order(tile);
            if entries.is_empty() {
                continue;
            }
            for write in entries {
                let Some(row) = entry.rows.row_mut(write.target_id) else {
                    continue;
                };
                row.state.body.velocity += write.effect;
                row.state.rest = Rest::Awake;
            }
            self.dirty.insert(tile);
        }
    }

    /// Moves each row that the pass moved out of its tile into the tile of its position, with
    /// its mob column.
    pub fn move_rows(&mut self) {
        self.changes.crossed.clear();
        for Leaving { from, id } in mem::take(&mut self.leaving) {
            let Some(row) = self
                .tiles
                .get_mut(&from)
                .and_then(|entry| entry.rows.despawn_row(id))
            else {
                continue;
            };
            let to = TileKey::at(row.state.body.position);
            if row.state.living.is_some() {
                self.changes.crossed.push(Crossing { id, from, to });
                self.living.insert(id, to);
            }
            self.tiles.entry(to).or_default().rows.insert_row(row);
            self.dirty.extend([from, to]);
        }
    }

    /// The freeze: writes the grid of each dirty tile from its rows, in parallel over the tiles,
    /// makes the written back buffers the front, and keeps the stirred tiles for the wakes of the
    /// next pass.
    pub fn freeze(&mut self, executor: &impl Executor, grid: &mut BodyGrid)
    where
        M: Send,
    {
        let dirty = mem::take(&mut self.dirty);
        let tiles: Vec<(TileKey, &mut Tile<M>)> = self
            .tiles
            .iter_mut()
            .filter(|(tile, _)| dirty.contains(tile))
            .map(|(&tile, entry)| (tile, entry))
            .collect();
        let per_task = tiles_per_task(executor, tiles.len());
        let reaches: Vec<f32> = executor.run(deal(tiles, per_task), &|chunk| {
            let mut reach = 0.0_f32;
            for (tile, entry) in chunk {
                let bodies: Vec<GridRow> = entry
                    .rows
                    .rows_mut()
                    .filter_map(|row| grid_row(row.state, row.motion, row.id))
                    .collect();
                reach = reach.max(entry.write(tile, &bodies));
            }
            reach
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

impl<M> Tile<M> {
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

/// The tiles in one task of a parallel pass over `tiles` tiles: four tasks per thread.
#[must_use]
pub fn tiles_per_task(executor: &impl Executor, tiles: usize) -> usize {
    tiles
        .div_ceil(executor.threads().saturating_mul(4).max(1))
        .max(1)
}

/// Deals `tiles` into tasks of about `per_task` tiles: the tiles with the most rows go first, one
/// to each task in turn, so the tasks carry about the same count of rows, and the task with the
/// largest tiles starts first.
pub(crate) fn deal<'a, M>(
    mut tiles: Vec<(TileKey, &'a mut Tile<M>)>,
    per_task: usize,
) -> Vec<Vec<(TileKey, &'a mut Tile<M>)>> {
    tiles.sort_by_key(|(_, entry)| Reverse(entry.rows.ids().len()));
    let count = tiles.len().div_ceil(per_task.max(1)).max(1);
    let mut tasks: Vec<Vec<(TileKey, &'a mut Tile<M>)>> = (0..count).map(|_| Vec::new()).collect();
    for (index, tile) in tiles.into_iter().enumerate() {
        if let Some(task) = tasks.get_mut(index % count) {
            task.push(tile);
        }
    }
    tasks
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
