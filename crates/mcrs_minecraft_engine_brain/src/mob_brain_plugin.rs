use bevy_app::App;
use bevy_app::FixedUpdate;
use bevy_app::Plugin;
use bevy_ecs::entity::Entity;
use bevy_ecs::lifecycle::Add;
use bevy_ecs::lifecycle::Remove;
use bevy_ecs::observer::On;
use bevy_ecs::prelude::With;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::schedule::SystemCondition;
use bevy_ecs::schedule::SystemSet;
use bevy_ecs::schedule::common_conditions::resource_exists;
use bevy_ecs::schedule::common_conditions::resource_exists_and_changed;
use bevy_ecs::system::Commands;
use bevy_ecs::system::Local;
use bevy_ecs::system::Query;
use bevy_ecs::system::Res;
use bevy_ecs::system::ResMut;
use bevy_ecs::system::Single;
use bevy_tasks::ComputeTaskPool;
use bevy_utils::Parallel;
use mcrs_minecraft_brain::AiDelay;
use mcrs_minecraft_brain::MobBrain;
use mcrs_minecraft_brain::StageConfig;
use mcrs_minecraft_brain::WorldSeed as StageSeed;
use mcrs_minecraft_engine_damage::DamageSet;
use mcrs_minecraft_engine_entity_motion::MotionId;
use mcrs_minecraft_engine_entity_motion::MotionState;
use mcrs_minecraft_engine_physics::Mob;
use mcrs_minecraft_engine_physics::PhysicsTables;
use mcrs_minecraft_engine_world::tick::TickSet;
use mcrs_minecraft_engine_world::world::dimension::Dimension;
use mcrs_minecraft_engine_world::world::storage::view::DimensionView;
use mcrs_minecraft_engine_world::world::storage::view::WorldView;
use mcrs_minecraft_engine_world::world_seed::WorldSeed;
use mcrs_minecraft_entity::EntityTypeData;
use mcrs_minecraft_entity::damage_type::DamageType;
use mcrs_minecraft_entity_key::TileKey;
use mcrs_minecraft_registries::CurrentRegistries;
use mcrs_minecraft_resource::TagKey;

use crate::BlockGoalWorld;
use crate::BrainState;
use crate::MobAi;
use crate::ai_window::AiWindow;
use crate::mob_ai::Arrival;
use crate::mob_ai::BrainOut;
use crate::mob_ai::Thinking;
use crate::panic_causes_in::panic_causes_in;
use crate::player_table::Players;
use crate::player_table::player_table;

#[derive(Clone, Copy, Debug)]
pub struct MobBrainPlugin<B> {
    pub max_lag: u8,
    pub delay: AiDelay,
    pub brain_for: fn(&'static EntityTypeData) -> Option<B>,
    pub panic_causes: TagKey<'static, DamageType>,
}

impl<B> MobBrainPlugin<B> {
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
pub enum MobBrainSet {
    Prepare,
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
                    .after(DamageSet::PacketReport)
                    .before(TickSet::Simulate)
                    .run_if(ready.clone()),
                MobBrainSet::Stage.in_set(TickSet::Simulate).run_if(ready),
            ),
        );
        app.add_observer(admit_brain::<B>);
        app.add_observer(release_brain::<B>);
        app.add_systems(
            FixedUpdate,
            (
                set_panic_causes::<B>(self.panic_causes)
                    .run_if(
                        resource_exists::<MobAi<B>>
                            .and_then(resource_exists_and_changed::<CurrentRegistries>),
                    )
                    .after(TickSet::ClientInput)
                    .before(MobBrainSet::Prepare),
                prepare::<B>.in_set(MobBrainSet::Prepare),
                run_stage::<B>.in_set(MobBrainSet::Stage),
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
        app.insert_resource(MobAi::new(config, self.brain_for));
    }
}

fn admit_brain<B>(
    add: On<Add<Mob>>,
    mobs: Query<(&Mob, &MotionId, &MotionState)>,
    ai: Option<ResMut<MobAi<B>>>,
    mut commands: Commands,
) where
    B: for<'a, 'w> MobBrain<BlockGoalWorld<'a, DimensionView<'w>>> + Send + Sync + 'static,
{
    let entity = add.event().entity;
    let (Some(mut ai), Ok((mob, id, state))) = (ai, mobs.get(entity)) else {
        return;
    };
    if mob.looks.no_ai {
        return;
    }
    let Some(brain) = ai.brain_for(mob.kind) else {
        return;
    };
    let rules = MobBrain::<BlockGoalWorld<'_, DimensionView<'_>>>::rules(&brain);
    let Some(brain_state) = BrainState::new(mob.kind, rules) else {
        return;
    };
    ai.admit(Arrival {
        id: id.0,
        key: state.0.key,
        tile: TileKey::at(state.0.body.position),
        phase: state.0.phase,
        brain,
    });
    commands.entity(entity).insert(brain_state);
}

fn release_brain<B: Send + Sync + 'static>(
    remove: On<Remove<BrainState>>,
    ids: Query<&MotionId>,
    ai: Option<ResMut<MobAi<B>>>,
) {
    if let (Some(mut ai), Ok(id)) = (ai, ids.get(remove.event().entity)) {
        ai.release(id.0);
    }
}

fn set_panic_causes<B: Send + Sync + 'static>(
    panic_causes: TagKey<'static, DamageType>,
) -> impl FnMut(Res<CurrentRegistries>, ResMut<MobAi<B>>) {
    move |registries, mut ai| {
        ai.set_panic_causes(panic_causes_in(&registries, panic_causes));
    }
}

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::blocking_in_system,
        reason = "the wall clock measures the AI phases for the periodic mob AI profile log"
    )
)]
#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::alloc_in_tick,
        reason = "the tile tasks borrow the rows of this tick and move to the compute pool, so their lists cannot live in a reused buffer"
    )
)]
#[cfg_attr(mcrs_lint, mcrs::alloc_sites(2))]
fn prepare<B>(
    dimension: Single<Entity, With<Dimension>>,
    world: WorldView,
    tables: Res<PhysicsTables>,
    players: Players,
    mut mobs: Thinking,
    mut ai: ResMut<MobAi<B>>,
    mut threads: Local<Parallel<BrainOut>>,
) where
    B: for<'a, 'w> MobBrain<BlockGoalWorld<'a, DimensionView<'w>>> + Send + Sync + 'static,
{
    let Some(blocks) = world.get(*dimension) else {
        return;
    };
    let goal_world = BlockGoalWorld::new(&blocks, tables.collision());
    ai.prepare(
        &goal_world,
        player_table(&players),
        mobs.reborrow(),
        &mut threads,
    );
}

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::blocking_in_system,
        reason = "the wall clock measures the AI stage for the periodic mob AI profile log"
    )
)]
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
    window.add(ai.tick(), ai.stats());
}
