use std::mem;
use std::time::Duration;

use bevy_app::App;
use bevy_app::FixedUpdate;
use bevy_app::Plugin;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::With;
use bevy_ecs::resource::Resource;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::schedule::SystemCondition;
use bevy_ecs::schedule::SystemSet;
use bevy_ecs::schedule::common_conditions::not;
use bevy_ecs::schedule::common_conditions::resource_exists;
use bevy_ecs::schedule::common_conditions::resource_exists_and_changed;
use bevy_ecs::system::Commands;
use bevy_ecs::system::Local;
use bevy_ecs::system::Query;
use bevy_ecs::system::Res;
use bevy_ecs::system::ResMut;
use bevy_ecs::system::Single;
use bevy_tasks::ComputeTaskPool;
use mcrs_minecraft_brain::AiDelay;
use mcrs_minecraft_brain::OrderKey;
use mcrs_minecraft_brain::PlayerFlags;
use mcrs_minecraft_brain::PlayerId;
use mcrs_minecraft_brain::PlayerRow;
use mcrs_minecraft_brain::StageConfig;
use mcrs_minecraft_brain::WorldSeed as StageSeed;
use mcrs_minecraft_engine::ability::PlayerGameMode;
use mcrs_minecraft_engine::world_seed::WorldSeed;
use mcrs_minecraft_engine_physics::EntityRows;
use mcrs_minecraft_engine_physics::Mobs;
use mcrs_minecraft_engine_physics::PhysicsTables;
use mcrs_minecraft_engine_world::entity::EntityUuid;
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::tick::TickSet;
use mcrs_minecraft_engine_world::world::dimension::Dimension;
use mcrs_minecraft_engine_world::world::storage::view::WorldView;
use mcrs_minecraft_entity::EntityTypeData;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_item::damage_type::DamageType;
use mcrs_minecraft_item::damage_type_tag_keys::PANIC_CAUSES;
use mcrs_minecraft_registries::CurrentRegistries;

use crate::AiStats;
use crate::BlockGoalWorld;
use crate::HotTick;
use crate::MobAi;
use crate::PanicCauses;

/// The ticks between two log lines of the AI cost.
const LOG_PERIOD: u64 = 200;

/// The AI of the pigs of a dimension world: the hot side between the client input and the entity
/// pass, and the stage in `Simulate`, beside the pass, on the compute pool.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MobBrainPlugin {
    pub max_lag: u8,
    pub delay: AiDelay,
}

impl Default for MobBrainPlugin {
    fn default() -> Self {
        Self {
            max_lag: StageConfig::DEFAULT_MAX_LAG,
            delay: AiDelay::AtLeast(1),
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

/// The configuration of the stage until the first tick inserts [`MobAi`].
#[derive(Resource, Clone, Copy, Debug)]
struct StageSettings(MobBrainPlugin);

impl Plugin for MobBrainPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(StageSettings(*self));
        app.configure_sets(
            FixedUpdate,
            (
                MobBrainSet::Prepare
                    .after(TickSet::ClientInput)
                    .before(TickSet::Simulate),
                MobBrainSet::Stage.in_set(TickSet::Simulate),
            ),
        );
        let ready = resource_exists::<PanicCauses>
            .and_then(resource_exists::<PhysicsTables>)
            .and_then(resource_exists::<MobAi>);
        app.add_systems(
            FixedUpdate,
            (
                build_panic_causes.run_if(resource_exists_and_changed::<CurrentRegistries>),
                start.run_if(not(resource_exists::<MobAi>)),
                prepare.run_if(ready.clone()),
            )
                .chain()
                .in_set(MobBrainSet::Prepare),
        );
        app.add_systems(
            FixedUpdate,
            run_stage.run_if(ready).in_set(MobBrainSet::Stage),
        );
    }
}

fn build_panic_causes(mut commands: Commands, registries: Res<CurrentRegistries>) {
    let panic_causes = registries
        .tags::<DamageType>()
        .and_then(|tags| {
            let tag = tags.id(PANIC_CAUSES)?;
            Some(PanicCauses::new(
                tags.members(tag)
                    .filter_map(|id| u32::try_from(id.index()).ok()),
            ))
        })
        .unwrap_or_default();
    commands.insert_resource(panic_causes);
}

fn start(mut commands: Commands, settings: Res<StageSettings>, seed: Option<Res<WorldSeed>>) {
    let seed = StageSeed(seed.map_or(0, |seed| seed.0));
    let StageSettings(plugin) = *settings;
    let config = match StageConfig::new(seed).with_delay(plugin.max_lag, plugin.delay) {
        Ok(config) => config,
        Err(error) => {
            tracing::error!(%error, "the AI keeps its default delay");
            StageConfig::new(seed)
        }
    };
    commands.insert_resource(MobAi::new(config));
}

#[expect(
    clippy::too_many_arguments,
    reason = "a system names each resource and query that it reads"
)]
fn prepare(
    dimension: Single<Entity, With<Dimension>>,
    world: WorldView,
    tables: Res<PhysicsTables>,
    panic_causes: Res<PanicCauses>,
    mobs: Res<Mobs>,
    players: Query<(Entity, &EntityUuid, &Transform, Option<&PlayerGameMode>), With<Player>>,
    mut rows: ResMut<EntityRows>,
    mut ai: ResMut<MobAi>,
    mut table: Local<Vec<PlayerRow>>,
) {
    let Some(blocks) = world.get(*dimension) else {
        return;
    };
    table.clear();
    table.extend(players.iter().map(|(entity, uuid, transform, mode)| {
        let spectator = mode.is_some_and(|mode| mode.0 == GameMode::Spectator);
        PlayerRow {
            player: PlayerId(entity.to_bits()),
            key: OrderKey::new(uuid.0.as_u128()),
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
        panic_causes: &panic_causes,
        players: &table,
    };
    ai.prepare(&hot, &mut rows, ComputeTaskPool::get());
}

fn run_stage(
    dimension: Single<Entity, With<Dimension>>,
    world: WorldView,
    tables: Res<PhysicsTables>,
    mut ai: ResMut<MobAi>,
    mut window: Local<AiWindow>,
) {
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
