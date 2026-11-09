use alloc::vec::Vec;

use crate::Cadence;
use crate::DecisionPipeline;
use crate::Fact;
use crate::IssuedDecisions;
use crate::MobView;
use crate::PlayerTable;
use crate::Replica;
use crate::Stage;
use crate::StageConfig;
use crate::TickContext;
use crate::TileWork;
use mcrs_minecraft_entity_key::EntityId;
use mcrs_minecraft_entity_key::EntityKey;
use mcrs_minecraft_entity_key::TileKey;

/// The AI of the mobs of one world, as a host drives it: the brains, the snapshot replica and the
/// decisions between their issue and their application. The driver keeps the order of the issue,
/// the collection, the ready mark, the due decisions and the retirement of old snapshots; the host
/// only lends its data and runs the tile tasks of the stage.
///
/// One AI tick of a host:
///
/// 1. [`AiDriver::begin_tick`];
/// 2. the life cycle: [`AiDriver::spawn`], [`AiDriver::relocate_all`] and [`AiDriver::despawn`];
/// 3. the snapshot: [`AiDriver::publish_tile`], [`AiDriver::publish_facts`] and
///    [`AiDriver::publish_players`];
/// 4. [`AiDriver::take_due`], before or after the stage, and the application of each due decision;
/// 5. [`AiDriver::run_stage`].
#[derive(Clone, Debug)]
pub struct AiDriver<B> {
    stage: Stage<B>,
    replica: Replica,
    pipeline: DecisionPipeline,
    tick: u64,
    waits: u64,
}

impl<B> AiDriver<B> {
    #[must_use]
    pub fn new(config: StageConfig) -> Self {
        Self {
            pipeline: DecisionPipeline::new(&config),
            stage: Stage::new(config),
            replica: Replica::default(),
            tick: 0,
            waits: 0,
        }
    }

    /// Starts the next AI tick and issues its work to the stage. The first tick is 1.
    pub fn begin_tick(&mut self) -> u64 {
        self.tick = self.tick.saturating_add(1);
        self.pipeline.issue(self.tick);
        self.tick
    }

    /// The last tick that [`AiDriver::begin_tick`] started: the clock of the cadences, the keyed
    /// random values and the damage records.
    #[must_use]
    pub const fn tick(&self) -> u64 {
        self.tick
    }

    #[must_use]
    pub const fn stage(&self) -> &Stage<B> {
        &self.stage
    }

    /// The ticks in which a due decision was not ready.
    #[must_use]
    pub const fn waits(&self) -> u64 {
        self.waits
    }

    /// Adds the brain of a mob that came into `tile`, as [`Stage::spawn`] does.
    pub fn spawn(
        &mut self,
        tile: TileKey,
        mob: EntityId,
        key: EntityKey,
        cadence: Cadence,
        brain: B,
    ) {
        self.stage.spawn(tile, mob, key, cadence, brain);
    }

    /// Removes the brain of a mob that left the world.
    pub fn despawn(&mut self, mob: EntityId) -> Option<B> {
        self.stage.despawn(mob)
    }

    /// Moves the brain of each mob of `moves` into its new tile.
    pub fn relocate_all(&mut self, moves: impl IntoIterator<Item = (EntityId, TileKey)>) {
        self.stage.relocate_all(moves);
    }

    /// Publishes the views of the mobs of `tile` at the start of this tick.
    pub fn publish_tile(&mut self, tile: TileKey, views: Vec<MobView>) {
        self.replica.publish_tile(self.tick, tile, views);
    }

    /// Publishes the facts of the ticks before this one for the mobs of `tile`.
    pub fn publish_facts(&mut self, tile: TileKey, facts: impl IntoIterator<Item = Fact>) {
        self.replica.publish_facts(self.tick, tile, facts);
    }

    pub fn publish_players(&mut self, players: PlayerTable) {
        self.replica.publish_players(self.tick, players);
    }

    /// The decisions that apply in this tick, oldest issue tick first. A tick whose due decisions
    /// are not ready counts as a wait and applies none.
    pub fn take_due(&mut self) -> Vec<IssuedDecisions> {
        match self.pipeline.take_due(self.tick) {
            Ok(due) => due,
            Err(_) => {
                self.waits = self.waits.saturating_add(1);
                Vec::new()
            }
        }
    }

    /// The stage of this tick over `world`, the blocks at the start of the tick. `run` runs each
    /// tile task once with [`TileWork::run`], on any threads and in any split; the result does not
    /// depend on either. Returns the count of decisions that the tick issued.
    pub fn run_stage<W: ?Sized>(
        &mut self,
        world: &W,
        run: impl FnOnce(&mut [TileWork<'_, B>], &TickContext<'_, W>),
    ) -> usize {
        let tick = self.tick;
        let cx = TickContext {
            tick,
            config: *self.stage.config(),
            replica: &self.replica,
            world,
        };
        let mut work = self.stage.work();
        run(&mut work, &cx);
        drop(work);
        let issued = self.stage.collect(tick);
        let decisions = issued.len();
        self.pipeline.ready(issued);
        self.replica.retire(tick.saturating_add(1));
        decisions
    }
}
