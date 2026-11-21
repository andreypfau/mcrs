use bevy_app::App;
use bevy_app::FixedUpdate;
use bevy_app::Plugin;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::With;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::schedule::SystemCondition;
use bevy_ecs::schedule::SystemSet;
use bevy_ecs::schedule::common_conditions::resource_exists;
use bevy_ecs::schedule::common_conditions::resource_exists_and_changed;
use bevy_ecs::system::Local;
use bevy_ecs::system::Res;
use bevy_ecs::system::ResMut;
use bevy_ecs::system::Single;
use bevy_tasks::ComputeTaskPool;
use mcrs_minecraft_brain::AiDelay;
use mcrs_minecraft_brain::MobBrain;
use mcrs_minecraft_brain::StageConfig;
use mcrs_minecraft_brain::WorldSeed as StageSeed;
use mcrs_minecraft_engine_physics::Mobs;
use mcrs_minecraft_engine_physics::PhysicsTables;
use mcrs_minecraft_engine_world::tick::TickSet;
use mcrs_minecraft_engine_world::world::dimension::Dimension;
use mcrs_minecraft_engine_world::world::storage::view::DimensionView;
use mcrs_minecraft_engine_world::world::storage::view::WorldView;
use mcrs_minecraft_engine_world::world_seed::WorldSeed;
use mcrs_minecraft_entity::EntityTypeData;
use mcrs_minecraft_entity::damage_type::DamageType;
use mcrs_minecraft_registries::CurrentRegistries;
use mcrs_minecraft_resource::TagKey;

use crate::BlockGoalWorld;
use crate::EntityRows;
use crate::HotTick;
use crate::RowMobAi;
use crate::ai_window::AiWindow;
use crate::panic_causes_in::panic_causes_in;
use crate::player_table::Players;
use crate::player_table::player_table;

#[derive(Clone, Copy, Debug)]
pub struct RowBrainPlugin<B> {
    pub max_lag: u8,
    pub delay: AiDelay,
    pub brain_for: fn(&'static EntityTypeData) -> Option<B>,
    pub panic_causes: TagKey<'static, DamageType>,
}

impl<B> RowBrainPlugin<B> {
    #[must_use]
    pub fn new(
        brain_for: fn(&'static EntityTypeData) -> Option<B>,
        panic_causes: TagKey<'static, DamageType>,
    ) -> Self {
        Self {
            max_lag: StageConfig::DEFAULT_MAX_LAG,
            delay: AiDelay::AtLeast(1),
            brain_for,
            panic_causes,
        }
    }
}

#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RowBrainSet {
    Prepare,
    Stage,
}

impl<B> Plugin for RowBrainPlugin<B>
where
    B: for<'a, 'w> MobBrain<BlockGoalWorld<'a, DimensionView<'w>>> + Send + Sync + 'static,
{
    fn build(&self, app: &mut App) {
        let ready = resource_exists::<PhysicsTables>.and_then(resource_exists::<RowMobAi<B>>);
        app.configure_sets(
            FixedUpdate,
            (
                RowBrainSet::Prepare
                    .after(TickSet::ClientInput)
                    .before(TickSet::Simulate)
                    .run_if(ready.clone()),
                RowBrainSet::Stage.in_set(TickSet::Simulate).run_if(ready),
            ),
        );
        app.add_systems(
            FixedUpdate,
            (
                set_panic_causes::<B>(self.panic_causes)
                    .run_if(
                        resource_exists::<RowMobAi<B>>
                            .and_then(resource_exists_and_changed::<CurrentRegistries>),
                    )
                    .after(TickSet::ClientInput)
                    .before(RowBrainSet::Prepare),
                prepare::<B>.in_set(RowBrainSet::Prepare),
                run_stage::<B>.in_set(RowBrainSet::Stage),
            ),
        );
    }

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
        app.insert_resource(RowMobAi::new(config, self.brain_for));
    }
}

fn set_panic_causes<B: Send + Sync + 'static>(
    panic_causes: TagKey<'static, DamageType>,
) -> impl FnMut(Res<CurrentRegistries>, ResMut<RowMobAi<B>>) {
    move |registries, mut ai| {
        ai.set_panic_causes(panic_causes_in(&registries, panic_causes));
    }
}

fn prepare<B>(
    dimension: Single<Entity, With<Dimension>>,
    world: WorldView,
    tables: Res<PhysicsTables>,
    mobs: Res<Mobs>,
    players: Players,
    mut rows: ResMut<EntityRows>,
    mut ai: ResMut<RowMobAi<B>>,
) where
    B: for<'a, 'w> MobBrain<BlockGoalWorld<'a, DimensionView<'w>>> + Send + Sync + 'static,
{
    let Some(blocks) = world.get(*dimension) else {
        return;
    };
    let table = player_table(&players);
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
    mut ai: ResMut<RowMobAi<B>>,
    mut window: Local<AiWindow>,
) where
    B: for<'a, 'w> MobBrain<BlockGoalWorld<'a, DimensionView<'w>>> + Send + Sync + 'static,
{
    let Some(blocks) = world.get(*dimension) else {
        return;
    };
    let goal_world = BlockGoalWorld::new(&blocks, tables.collision());
    ai.run_stage(ComputeTaskPool::get(), &goal_world);
    window.add(ai.tick(), ai.stats());
}
