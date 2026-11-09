use std::cmp::Reverse;
use std::collections::BTreeMap;
use std::mem;
use std::time::Instant;

use bevy_ecs::resource::Resource;
use bevy_tasks::TaskPool;
use mcrs_minecraft_brain::AiDriver;
use mcrs_minecraft_brain::Cadence;
use mcrs_minecraft_brain::Decision;
use mcrs_minecraft_brain::Fact;
use mcrs_minecraft_brain::FactKind;
use mcrs_minecraft_brain::GoalWorld;
use mcrs_minecraft_brain::MobBrain;
use mcrs_minecraft_brain::PanicCauses;
use mcrs_minecraft_brain::PlayerTable;
use mcrs_minecraft_brain::Stage;
use mcrs_minecraft_brain::StageConfig;
use mcrs_minecraft_brain::TickContext;
use mcrs_minecraft_brain::TileWork;
use mcrs_minecraft_engine_physics::Mobs;
use mcrs_minecraft_entity::EntityTypeData;
use mcrs_minecraft_entity_key::EntityId;
use mcrs_minecraft_entity_key::TileKey;
use mcrs_minecraft_entity_rows::Placed;
use mcrs_minecraft_entity_rows::tiles_per_task;
use mcrs_minecraft_volume::BlockVolume;

use crate::AiStats;
use crate::BlockGoalWorld;
use crate::EntityRows;
use crate::HotTick;
use crate::MobColumn;
use crate::mob_column_ext::BrainRules;
use crate::tile_task::TileOut;
use crate::tile_task::TileTask;

/// The AI of the mobs of one dimension world, with the brains `B`. In each tick
/// [`MobAi::prepare`] publishes the snapshot of the tick, applies the due decisions and steers the
/// rows before the entity pass; [`MobAi::run_stage`] runs the stage of the tick beside the pass.
/// Both work in parallel over the tiles, and their result does not depend on the thread count.
/// The hot state of each mob with a brain is the [`MobColumn`] of its row.
#[derive(Resource, Debug)]
pub struct MobAi<B: Send + Sync + 'static> {
    driver: AiDriver<B>,
    /// The brain of a mob that came into the world, from its type; `None` for a type without AI.
    brain_for: fn(&'static EntityTypeData) -> Option<B>,
    panic_causes: PanicCauses,
    /// The mobs whose path ended in the last tick.
    ended: Vec<EntityId>,
    stats: AiStats,
}

impl<B: Send + Sync + 'static> MobAi<B> {
    /// The AI of a world in which each mob without `NoAI` gets the brain that `brain_for` makes
    /// for its type.
    #[must_use]
    pub fn new(config: StageConfig, brain_for: fn(&'static EntityTypeData) -> Option<B>) -> Self {
        Self {
            driver: AiDriver::new(config),
            brain_for,
            panic_causes: PanicCauses::default(),
            ended: Vec::new(),
            stats: AiStats::default(),
        }
    }

    /// Sets the damage types whose hit makes a mob panic; a new AI has none.
    pub fn set_panic_causes(&mut self, panic_causes: PanicCauses) {
        self.panic_causes = panic_causes;
    }

    #[must_use]
    pub const fn stage(&self) -> &Stage<B> {
        self.driver.stage()
    }

    /// The last AI tick: the clock of the cadences, the keyed random values and the damage
    /// records.
    #[must_use]
    pub const fn tick(&self) -> u64 {
        self.driver.tick()
    }

    #[must_use]
    pub const fn stats(&self) -> &AiStats {
        &self.stats
    }

    /// The hot side of the next AI tick, before the entity pass: follows the life cycle of the
    /// mobs, publishes the snapshot with `players`, the players of the dimension at the end of
    /// the last tick, applies the due decisions and queues an intent for each living mob of the
    /// stage.
    pub fn prepare<V: BlockVolume + Sync>(
        &mut self,
        hot: &HotTick<'_, V>,
        players: PlayerTable,
        rows: &mut EntityRows,
        pool: &TaskPool,
    ) where
        B: for<'w> MobBrain<BlockGoalWorld<'w, V>>,
    {
        let started = Instant::now();
        let tick = self.driver.begin_tick();
        self.follow_life_cycle(tick, hot.mobs, rows);
        let followed = Instant::now();
        let mut due = self.due();
        let rules = BrainRules {
            panic_causes: &self.panic_causes,
            idle_limit: <B as MobBrain<BlockGoalWorld<'_, V>>>::IDLE_LIMIT,
        };
        let mut tasks: Vec<TileTask<'_>> = Vec::new();
        for (tile, mobs) in rows.mob_tiles_mut() {
            tasks.push(TileTask {
                tile,
                mobs,
                due: due.get(&tile).map_or(&[], Vec::as_slice),
                players: &players,
                rules,
                tick,
            });
        }
        let per_task = tiles_per_task(pool, tasks.len());
        let dealt = deal(tasks, per_task, |task| task.mobs.len());
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
        self.publish(outs, players, rows);
        self.stats.life_cycle = followed.duration_since(started);
        self.stats.tile_tasks = ran.duration_since(followed);
        self.stats.publish = ran.elapsed();
        self.stats.prepare = started.elapsed();
    }

    /// The stage of the tick of the last [`MobAi::prepare`] over `world`, the blocks at the start
    /// of the tick.
    pub fn run_stage<W: GoalWorld + Sync>(&mut self, pool: &TaskPool, world: &W)
    where
        B: MobBrain<W>,
    {
        let started = Instant::now();
        let evaluations = self.driver.stage().evaluations();
        self.stats.decisions = self
            .driver
            .run_stage(world, |work, cx| run_tiles(pool, work, cx));
        self.stats.evaluations = self
            .driver
            .stage()
            .evaluations()
            .saturating_sub(evaluations);
        self.stats.stage = started.elapsed();
    }

    /// Follows the life cycle of the rows since the last tick: drops the brain of each mob that
    /// left the world, moves the brain of each mob that changed tile, and gives each new mob with
    /// AI a brain and a mob column. The column moves with its row, so only the changes run.
    fn follow_life_cycle(&mut self, tick: u64, mobs: &Mobs, rows: &mut EntityRows) {
        let changes = rows.changes();
        for removed in changes.removed() {
            self.driver.despawn(removed.id);
        }
        self.driver.relocate_all(
            changes
                .crossed()
                .iter()
                .map(|crossing| (crossing.id, crossing.to)),
        );
        let spawned: Vec<Placed> = changes
            .spawned()
            .iter()
            .filter(|at| self.driver.stage().tile_of(at.id).is_none())
            .copied()
            .collect();
        for at in spawned {
            let Some(mob) = mobs.0.get(&at.id) else {
                continue;
            };
            if mob.looks.no_ai {
                continue;
            }
            let Some(brain) = (self.brain_for)(mob.kind) else {
                continue;
            };
            let Some(column) = MobColumn::new(mob.kind) else {
                continue;
            };
            let Some(state) = rows.state_of(at) else {
                continue;
            };
            let cadence = Cadence::new(tick, state.phase);
            self.driver.spawn(at.tile, at.id, state.key, cadence, brain);
            rows.set_mob(at, column);
        }
        self.stats.mobs = self.driver.stage().len();
    }

    /// The decisions due in this tick, by the tile of their mob now, each tile in the order of the
    /// issue ticks and the keys.
    fn due(&mut self) -> BTreeMap<TileKey, Vec<Decision>> {
        let mut by_tile: BTreeMap<TileKey, Vec<Decision>> = BTreeMap::new();
        let issued = self.driver.take_due();
        self.stats.waits = self.driver.waits();
        for decisions in issued {
            for decision in decisions.tiles.into_values().flatten() {
                if let Some(tile) = self.driver.stage().tile_of(decision.mob) {
                    by_tile.entry(tile).or_default().push(decision);
                }
            }
        }
        for decisions in by_tile.values_mut() {
            decisions.sort_by_key(|decision| (decision.issue_tick, decision.key));
        }
        by_tile
    }

    fn publish(&mut self, outs: Vec<TileOut>, players: PlayerTable, rows: &mut EntityRows) {
        let mut facts: BTreeMap<TileKey, Vec<Fact>> = BTreeMap::new();
        for mob in mem::take(&mut self.ended) {
            if let Some(tile) = self.driver.stage().tile_of(mob) {
                facts.entry(tile).or_default().push(Fact {
                    mob,
                    kind: FactKind::PathEnded,
                });
            }
        }
        let mut applied: usize = 0;
        for out in outs {
            applied = applied.saturating_add(out.applied);
            self.driver.publish_tile(out.tile, out.views);
            if !out.facts.is_empty() {
                facts.entry(out.tile).or_default().extend(out.facts);
            }
            self.ended.extend(out.ended);
            rows.steer_all(out.tile, out.intents);
        }
        for (tile, facts) in facts {
            self.driver.publish_facts(tile, facts);
        }
        self.driver.publish_players(players);
        self.stats.applied = applied;
    }
}

/// Runs the tile tasks of one stage tick on `pool`.
fn run_tiles<B, W>(pool: &TaskPool, work: &mut [TileWork<'_, B>], cx: &TickContext<'_, W>)
where
    B: MobBrain<W> + Send,
    W: GoalWorld + Sync,
{
    let per_task = tiles_per_task(pool, work.len());
    pool.scope(|scope| {
        for chunk in work.chunks_mut(per_task) {
            scope.spawn(async move {
                for task in chunk {
                    task.run(cx);
                }
            });
        }
    });
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
