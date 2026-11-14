use bevy_app::App;
use bevy_app::FixedUpdate;
use bevy_app::Plugin;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::schedule::SystemSet;
use bevy_ecs::schedule::common_conditions::any_with_component;
use bevy_ecs::schedule::common_conditions::on_message;
use bevy_ecs::schedule::common_conditions::resource_exists;
use bevy_ecs::schedule::common_conditions::resource_exists_and_changed;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_engine::column_change::ColumnChange;
use mcrs_minecraft_engine_entity_motion::MotionPass;
use mcrs_minecraft_engine_world::block_update::BlockSetRequest;
use mcrs_minecraft_engine_world::tick::TickSet;
use mcrs_minecraft_falling_block::Settings;
use mcrs_minecraft_item::Items;
use mcrs_minecraft_registries::CurrentRegistries;

use crate::FallingBlockEntity;
use crate::Landed;
use crate::PackConcretePairs;
use crate::PendingFall;
use crate::apply_landings::apply_landings;
use crate::begin_falls::begin_falls;
use crate::begin_ticks::begin_ticks;
use crate::decide_falls::decide_falls;
use crate::fall_request::FallRequest;
use crate::fall_request::propose_falls;
use crate::falling_block_rules::FallingBlockConfig;
use crate::falling_block_rules::FallingBlockRules;
use crate::falling_block_rules::build_rules;

/// The falling block in a dimension world: each [`FallRequest`] becomes a falling block that moves
/// in the pass of `EntityMotionPlugin` and lands as a block or drops as an item. The embedder adds
/// `EntityMotionPlugin`, `EntityMotionReplicationPlugin`, a consumer of [`BlockSetRequest`], and
/// `ItemEntityPlugin` for the drops to move.
#[derive(Clone, Debug, Default)]
pub struct FallingBlockPlugin {
    pub settings: Settings,
    // chisle: the load of the data pack reads only registry files, so the concrete pairs of each
    // pack come through the plugin. A reader of the non-registry files of a pack in the registry
    // load lifts it.
    /// The concrete pairs file of each pack, lowest pack first (FBK-CON-001).
    pub concrete_pairs: Vec<PackConcretePairs>,
}

/// The public points of the falling block in `Simulate`.
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FallingBlockSet {
    /// Each requested fall is a [`PendingFall`] here, before the world writes and the spawn.
    StartReview,
    /// Each landing of this tick is a [`Landed`] here, before the world writes.
    LandingReview,
}

impl Plugin for FallingBlockPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(FallingBlockConfig {
            settings: self.settings,
            concrete_pairs: self.concrete_pairs.clone(),
        })
        .add_message::<FallRequest>()
        .add_message::<BlockSetRequest>()
        .add_message::<ColumnChange>()
        .configure_sets(
            FixedUpdate,
            (
                FallingBlockSet::StartReview
                    .in_set(TickSet::Simulate)
                    .before(MotionPass),
                FallingBlockSet::LandingReview
                    .in_set(TickSet::Simulate)
                    .after(MotionPass),
            ),
        )
        .add_systems(
            FixedUpdate,
            build_rules
                .in_set(TickSet::DomainInput)
                .run_if(resource_exists::<Blocks>)
                .run_if(resource_exists::<Items>)
                .run_if(resource_exists_and_changed::<CurrentRegistries>),
        )
        .add_systems(
            FixedUpdate,
            (
                propose_falls
                    .before(FallingBlockSet::StartReview)
                    .run_if(on_message::<FallRequest>),
                begin_falls
                    .after(FallingBlockSet::StartReview)
                    .run_if(any_with_component::<PendingFall>),
                begin_ticks.run_if(any_with_component::<FallingBlockEntity>),
            )
                .chain()
                .in_set(TickSet::Simulate)
                .before(MotionPass)
                .run_if(resource_exists::<FallingBlockRules>),
        )
        .add_systems(
            FixedUpdate,
            (
                decide_falls
                    .before(FallingBlockSet::LandingReview)
                    .run_if(any_with_component::<FallingBlockEntity>),
                apply_landings
                    .after(FallingBlockSet::LandingReview)
                    .run_if(any_with_component::<Landed>),
            )
                .in_set(TickSet::Simulate)
                .after(MotionPass)
                .run_if(resource_exists::<FallingBlockRules>),
        );
    }
}
