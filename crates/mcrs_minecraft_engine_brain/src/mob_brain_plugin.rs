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
use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_brain::AiDelay;
use mcrs_minecraft_brain::OrderKey;
use mcrs_minecraft_brain::PlayerFlags;
use mcrs_minecraft_brain::PlayerId;
use mcrs_minecraft_brain::PlayerRow;
use mcrs_minecraft_brain::Point;
use mcrs_minecraft_brain::StageConfig;
use mcrs_minecraft_brain::WorldSeed as StageSeed;
use mcrs_minecraft_engine::ability::PlayerGameMode;
use mcrs_minecraft_engine::world_seed::WorldSeed;
use mcrs_minecraft_engine_physics::EntityRows;
use mcrs_minecraft_engine_physics::Mobs;
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::tick::TickSet;
use mcrs_minecraft_engine_world::world::dimension::Dimension;
use mcrs_minecraft_engine_world::world::storage::view::WorldView;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_item::damage_type::DamageType;
use mcrs_minecraft_item::damage_type_tag_keys::PANIC_CAUSES;
use mcrs_minecraft_registries::CurrentRegistries;

use crate::BlockGoalWorld;
use crate::CellTable;
use crate::HotTick;
use crate::MobAi;
use crate::PanicCauses;

/// The eye height of a standing player.
const PLAYER_EYE_HEIGHT: f32 = 1.62;
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

/// The block cells and the damage types that the AI reads, from the block definitions and the
/// tags of the world.
#[derive(Resource, Debug)]
pub struct GoalCells {
    pub cells: CellTable,
    pub panic_causes: PanicCauses,
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
        let ready = resource_exists::<GoalCells>.and_then(resource_exists::<MobAi>);
        app.add_systems(
            FixedUpdate,
            (
                build_cells.run_if(
                    resource_exists::<Blocks>
                        .and_then(resource_exists_and_changed::<CurrentRegistries>),
                ),
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

fn build_cells(mut commands: Commands, blocks: Res<Blocks>, registries: Res<CurrentRegistries>) {
    let Some(tags) = registries.tags::<Block>() else {
        tracing::error!("the registries declare no block tags; the mobs keep their cells");
        return;
    };
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
    commands.insert_resource(GoalCells {
        cells: CellTable::from_definitions(&blocks, &tags),
        panic_causes,
    });
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

fn prepare(
    dimension: Single<Entity, With<Dimension>>,
    world: WorldView,
    cells: Res<GoalCells>,
    mobs: Res<Mobs>,
    players: Query<(Entity, &Transform, Option<&PlayerGameMode>), With<Player>>,
    mut rows: ResMut<EntityRows>,
    mut ai: ResMut<MobAi>,
    mut table: Local<Vec<PlayerRow>>,
) {
    let Some(blocks) = world.get(*dimension) else {
        return;
    };
    table.clear();
    table.extend(players.iter().map(|(entity, transform, mode)| {
        let spectator = mode.is_some_and(|mode| mode.0 == GameMode::Spectator);
        let position = transform.translation;
        PlayerRow {
            player: PlayerId(entity.to_bits()),
            key: OrderKey::new(0, u128::from(entity.to_bits())),
            position: Point::new(position.x, position.y, position.z),
            eye_height: PLAYER_EYE_HEIGHT,
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
        cells: &cells.cells,
        mobs: &mobs,
        panic_causes: &cells.panic_causes,
        players: &table,
    };
    ai.prepare(&hot, &mut rows, ComputeTaskPool::get());
}

fn run_stage(
    dimension: Single<Entity, With<Dimension>>,
    world: WorldView,
    cells: Res<GoalCells>,
    mut ai: ResMut<MobAi>,
) {
    let Some(blocks) = world.get(*dimension) else {
        return;
    };
    let goal_world = BlockGoalWorld::new(&blocks, &cells.cells);
    ai.run_stage(ComputeTaskPool::get(), &goal_world);
    let stats = ai.stats();
    if stats.mobs > 0 && ai.tick().is_multiple_of(LOG_PERIOD) {
        tracing::debug!(
            target: "mcrs_ai",
            tick = ai.tick(),
            mobs = stats.mobs,
            prepare_us = stats.prepare.as_micros(),
            stage_us = stats.stage.as_micros(),
            evaluations = stats.evaluations,
            decisions = stats.decisions,
            applied = stats.applied,
            waits = stats.waits,
            "mob AI"
        );
    }
}
