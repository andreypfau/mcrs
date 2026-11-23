use std::time::Instant;

use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::Query;
use bevy_ecs::world::Mut;
use bevy_tasks::TaskPool;
use bevy_utils::Parallel;
use mcrs_minecraft_brain::AiDriver;
use mcrs_minecraft_brain::Cadence;
use mcrs_minecraft_brain::Decision;
use mcrs_minecraft_brain::Fact;
use mcrs_minecraft_brain::FactKind;
use mcrs_minecraft_brain::MobBrain;
use mcrs_minecraft_brain::MobView;
use mcrs_minecraft_brain::PanicCauses;
use mcrs_minecraft_brain::PlayerTable;
use mcrs_minecraft_brain::Stage;
use mcrs_minecraft_brain::StageConfig;
use mcrs_minecraft_damage::HitRecord;
use mcrs_minecraft_damage::Vitals;
use mcrs_minecraft_engine_entity_motion::MobIntent;
use mcrs_minecraft_engine_entity_motion::MotionId;
use mcrs_minecraft_engine_entity_motion::MotionState;
use mcrs_minecraft_entity::EntityTypeData;
use mcrs_minecraft_entity_key::EntityId;
use mcrs_minecraft_entity_key::EntityUuid;
use mcrs_minecraft_entity_key::TileKey;
use mcrs_minecraft_goal_world::GoalWorld;
use mcrs_minecraft_volume::BlockVolume;

use crate::AiStats;
use crate::BlockGoalWorld;
use crate::BrainState;
use crate::brain_state_ext::BrainStateExt;
use crate::run_tiles::run_tiles;
use crate::sensed::Sensed;

pub(crate) type Thinking<'w, 's> = Query<
    'w,
    's,
    (
        &'static MotionId,
        &'static MotionState,
        Option<&'static Vitals>,
        Option<&'static HitRecord>,
        &'static mut BrainState,
        &'static mut MobIntent,
    ),
>;

#[derive(Resource, Debug)]
pub struct MobAi<B: Send + Sync + 'static> {
    driver: AiDriver<B>,
    brain_for: fn(&'static EntityTypeData) -> Option<B>,
    panic_causes: PanicCauses,
    arrivals: Vec<Arrival<B>>,
    departures: Vec<EntityId>,
    due: Vec<Decision>,
    ended: Vec<EntityId>,
    gathered: BrainOut,
    facts: Vec<TiledFact>,
    stats: AiStats,
}

#[derive(Debug)]
pub(crate) struct Arrival<B> {
    pub(crate) id: EntityId,
    pub(crate) key: EntityUuid,
    pub(crate) tile: TileKey,
    pub(crate) phase: u8,
    pub(crate) brain: B,
}

#[derive(Debug, Default)]
pub(crate) struct BrainOut {
    views: Vec<MobView>,
    facts: Vec<Fact>,
    ended: Vec<EntityId>,
    moves: Vec<Move>,
    applied: usize,
}

#[derive(Clone, Copy, Debug)]
struct Move {
    mob: EntityId,
    tile: TileKey,
}

#[derive(Clone, Copy, Debug)]
struct TiledFact {
    tile: TileKey,
    fact: Fact,
}

struct Pass<'a, B, V> {
    tick: u64,
    players: &'a PlayerTable,
    panic_causes: &'a PanicCauses,
    due: &'a [Decision],
    stage: &'a Stage<B>,
    world: &'a BlockGoalWorld<'a, V>,
}

impl<B: Send + Sync + 'static> MobAi<B> {
    #[must_use]
    pub fn new(config: StageConfig, brain_for: fn(&'static EntityTypeData) -> Option<B>) -> Self {
        Self {
            driver: AiDriver::new(config),
            brain_for,
            panic_causes: PanicCauses::default(),
            arrivals: Vec::new(),
            departures: Vec::new(),
            due: Vec::new(),
            ended: Vec::new(),
            gathered: BrainOut::default(),
            facts: Vec::new(),
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

    pub(crate) fn brain_for(&self, kind: &'static EntityTypeData) -> Option<B> {
        (self.brain_for)(kind)
    }

    pub(crate) fn admit(&mut self, arrival: Arrival<B>) {
        self.arrivals.push(arrival);
    }

    pub(crate) fn release(&mut self, mob: EntityId) {
        self.arrivals.retain(|arrival| arrival.id != mob);
        self.departures.push(mob);
    }

    pub(crate) fn prepare<V: BlockVolume + Sync>(
        &mut self,
        world: &BlockGoalWorld<'_, V>,
        players: PlayerTable,
        mobs: &mut Thinking<'_, '_>,
        threads: &mut Parallel<BrainOut>,
    ) {
        let started = Instant::now();
        let tick = self.driver.begin_tick();
        self.follow_life_cycle(tick);
        let followed = Instant::now();
        self.take_due();
        let pass = Pass {
            tick,
            players: &players,
            panic_causes: &self.panic_causes,
            due: &self.due,
            stage: self.driver.stage(),
            world,
        };
        let shared = &*threads;
        mobs.par_iter_mut().for_each_init(
            || shared.borrow_local_mut(),
            |out, (id, state, vitals, hits, brain, intent)| {
                let sensed = Sensed {
                    state: &state.0,
                    vitals,
                    hits,
                };
                pass.think(out, id.0, sensed, brain, intent);
            },
        );
        let ran = Instant::now();
        self.publish(threads, players);
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

    fn follow_life_cycle(&mut self, tick: u64) {
        for mob in self.departures.drain(..) {
            self.driver.despawn(mob);
        }
        self.arrivals.sort_unstable_by_key(|arrival| arrival.key);
        for arrival in self.arrivals.drain(..) {
            if self.driver.stage().tile_of(arrival.id).is_some() {
                continue;
            }
            let cadence = Cadence::new(tick, arrival.phase);
            self.driver.spawn(
                arrival.tile,
                arrival.id,
                arrival.key,
                cadence,
                arrival.brain,
            );
        }
        self.stats.mobs = self.driver.stage().len();
    }

    fn take_due(&mut self) {
        self.due.clear();
        let issued = self.driver.take_due();
        self.stats.waits = self.driver.waits();
        for decisions in issued {
            for decision in decisions.tiles.into_values().flatten() {
                if self.driver.stage().tile_of(decision.mob).is_some() {
                    self.due.push(decision);
                }
            }
        }
        self.due
            .sort_by_key(|decision| (decision.key, decision.issue_tick));
    }

    fn publish(&mut self, threads: &mut Parallel<BrainOut>, players: PlayerTable) {
        let gathered = &mut self.gathered;
        let mut applied: usize = 0;
        for out in threads.iter_mut() {
            gathered.views.append(&mut out.views);
            gathered.facts.append(&mut out.facts);
            gathered.ended.append(&mut out.ended);
            gathered.moves.append(&mut out.moves);
            applied = applied.saturating_add(out.applied);
            out.applied = 0;
        }
        gathered.moves.sort_unstable_by_key(|moved| moved.mob);
        self.driver.relocate_all(
            gathered
                .moves
                .drain(..)
                .map(|moved| (moved.mob, moved.tile)),
        );
        gathered
            .views
            .sort_unstable_by_key(|view| (TileKey::at(view.position), view.key));
        for views in gathered
            .views
            .chunk_by(|a, b| TileKey::at(a.position) == TileKey::at(b.position))
        {
            if let Some(first) = views.first() {
                self.driver
                    .publish_tile(TileKey::at(first.position), views.to_vec());
            }
        }
        gathered.views.clear();
        self.facts.clear();
        let ended = self.ended.drain(..).map(|mob| Fact {
            mob,
            kind: FactKind::PathEnded,
        });
        for fact in ended.chain(gathered.facts.drain(..)) {
            if let Some(tile) = self.driver.stage().tile_of(fact.mob) {
                self.facts.push(TiledFact { tile, fact });
            }
        }
        self.facts.sort_by_key(|tiled| (tiled.tile, tiled.fact.mob));
        for facts in self.facts.chunk_by(|a, b| a.tile == b.tile) {
            if let Some(first) = facts.first() {
                self.driver
                    .publish_facts(first.tile, facts.iter().map(|tiled| tiled.fact));
            }
        }
        gathered.ended.sort_unstable();
        self.ended.append(&mut gathered.ended);
        self.driver.publish_players(players);
        self.stats.applied = applied;
    }
}

impl<B, V: BlockVolume> Pass<'_, B, V> {
    fn think(
        &self,
        out: &mut BrainOut,
        id: EntityId,
        sensed: Sensed<'_>,
        mut brain: Mut<'_, BrainState>,
        mut intent: Mut<'_, MobIntent>,
    ) {
        let mut next = *brain;
        let state = sensed.state;
        let observed = next.observe(self.panic_causes, self.players, self.tick, id, sensed);
        let kinds = [
            (observed.struck, FactKind::Hurt),
            (observed.idle_reset, FactKind::IdleReset),
            (observed.resumed, FactKind::Resumed),
        ];
        for (set, kind) in kinds {
            if set {
                out.facts.push(Fact { mob: id, kind });
            }
        }
        out.views.push(observed.view);
        let first = self
            .due
            .partition_point(|decision| decision.key < state.key);
        let due = self.due.get(first..).unwrap_or_default();
        for decision in due.iter().take_while(|decision| decision.key == state.key) {
            if decision.mob == id {
                next.apply(&decision.intents, state);
                out.applied = out.applied.saturating_add(1);
            }
        }
        if let Some((steered, ended)) = next.steer(self.world, self.tick, sensed) {
            intent.set_if_neq(steered);
            if ended.is_some() {
                out.ended.push(id);
            }
        }
        brain.set_if_neq(next);
        let tile = TileKey::at(state.body.position);
        if self.stage.tile_of(id) != Some(tile) {
            out.moves.push(Move { mob: id, tile });
        }
    }
}
