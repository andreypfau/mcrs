use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::mem;

use crate::Cadence;
use crate::Decision;
use crate::Intents;
use crate::IssuedDecisions;
use crate::MobBrain;
use crate::MobId;
use crate::MobView;
use crate::OrderKey;
use crate::PlayerRow;
use crate::Point;
use crate::Replica;
use crate::StageConfig;
use crate::TileBrains;
use crate::TileId;
use crate::Turn;
use crate::WakeMode;

/// The brains of the mobs of one world, by tile. A tick of the stage is a parallel map over its
/// tiles ([`Stage::work`] and [`TileWork::run`]) and a gather ([`Stage::collect`]); the result does
/// not depend on the thread count or on the split of the tiles into tasks.
#[derive(Clone, Debug)]
pub struct Stage<B> {
    config: StageConfig,
    tiles: BTreeMap<TileId, TileBrains<B>>,
    locations: BTreeMap<MobId, Location>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Location {
    tile: TileId,
    key: OrderKey,
}

/// The read-only inputs of the tile tasks of one stage tick.
#[derive(Debug)]
pub struct TickContext<'a, W: ?Sized> {
    pub tick: u64,
    pub config: StageConfig,
    pub replica: &'a Replica,
    pub world: &'a W,
}

/// The work of one tile in one stage tick.
#[derive(Debug)]
pub struct TileWork<'a, B> {
    tile: TileId,
    brains: &'a mut TileBrains<B>,
}

impl<B> Stage<B> {
    #[must_use]
    pub fn new(config: StageConfig) -> Self {
        Self {
            config,
            tiles: BTreeMap::new(),
            locations: BTreeMap::new(),
        }
    }

    #[must_use]
    pub const fn config(&self) -> &StageConfig {
        &self.config
    }

    /// Adds the brain of a mob that entered `tile`; its first full evaluation is the first of
    /// `cadence`. A mob that the stage keeps already loses its old brain.
    pub fn spawn(&mut self, tile: TileId, mob: MobId, key: OrderKey, cadence: Cadence, brain: B) {
        self.despawn(mob);
        let wake = cadence.next_evaluation(0);
        self.tiles
            .entry(tile)
            .or_default()
            .insert(mob, key, cadence, wake, brain);
        self.locations.insert(mob, Location { tile, key });
    }

    /// Removes the brain of a mob that left the world.
    pub fn despawn(&mut self, mob: MobId) -> Option<B> {
        let location = self.locations.remove(&mob)?;
        let tiles = self.tiles.get_mut(&location.tile)?;
        let removed = tiles.remove(&location.key);
        if tiles.is_empty() {
            self.tiles.remove(&location.tile);
        }
        removed.map(|removed| removed.brain)
    }

    /// Moves the brain of a mob that the hot tick moved into another tile.
    pub fn relocate(&mut self, mob: MobId, to: TileId) {
        let Some(location) = self.locations.get_mut(&mob) else {
            return;
        };
        if location.tile == to {
            return;
        }
        let from = mem::replace(&mut location.tile, to);
        let key = location.key;
        let Some(tiles) = self.tiles.get_mut(&from) else {
            return;
        };
        let removed = tiles.remove(&key);
        if tiles.is_empty() {
            self.tiles.remove(&from);
        }
        if let Some(removed) = removed {
            self.tiles.entry(to).or_default().insert(
                removed.mob,
                removed.key,
                removed.cadence,
                removed.wake,
                removed.brain,
            );
        }
    }

    /// The tile of `mob`.
    #[must_use]
    pub fn tile_of(&self, mob: MobId) -> Option<TileId> {
        self.locations.get(&mob).map(|location| location.tile)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.locations.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.locations.is_empty()
    }

    pub fn tiles(&self) -> impl Iterator<Item = (TileId, &TileBrains<B>)> {
        self.tiles.iter().map(|(&tile, brains)| (tile, brains))
    }

    /// The count of full evaluations that the stage ran.
    #[must_use]
    pub fn evaluations(&self) -> u64 {
        self.tiles.values().map(TileBrains::evaluations).sum()
    }

    /// The tile tasks of one tick, in tile order. Each task borrows only its own tile.
    pub fn work(&mut self) -> Vec<TileWork<'_, B>> {
        self.tiles
            .iter_mut()
            .map(|(&tile, brains)| TileWork { tile, brains })
            .collect()
    }

    /// Gathers the decisions that the tile tasks of `tick` made.
    pub fn collect(&mut self, tick: u64) -> IssuedDecisions {
        let mut issued = IssuedDecisions {
            tick,
            tiles: BTreeMap::new(),
        };
        for (&tile, brains) in &mut self.tiles {
            if !brains.decisions.is_empty() {
                issued.tiles.insert(tile, mem::take(&mut brains.decisions));
            }
        }
        issued
    }
}

impl<B> TileWork<'_, B> {
    #[must_use]
    pub const fn tile(&self) -> TileId {
        self.tile
    }

    /// The full evaluations of the tick in this tile: each mob that a fact or its own wake tick
    /// names, in the order of the order keys.
    pub fn run<W: ?Sized>(&mut self, cx: &TickContext<'_, W>)
    where
        B: MobBrain<W>,
    {
        let tick = cx.tick;
        let brains = &mut *self.brains;
        brains.decisions.clear();
        brains.near_players.clear();
        for fact in cx.replica.facts(self.tile, tick) {
            if let Some(at) = brains.position(fact.mob)
                && let (Some(wake), Some(cadence)) =
                    (brains.wakes.get_mut(at), brains.cadences.get(at))
            {
                *wake = (*wake).min(cadence.next_evaluation(tick));
            }
        }
        let views = cx.replica.views(self.tile, tick);
        let mut players_loaded = false;
        let TileBrains {
            ids,
            keys,
            cadences,
            wakes,
            brains: states,
            near_players,
            decisions,
            evaluations,
        } = brains;
        let rows = ids.iter().zip(keys.iter()).zip(cadences.iter());
        for (index, (((mob, key), cadence), (wake, brain))) in rows
            .zip(wakes.iter_mut().zip(states.iter_mut()))
            .enumerate()
        {
            if *wake > tick {
                continue;
            }
            if !cadence.is_full_evaluation(tick) {
                *wake = cadence.next_evaluation(tick);
                continue;
            }
            let Some(view) = view_of(views, index, *mob, key) else {
                *wake = cadence.next_evaluation(tick.saturating_add(1));
                continue;
            };
            if !players_loaded {
                players_loaded = true;
                load_players(cx, views, B::PLAYER_RANGE, near_players);
            }
            let turn = Turn {
                tick,
                seed: cx.config.seed(),
                max_lag: cx.config.max_lag(),
                cadence: *cadence,
                view,
                players: near_players,
                world: cx.world,
            };
            let mut intents = Intents::default();
            let next = brain.evaluate(&turn, &mut intents);
            *evaluations += 1;
            *wake = match cx.config.wake() {
                WakeMode::Sleep => next.max(tick.saturating_add(1)),
                WakeMode::Poll => turn.next_evaluation(),
            };
            if !intents.is_empty() {
                decisions.push(Decision {
                    mob: *mob,
                    key: *key,
                    issue_tick: tick,
                    intents,
                });
            }
        }
    }
}

fn view_of<'a>(
    views: &'a [MobView],
    index: usize,
    mob: MobId,
    key: &OrderKey,
) -> Option<&'a MobView> {
    if let Some(view) = views.get(index)
        && view.mob == mob
    {
        return Some(view);
    }
    let at = views.binary_search_by_key(key, |view| view.key).ok()?;
    views.get(at).filter(|view| view.mob == mob)
}

fn load_players<W: ?Sized>(
    cx: &TickContext<'_, W>,
    views: &[MobView],
    range: f64,
    out: &mut Vec<PlayerRow>,
) {
    let Some(table) = cx.replica.players(cx.tick) else {
        return;
    };
    if table.is_empty() {
        return;
    }
    let mut min = Point::new(f64::INFINITY, 0.0, f64::INFINITY);
    let mut max = Point::new(f64::NEG_INFINITY, 0.0, f64::NEG_INFINITY);
    for view in views {
        min.x = min.x.min(view.position.x);
        min.z = min.z.min(view.position.z);
        max.x = max.x.max(view.position.x);
        max.z = max.z.max(view.position.z);
    }
    if min.x > max.x {
        return;
    }
    let margin = range + 1.0;
    table.collect_in(
        Point::new(min.x - margin, 0.0, min.z - margin),
        Point::new(max.x + margin, 0.0, max.z + margin),
        out,
    );
}
