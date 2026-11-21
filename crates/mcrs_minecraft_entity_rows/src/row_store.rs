use std::cmp::Reverse;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::mem;

use glam::DVec3;
use mcrs_minecraft_entity_key::EntityId;
use mcrs_minecraft_entity_key::TileKey;
use mcrs_minecraft_physics::BodyGrid;
use mcrs_minecraft_physics::EntityState;
use mcrs_minecraft_physics::GridRow;
use mcrs_minecraft_physics::MotionRow;
use mcrs_minecraft_physics::Rest;
use mcrs_minecraft_physics::TileBodies;
use mcrs_minecraft_physics::grid_row;
use rustc_hash::FxHashMap;

use crate::Changes;
use crate::Crossing;
use crate::CrowdHit;
use crate::Executor;
use crate::Keyed;
use crate::MobIntent;
use crate::MobRows;
use crate::Placed;
use crate::Row;
use crate::TileQueues;
use crate::TileRows;

#[derive(Debug)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::resource::Resource))]
pub struct RowStore<M> {
    pub(crate) tiles: BTreeMap<TileKey, Tile<M>>,
    pub(crate) living: FxHashMap<EntityId, TileKey>,
    pub(crate) dirty: BTreeSet<TileKey>,
    pub(crate) written: BTreeSet<TileKey>,
    pub(crate) stirred: BTreeSet<TileKey>,
    pub(crate) woken: BTreeSet<TileKey>,
    pub(crate) leaving: Vec<Leaving>,
    pub(crate) moved: Vec<Placed>,
    pub(crate) changes: Changes,
    pub(crate) crowd_hits: Vec<CrowdHit>,
    pub(crate) reach: f32,
}

#[derive(Debug)]
pub(crate) struct Tile<M> {
    pub(crate) rows: TileRows<M>,
    pub(crate) back: TileBodies,
    pub(crate) intents: Vec<MobIntent>,
}

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
            crowd_hits: Vec::new(),
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

    pub fn set_mob(&mut self, at: Placed, mob: M) {
        if let Some(entry) = self.tiles.get_mut(&at.tile) {
            entry.rows.set_mob(at.id, mob);
        }
    }

    pub fn steer(&mut self, tile: TileKey, intent: MobIntent) {
        self.tiles.entry(tile).or_default().intents.push(intent);
    }

    pub fn steer_all(&mut self, tile: TileKey, intents: impl IntoIterator<Item = MobIntent>) {
        self.tiles.entry(tile).or_default().intents.extend(intents);
    }

    pub fn despawn(&mut self, tile: TileKey, id: EntityId) -> Option<Row<M>> {
        let row = self.tiles.get_mut(&tile)?.rows.despawn_row(id)?;
        if row.state.living.is_some() {
            self.living.remove(&id);
            self.changes.removed.push(Placed { tile, id });
        }
        self.dirty.insert(tile);
        Some(row)
    }

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

    #[must_use]
    pub fn tile_of(&self, id: EntityId) -> Option<TileKey> {
        self.living.get(&id).copied()
    }

    #[must_use]
    pub fn state_of(&self, at: Placed) -> Option<&EntityState> {
        let rows = &self.tiles.get(&at.tile)?.rows;
        let row = rows.row_of(at.id)?;
        rows.states().get(row)
    }

    pub fn state_mut(&mut self, at: Placed) -> Option<&mut EntityState> {
        let row = self.tiles.get_mut(&at.tile)?.rows.row_mut(at.id)?;
        self.dirty.insert(at.tile);
        Some(row.state)
    }

    #[must_use]
    pub fn tile_count(&self) -> usize {
        self.tiles.len()
    }

    #[must_use]
    pub fn tile(&self, tile: TileKey) -> Option<&TileRows<M>> {
        self.tiles.get(&tile).map(|entry| &entry.rows)
    }

    pub fn tiles(&self) -> impl Iterator<Item = (TileKey, &TileRows<M>)> {
        self.tiles.iter().map(|(&tile, entry)| (tile, &entry.rows))
    }

    pub fn mob_tiles_mut(&mut self) -> impl Iterator<Item = (TileKey, MobRows<'_, M>)> {
        self.tiles
            .iter_mut()
            .filter(|(_, entry)| entry.rows.mob_count() > 0)
            .map(|(&tile, entry)| (tile, entry.rows.mobs_mut()))
    }

    #[must_use]
    pub fn changes(&self) -> &Changes {
        &self.changes
    }

    #[must_use]
    pub fn moved(&self) -> &[Placed] {
        &self.moved
    }

    #[must_use]
    pub fn crowd_hits(&self) -> &[CrowdHit] {
        &self.crowd_hits
    }

    pub fn stir(&mut self, tile: TileKey) {
        self.stirred.insert(tile);
    }

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
                reach = reach.max(entry.back.fill(tile, &bodies));
            }
            reach
        });
        self.reach = reaches.into_iter().fold(self.reach, f32::max);
        self.written.extend(dirty);
        grid.widen(self.reach);
        for tile in mem::take(&mut self.written) {
            if let Some(entry) = self.tiles.get_mut(&tile) {
                grid.swap(tile, &mut entry.back);
            }
        }
        self.woken = mem::take(&mut self.stirred);
    }
}

#[must_use]
pub fn tiles_per_task(executor: &impl Executor, tiles: usize) -> usize {
    tiles
        .div_ceil(executor.threads().saturating_mul(4).max(1))
        .max(1)
}

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
