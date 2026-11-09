use std::mem;
use std::time::Duration;

use bevy_app::App;
use bevy_app::FixedUpdate;
use bevy_app::Plugin;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::With;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::schedule::SystemCondition;
use bevy_ecs::schedule::SystemSet;
use bevy_ecs::schedule::common_conditions::resource_exists;
use bevy_ecs::system::Local;
use bevy_ecs::system::Query;
use bevy_ecs::system::Res;
use bevy_ecs::system::ResMut;
use bevy_ecs::system::Single;
use bevy_tasks::ComputeTaskPool;
use mcrs_minecraft_brain::AiDelay;
use mcrs_minecraft_brain::MobBrain;
use mcrs_minecraft_brain::PlayerFlags;
use mcrs_minecraft_brain::PlayerRow;
use mcrs_minecraft_brain::PlayerTable;
use mcrs_minecraft_brain::StageConfig;
use mcrs_minecraft_brain::WorldSeed as StageSeed;
use mcrs_minecraft_engine::ability::PlayerGameMode;
use mcrs_minecraft_engine::network_id::player_network_id;
use mcrs_minecraft_engine_physics::Mobs;
use mcrs_minecraft_engine_physics::PhysicsTables;
use mcrs_minecraft_engine_world::entity::EntityUuid;
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::session::Owner;
use mcrs_minecraft_engine_world::tick::TickSet;
use mcrs_minecraft_engine_world::world::dimension::Dimension;
use mcrs_minecraft_engine_world::world::storage::view::DimensionView;
use mcrs_minecraft_engine_world::world::storage::view::WorldView;
use mcrs_minecraft_engine_world::world_seed::WorldSeed;
use mcrs_minecraft_entity::EntityTypeData;
use mcrs_minecraft_entity_key::EntityId;
use mcrs_minecraft_entity_key::EntityKey;
use mcrs_minecraft_game_mode::GameMode;

use crate::AiStats;
use crate::BlockGoalWorld;
use crate::EntityRows;
use crate::HotTick;
use crate::MobAi;

/// The ticks between two log lines of the AI cost.
const LOG_PERIOD: u64 = 200;

/// The AI of the mobs of a dimension world with the brains `B`: the hot side between the client
/// input and the entity pass, and the stage in `Simulate`, beside the pass, on the compute pool.
/// The plugin inserts a [`MobAi<B>`] with no panic causes; the application sets them through
/// [`MobAi::set_panic_causes`] before [`MobBrainSet::Prepare`].
#[derive(Clone, Copy, Debug)]
pub struct MobBrainPlugin<B> {
    pub max_lag: u8,
    pub delay: AiDelay,
    /// The brain of a mob that comes into the world, from its type; `None` for a type without AI.
    pub brain_for: fn(&'static EntityTypeData) -> Option<B>,
}

impl<B> MobBrainPlugin<B> {
    /// The AI with the default lag and delay, in which each mob without `NoAI` gets the brain that
    /// `brain_for` makes for its type.
    #[must_use]
    pub fn new(brain_for: fn(&'static EntityTypeData) -> Option<B>) -> Self {
        Self {
            max_lag: StageConfig::DEFAULT_MAX_LAG,
            delay: AiDelay::AtLeast(1),
            brain_for,
        }
    }
}

/// The steps of the AI in a tick.
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MobBrainSet {
    /// The snapshot, the application of the due decisions and the intents, after the client
    /// input and before the entity pass.
    Prepare,
    /// The stage of the tick, beside the entity pass.
    Stage,
}

impl<B> Plugin for MobBrainPlugin<B>
where
    B: for<'a, 'w> MobBrain<BlockGoalWorld<'a, DimensionView<'w>>> + Send + Sync + 'static,
{
    fn build(&self, app: &mut App) {
        let ready = resource_exists::<PhysicsTables>.and_then(resource_exists::<MobAi<B>>);
        app.configure_sets(
            FixedUpdate,
            (
                MobBrainSet::Prepare
                    .after(TickSet::ClientInput)
                    .before(TickSet::Simulate)
                    .run_if(ready.clone()),
                MobBrainSet::Stage.in_set(TickSet::Simulate).run_if(ready),
            ),
        );
        app.add_systems(
            FixedUpdate,
            (
                prepare::<B>.in_set(MobBrainSet::Prepare),
                run_stage::<B>.in_set(MobBrainSet::Stage),
            ),
        );
    }

    /// A dimension world has its [`WorldSeed`] before its plugins build; a world without one
    /// keys the AI by the seed 0.
    fn finish(&self, app: &mut App) {
        let seed = StageSeed(
            app.world()
                .get_resource::<WorldSeed>()
                .map_or(0, |seed| seed.0),
        );
        let config = match StageConfig::new(seed).with_delay(self.max_lag, self.delay) {
            Ok(config) => config,
            Err(error) => {
                tracing::error!(%error, "the AI keeps its default delay");
                StageConfig::new(seed)
            }
        };
        app.insert_resource(MobAi::new(config, self.brain_for));
    }
}

fn prepare<B>(
    dimension: Single<Entity, With<Dimension>>,
    world: WorldView,
    tables: Res<PhysicsTables>,
    mobs: Res<Mobs>,
    players: Query<(&Owner, &EntityUuid, &Transform, Option<&PlayerGameMode>), With<Player>>,
    mut rows: ResMut<EntityRows>,
    mut ai: ResMut<MobAi<B>>,
) where
    B: for<'a, 'w> MobBrain<BlockGoalWorld<'a, DimensionView<'w>>> + Send + Sync + 'static,
{
    let Some(blocks) = world.get(*dimension) else {
        return;
    };
    let table = PlayerTable::new(players.iter().map(|(owner, uuid, transform, mode)| {
        let spectator = mode.is_some_and(|mode| mode.0 == GameMode::Spectator);
        PlayerRow {
            player: EntityId(u64::from(player_network_id(owner.0).cast_unsigned())),
            key: EntityKey(uuid.0.as_u128()),
            position: transform.translation,
            eye_height: EntityTypeData::PLAYER.eye_height,
            flags: if spectator {
                PlayerFlags::ALIVE.union(PlayerFlags::SPECTATOR)
            } else {
                PlayerFlags::ALIVE
            },
            visibility: 1.0,
            vehicle: None,
        }
    }));
    let hot = HotTick {
        blocks: &blocks,
        collision: tables.collision(),
        mobs: &mobs,
    };
    ai.prepare(&hot, table, &mut rows, ComputeTaskPool::get());
}

fn run_stage<B>(
    dimension: Single<Entity, With<Dimension>>,
    world: WorldView,
    tables: Res<PhysicsTables>,
    mut ai: ResMut<MobAi<B>>,
    mut window: Local<AiWindow>,
) where
    B: for<'a, 'w> MobBrain<BlockGoalWorld<'a, DimensionView<'w>>> + Send + Sync + 'static,
{
    let Some(blocks) = world.get(*dimension) else {
        return;
    };
    let goal_world = BlockGoalWorld::new(&blocks, tables.collision());
    ai.run_stage(ComputeTaskPool::get(), &goal_world);
    window.add(ai.stats());
    if ai.tick().is_multiple_of(LOG_PERIOD) {
        let window = mem::take(&mut *window);
        if ai.stats().mobs > 0 && window.ticks > 0 {
            tracing::debug!(
                target: "mcrs_ai",
                tick = ai.tick(),
                mobs = ai.stats().mobs,
                prepare_mean_us = window.prepare.as_micros() / u128::from(window.ticks),
                prepare_max_us = window.prepare_max.as_micros(),
                life_cycle_mean_us = window.life_cycle.as_micros() / u128::from(window.ticks),
                tile_tasks_mean_us = window.tile_tasks.as_micros() / u128::from(window.ticks),
                publish_mean_us = window.publish.as_micros() / u128::from(window.ticks),
                stage_mean_us = window.stage.as_micros() / u128::from(window.ticks),
                stage_max_us = window.stage_max.as_micros(),
                evaluations_per_tick = window.evaluations / window.ticks,
                decisions_per_tick = window.decisions / window.ticks,
                waits = ai.stats().waits,
                "mob AI"
            );
        }
    }
}

/// The sums of the AI stats over the ticks since the last log line.
#[derive(Debug, Default)]
struct AiWindow {
    ticks: u64,
    prepare: Duration,
    prepare_max: Duration,
    life_cycle: Duration,
    tile_tasks: Duration,
    publish: Duration,
    stage: Duration,
    stage_max: Duration,
    evaluations: u64,
    decisions: u64,
}

impl AiWindow {
    fn add(&mut self, stats: &AiStats) {
        self.ticks = self.ticks.saturating_add(1);
        self.prepare = self.prepare.saturating_add(stats.prepare);
        self.prepare_max = self.prepare_max.max(stats.prepare);
        self.life_cycle = self.life_cycle.saturating_add(stats.life_cycle);
        self.tile_tasks = self.tile_tasks.saturating_add(stats.tile_tasks);
        self.publish = self.publish.saturating_add(stats.publish);
        self.stage = self.stage.saturating_add(stats.stage);
        self.stage_max = self.stage_max.max(stats.stage);
        self.evaluations = self.evaluations.saturating_add(stats.evaluations);
        self.decisions = self
            .decisions
            .saturating_add(u64::try_from(stats.decisions).unwrap_or(u64::MAX));
    }
}
