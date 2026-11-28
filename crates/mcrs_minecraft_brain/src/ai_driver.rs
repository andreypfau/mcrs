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
use crate::StageRun;
use crate::TickContext;
use crate::TileBrains;
use crate::TileWork;
use mcrs_minecraft_entity_key::EntityId;
use mcrs_minecraft_entity_key::EntityUuid;
use mcrs_minecraft_entity_key::TileKey;

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

    pub fn begin_tick(&mut self) -> u64 {
        self.tick = self.tick.saturating_add(1);
        self.pipeline.issue(self.tick);
        self.tick
    }

    #[must_use]
    pub const fn tick(&self) -> u64 {
        self.tick
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.stage.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.stage.is_empty()
    }

    #[must_use]
    pub fn tile_of(&self, mob: EntityId) -> Option<TileKey> {
        self.stage.tile_of(mob)
    }

    #[must_use]
    pub fn evaluations(&self) -> u64 {
        self.stage.evaluations()
    }

    pub fn tiles(&self) -> impl Iterator<Item = (TileKey, &TileBrains<B>)> {
        self.stage.tiles()
    }

    #[must_use]
    pub const fn waits(&self) -> u64 {
        self.waits
    }

    pub fn spawn(
        &mut self,
        tile: TileKey,
        mob: EntityId,
        key: EntityUuid,
        cadence: Cadence,
        brain: B,
    ) {
        self.stage.spawn(tile, mob, key, cadence, brain);
    }

    pub fn despawn(&mut self, mob: EntityId) -> Option<B> {
        self.stage.despawn(mob)
    }

    pub fn relocate_all(&mut self, moves: impl IntoIterator<Item = (EntityId, TileKey)>) {
        self.stage.relocate_all(moves);
    }

    pub fn publish_tile(&mut self, tile: TileKey, views: Vec<MobView>) {
        self.replica.publish_tile(self.tick, tile, views);
    }

    pub fn publish_facts(&mut self, tile: TileKey, facts: impl IntoIterator<Item = Fact>) {
        self.replica.publish_facts(self.tick, tile, facts);
    }

    pub fn publish_players(&mut self, players: PlayerTable) {
        self.replica.publish_players(self.tick, players);
    }

    pub fn take_due(&mut self) -> Vec<IssuedDecisions> {
        match self.pipeline.take_due(self.tick) {
            Ok(due) => due,
            Err(_) => {
                self.waits = self.waits.saturating_add(1);
                Vec::new()
            }
        }
    }

    pub fn run_stage<W: ?Sized>(
        &mut self,
        world: &W,
        run: impl FnOnce(&mut [TileWork<'_, B>], &TickContext<'_, W>),
    ) -> StageRun {
        let tick = self.tick;
        let evaluations_before = self.stage.evaluations();
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
        StageRun {
            decisions,
            evaluations: self.stage.evaluations().saturating_sub(evaluations_before),
        }
    }
}
