use std::cmp::Reverse;
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
use mcrs_minecraft_goals::GoalWorld;
use mcrs_minecraft_goals::PigBrain;
use mcrs_minecraft_physics::TileRows;
use mcrs_minecraft_volume::BlockVolume;
use rustc_hash::FxHashMap;

use crate::AiStats;
use crate::HotTick;
use crate::host_mob::HostMob;
use crate::life_task::LifeOut;
use crate::life_task::LifeTask;
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
        self.follow_life_cycle(tick, hot.mobs, rows, pool);
        let followed = Instant::now();
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
        let dealt = deal(tasks, per_task, |task| task.rows.ids().len());
        let outs: Vec<Vec<(usize, TileOut)>> = pool.scope(|scope| {
            for chunk in dealt {
                scope.spawn(async move {
                    chunk
                        .into_iter()
                        .map(|(index, mut task)| (index, task.run(hot)))
                        .collect()
                });
            }
        });
        let outs = in_order(outs);
        due.clear();
        let ran = Instant::now();
        self.publish(tick, outs, hot, rows);
        self.stats.life_cycle = followed.duration_since(started);
        self.stats.tile_tasks = ran.duration_since(followed);
        self.stats.publish = ran.elapsed();
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
    /// changed tile, and drops each mob that left the world. The tiles find their changes in
    /// parallel; only the changes run on one thread.
    fn follow_life_cycle(
        &mut self,
        tick: u64,
        mobs: &Mobs,
        rows: &mut EntityRows,
        pool: &TaskPool,
    ) {
        let mut departed = Vec::new();
        let mut tasks: Vec<LifeTask<'_>> = Vec::with_capacity(self.tiles.len());
        let mut held = self.tiles.iter_mut().peekable();
        for (tile, tile_rows) in rows.tiles_mut() {
            while let Some((_, hosts)) = held.next_if(|(key, _)| **key < tile) {
                departed.append(hosts);
            }
            let tile_rows: &TileRows = tile_rows;
            tasks.push(LifeTask {
                tile,
                rows: tile_rows,
                mobs: held
                    .next_if(|(key, _)| **key == tile)
                    .map(|(_, hosts)| hosts),
            });
        }
        for (_, hosts) in held {
            departed.append(hosts);
        }
        let per_task = tiles_per_task(pool, tasks.len());
        let dealt = deal(tasks, per_task, |task| task.rows.ids().len());
        let outs: Vec<Vec<(usize, LifeOut)>> = pool.scope(|scope| {
            for chunk in dealt {
                scope.spawn(async move {
                    chunk
                        .into_iter()
                        .map(|(index, mut task)| (index, task.run(tick, mobs)))
                        .collect()
                });
            }
        });
        let mut arrivals = Vec::new();
        for out in in_order(outs) {
            arrivals.extend(out.arrivals);
            departed.extend(out.departed);
        }
        let mut moving: FxHashMap<EntityId, HostMob> =
            departed.into_iter().map(|host| (host.id, host)).collect();
        for arrival in arrivals {
            let host = match moving.remove(&arrival.id) {
                Some(mut host) => {
                    self.stage.relocate(host.mob(), tile_id(arrival.tile));
                    host.seen = tick;
                    host
                }
                None => {
                    let key = order_key(&arrival.key);
                    let host = HostMob::new(arrival.id, key, arrival.kind, tick);
                    self.stage.spawn(
                        tile_id(arrival.tile),
                        host.mob(),
                        key,
                        Cadence::new(tick, arrival.phase),
                        PigBrain::default(),
                    );
                    host
                }
            };
            insert(&mut self.tiles, arrival.tile, host);
            self.index.insert(arrival.id, arrival.tile);
        }
        for id in moving.into_keys() {
            self.index.remove(&id);
            self.stage.despawn(MobId(id.0));
        }
        self.tiles.retain(|_, hosts| !hosts.is_empty());
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
        outs: Vec<TileOut>,
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
        for out in outs {
            applied = applied.saturating_add(out.applied);
            self.replica
                .publish_tile(tick, tile_id(out.tile), out.views);
            if !out.facts.is_empty() {
                facts.entry(out.tile).or_default().extend(out.facts);
            }
            self.ended.extend(out.ended);
            rows.steer_all(out.tile, out.intents);
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

/// Deals `tasks` into groups of about `per_task`, each with its index: the heaviest tasks go
/// first, one to each group in turn, so the groups carry about the same weight.
fn deal<T>(tasks: Vec<T>, per_task: usize, weight: impl Fn(&T) -> usize) -> Vec<Vec<(usize, T)>> {
    let count = tasks.len().div_ceil(per_task.max(1)).max(1);
    let mut indexed: Vec<(usize, T)> = tasks.into_iter().enumerate().collect();
    indexed.sort_by_key(|(_, task)| Reverse(weight(task)));
    let mut groups: Vec<Vec<(usize, T)>> = (0..count).map(|_| Vec::new()).collect();
    for (at, task) in indexed.into_iter().enumerate() {
        if let Some(group) = groups.get_mut(at % count) {
            group.push(task);
        }
    }
    groups
}

/// The outputs of the groups of [`deal`] in the order of the tasks.
fn in_order<T>(groups: Vec<Vec<(usize, T)>>) -> Vec<T> {
    let mut outs: Vec<(usize, T)> = groups.into_iter().flatten().collect();
    outs.sort_unstable_by_key(|(index, _)| *index);
    outs.into_iter().map(|(_, out)| out).collect()
}

fn tile_id(tile: TileKey) -> TileId {
    TileId::new(tile.x, tile.z)
}

fn insert(tiles: &mut BTreeMap<TileKey, Vec<HostMob>>, tile: TileKey, host: HostMob) {
    let hosts = tiles.entry(tile).or_default();
    let at = hosts.partition_point(|held| held.id < host.id);
    hosts.insert(at, host);
}
