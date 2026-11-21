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
use mcrs_minecraft_brain::MobBrain;
use mcrs_minecraft_brain::PanicCauses;
use mcrs_minecraft_brain::PlayerTable;
use mcrs_minecraft_brain::Stage;
use mcrs_minecraft_brain::StageConfig;
use mcrs_minecraft_engine_physics::Mobs;
use mcrs_minecraft_entity::EntityTypeData;
use mcrs_minecraft_entity_key::EntityId;
use mcrs_minecraft_entity_key::TileKey;
use mcrs_minecraft_entity_rows::Placed;
use mcrs_minecraft_entity_rows::tiles_per_task;
use mcrs_minecraft_goal_world::GoalWorld;
use mcrs_minecraft_volume::BlockVolume;

use crate::AiStats;
use crate::BlockGoalWorld;
use crate::EntityRows;
use crate::HotTick;
use crate::run_tiles::run_tiles;
use crate::tile_task::TileOut;
use crate::tile_task::TileTask;

#[derive(Resource, Debug)]
pub struct RowMobAi<B: Send + Sync + 'static> {
    driver: AiDriver<B>,
    brain_for: fn(&'static EntityTypeData) -> Option<B>,
    panic_causes: PanicCauses,
    ended: Vec<EntityId>,
    stats: AiStats,
}

impl<B: Send + Sync + 'static> RowMobAi<B> {
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

    pub fn set_panic_causes(&mut self, panic_causes: PanicCauses) {
        self.panic_causes = panic_causes;
    }

    #[must_use]
    pub const fn stage(&self) -> &Stage<B> {
        self.driver.stage()
    }

    #[must_use]
    pub const fn tick(&self) -> u64 {
        self.driver.tick()
    }

    #[must_use]
    pub const fn stats(&self) -> &AiStats {
        &self.stats
    }

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
        self.follow_life_cycle::<V>(tick, hot.mobs, rows);
        let followed = Instant::now();
        let mut due = self.due();
        let mut tasks: Vec<TileTask<'_>> = Vec::new();
        for (tile, mobs) in rows.mob_tiles_mut() {
            tasks.push(TileTask {
                tile,
                mobs,
                due: due.get(&tile).map_or(&[], Vec::as_slice),
                players: &players,
                panic_causes: &self.panic_causes,
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

    fn follow_life_cycle<V>(&mut self, tick: u64, mobs: &Mobs, rows: &mut EntityRows)
    where
        B: for<'w> MobBrain<BlockGoalWorld<'w, V>>,
    {
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
            let rules = MobBrain::<BlockGoalWorld<'_, V>>::rules(&brain);
            let Some(column) = crate::BrainState::new(mob.kind, rules) else {
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

fn in_order<T>(groups: Vec<Vec<(usize, T)>>) -> Vec<T> {
    let mut outs: Vec<(usize, T)> = groups.into_iter().flatten().collect();
    outs.sort_unstable_by_key(|(index, _)| *index);
    outs.into_iter().map(|(_, out)| out).collect()
}
