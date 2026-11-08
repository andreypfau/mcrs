use std::collections::BTreeMap;
use std::mem;
use std::time::Instant;

use bevy_ecs::resource::Resource;
use bevy_tasks::TaskPool;
use mcrs_engine_muxer::EntityId;
use mcrs_engine_muxer::TileKey;
use mcrs_minecraft_brain::Cadence;
use mcrs_minecraft_brain::Decision;
use mcrs_minecraft_brain::DecisionPipeline;
use mcrs_minecraft_brain::Fact;
use mcrs_minecraft_brain::FactKind;
use mcrs_minecraft_brain::MobId;
use mcrs_minecraft_brain::PlayerTable;
use mcrs_minecraft_brain::Replica;
use mcrs_minecraft_brain::Stage;
use mcrs_minecraft_brain::StageConfig;
use mcrs_minecraft_brain::TickContext;
use mcrs_minecraft_brain::TileId;
use mcrs_minecraft_engine_physics::EntityRows;
use mcrs_minecraft_engine_physics::Mobs;
use mcrs_minecraft_engine_physics::Placed;
use mcrs_minecraft_goals::GoalWorld;
use mcrs_minecraft_goals::PigBrain;
use mcrs_minecraft_mob::MobType;
use mcrs_minecraft_physics::TileRows;
use mcrs_minecraft_volume::BlockVolume;
use rustc_hash::FxHashMap;

use crate::AiStats;
use crate::HotTick;
use crate::host_mob::HostMob;
use crate::order_key::order_key;
use crate::tile_task::TileOut;
use crate::tile_task::TileTask;

/// The AI of the mobs of one dimension world. In each tick [`MobAi::prepare`] publishes the
/// snapshot of the tick, applies the due decisions and steers the rows before the entity pass;
/// [`MobAi::run_stage`] runs the stage of the tick beside the pass. Both work in parallel over
/// the tiles, and their result does not depend on the thread count.
#[derive(Resource, Debug)]
pub struct MobAi {
    stage: Stage<PigBrain>,
    pipeline: DecisionPipeline,
    replica: Replica,
    /// The mobs of each tile, by entity id.
    tiles: BTreeMap<TileKey, Vec<HostMob>>,
    index: FxHashMap<EntityId, TileKey>,
    /// The mobs whose path ended in the last tick.
    ended: Vec<MobId>,
    /// The AI ticks so far: the clock of the cadences, the keyed random values and the damage
    /// records.
    tick: u64,
    stats: AiStats,
}

impl MobAi {
    #[must_use]
    pub fn new(config: StageConfig) -> Self {
        Self {
            pipeline: DecisionPipeline::new(&config),
            stage: Stage::new(config),
            replica: Replica::default(),
            tiles: BTreeMap::new(),
            index: FxHashMap::default(),
            ended: Vec::new(),
            tick: 0,
            stats: AiStats::default(),
        }
    }

    #[must_use]
    pub fn stage(&self) -> &Stage<PigBrain> {
        &self.stage
    }

    /// The last AI tick.
    #[must_use]
    pub const fn tick(&self) -> u64 {
        self.tick
    }

    #[must_use]
    pub const fn stats(&self) -> &AiStats {
        &self.stats
    }

    /// The hot side of the next AI tick, before the entity pass: follows the life cycle of the
    /// mobs, publishes the snapshot, applies the due decisions and queues an intent for each
    /// living mob of the stage.
    pub fn prepare<V: BlockVolume + Sync>(
        &mut self,
        hot: &HotTick<'_, V>,
        rows: &mut EntityRows,
        pool: &TaskPool,
    ) {
        let started = Instant::now();
        self.tick = self.tick.saturating_add(1);
        let tick = self.tick;
        self.follow_life_cycle(tick, hot.mobs, rows);
        let mut due = self.due(tick);
        let mut tasks: Vec<TileTask<'_>> = Vec::with_capacity(self.tiles.len());
        let mut held = self.tiles.iter_mut().peekable();
        for (tile, tile_rows) in rows.tiles_mut() {
            while held.next_if(|(key, _)| **key < tile).is_some() {}
            let Some((_, mobs)) = held.next_if(|(key, _)| **key == tile) else {
                continue;
            };
            let tile_rows: &TileRows = tile_rows;
            tasks.push(TileTask {
                tile,
                rows: tile_rows,
                mobs,
                due: due.get(&tile).map_or(&[], Vec::as_slice),
                tick,
            });
        }
        let per_task = tiles_per_task(pool, tasks.len());
        let outs: Vec<Vec<TileOut>> = pool.scope(|scope| {
            for chunk in tasks.chunks_mut(per_task) {
                scope.spawn(async move { chunk.iter_mut().map(|task| task.run(hot)).collect() });
            }
        });
        drop(tasks);
        due.clear();
        self.publish(tick, outs, hot, rows);
        self.stats.prepare = started.elapsed();
    }

    /// The stage of the tick of the last [`MobAi::prepare`] over `world`, the blocks at the start
    /// of the tick.
    pub fn run_stage<W: GoalWorld + Sync>(&mut self, pool: &TaskPool, world: &W) {
        let started = Instant::now();
        let tick = self.tick;
        let evaluations = self.stage.evaluations();
        let cx = TickContext {
            tick,
            config: *self.stage.config(),
            replica: &self.replica,
            world,
        };
        let mut work = self.stage.work();
        let per_task = tiles_per_task(pool, work.len());
        pool.scope(|scope| {
            let cx = &cx;
            for chunk in work.chunks_mut(per_task) {
                scope.spawn(async move {
                    for task in chunk {
                        task.run(cx);
                    }
                });
            }
        });
        drop(work);
        let issued = self.stage.collect(tick);
        self.stats.decisions = issued.len();
        self.pipeline.ready(issued);
        self.replica.retire(tick.saturating_add(1));
        self.stats.evaluations = self.stage.evaluations().saturating_sub(evaluations);
        self.stats.stage = started.elapsed();
    }

    /// Registers each pig with AI that came into the world, moves the brain of each mob that
    /// changed tile, and drops each mob that left the world.
    fn follow_life_cycle(&mut self, tick: u64, mobs: &Mobs, rows: &EntityRows) {
        for (&id, mob) in &mobs.0 {
            if mob.kind != MobType::Pig || mob.looks.no_ai {
                continue;
            }
            let tile = mob.tile;
            match self.index.get(&id).copied() {
                Some(held) if held == tile => {
                    if let Some(host) = find_mut(&mut self.tiles, held, id) {
                        host.seen = tick;
                    }
                }
                Some(held) => {
                    let Some(mut host) = take(&mut self.tiles, held, id) else {
                        continue;
                    };
                    host.seen = tick;
                    insert(&mut self.tiles, tile, host);
                    self.index.insert(id, tile);
                    self.stage.relocate(host.mob(), tile_id(tile));
                }
                None => {
                    let Some(state) = rows.state_of(Placed { tile, id }) else {
                        continue;
                    };
                    let key = order_key(&state.key);
                    let host = HostMob::new(id, key, mob.kind, tick);
                    self.stage.spawn(
                        tile_id(tile),
                        host.mob(),
                        key,
                        Cadence::new(tick, state.phase),
                        PigBrain::default(),
                    );
                    insert(&mut self.tiles, tile, host);
                    self.index.insert(id, tile);
                }
            }
        }
        let mut gone = Vec::new();
        for hosts in self.tiles.values_mut() {
            hosts.retain(|host| {
                let found = host.seen == tick;
                if !found {
                    gone.push(host.id);
                }
                found
            });
        }
        self.tiles.retain(|_, hosts| !hosts.is_empty());
        for id in gone {
            self.index.remove(&id);
            self.stage.despawn(MobId(id.0));
        }
        self.stats.mobs = self.index.len();
    }

    /// The decisions due in `tick`, by the tile of their mob now, each tile in the order of the
    /// issue ticks and the keys.
    fn due(&mut self, tick: u64) -> BTreeMap<TileKey, Vec<Decision>> {
        let mut by_tile: BTreeMap<TileKey, Vec<Decision>> = BTreeMap::new();
        let issued = match self.pipeline.take_due(tick) {
            Ok(issued) => issued,
            Err(_) => {
                self.stats.waits = self.stats.waits.saturating_add(1);
                Vec::new()
            }
        };
        for decisions in issued {
            for decision in decisions.tiles.into_values().flatten() {
                if let Some(&tile) = self.index.get(&EntityId(decision.mob.0)) {
                    by_tile.entry(tile).or_default().push(decision);
                }
            }
        }
        for decisions in by_tile.values_mut() {
            decisions.sort_by_key(|decision| (decision.issue_tick, decision.key));
        }
        by_tile
    }

    fn publish<V>(
        &mut self,
        tick: u64,
        outs: Vec<Vec<TileOut>>,
        hot: &HotTick<'_, V>,
        rows: &mut EntityRows,
    ) {
        let mut facts: BTreeMap<TileKey, Vec<Fact>> = BTreeMap::new();
        for mob in mem::take(&mut self.ended) {
            if let Some(&tile) = self.index.get(&EntityId(mob.0)) {
                facts.entry(tile).or_default().push(Fact {
                    mob,
                    kind: FactKind::PathEnded,
                });
            }
        }
        let mut applied: usize = 0;
        for out in outs.into_iter().flatten() {
            applied = applied.saturating_add(out.applied);
            self.replica
                .publish_tile(tick, tile_id(out.tile), out.views);
            if !out.facts.is_empty() {
                facts.entry(out.tile).or_default().extend(out.facts);
            }
            self.ended.extend(out.ended);
            for intent in out.intents {
                rows.steer(out.tile, intent);
            }
        }
        for (tile, facts) in facts {
            self.replica.publish_facts(tick, tile_id(tile), facts);
        }
        self.replica
            .publish_players(tick, PlayerTable::new(hot.players.iter().copied()));
        self.pipeline.issue(tick);
        self.stats.applied = applied;
    }
}

/// The tiles in one task of a parallel pass: four tasks per thread.
fn tiles_per_task(pool: &TaskPool, tiles: usize) -> usize {
    tiles
        .div_ceil(pool.thread_num().saturating_mul(4).max(1))
        .max(1)
}

fn tile_id(tile: TileKey) -> TileId {
    TileId::new(tile.x, tile.z)
}

fn find_mut(
    tiles: &mut BTreeMap<TileKey, Vec<HostMob>>,
    tile: TileKey,
    id: EntityId,
) -> Option<&mut HostMob> {
    let hosts = tiles.get_mut(&tile)?;
    let at = hosts.binary_search_by_key(&id, |host| host.id).ok()?;
    hosts.get_mut(at)
}

fn take(
    tiles: &mut BTreeMap<TileKey, Vec<HostMob>>,
    tile: TileKey,
    id: EntityId,
) -> Option<HostMob> {
    let hosts = tiles.get_mut(&tile)?;
    let at = hosts.binary_search_by_key(&id, |host| host.id).ok()?;
    Some(hosts.remove(at))
}

fn insert(tiles: &mut BTreeMap<TileKey, Vec<HostMob>>, tile: TileKey, host: HostMob) {
    let hosts = tiles.entry(tile).or_default();
    let at = hosts.partition_point(|held| held.id < host.id);
    hosts.insert(at, host);
}
