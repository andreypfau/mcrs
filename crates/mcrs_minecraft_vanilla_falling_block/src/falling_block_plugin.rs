use bevy_app::App;
use bevy_app::FixedUpdate;
use bevy_app::Plugin;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::schedule::SystemSet;
use bevy_ecs::schedule::common_conditions::any_with_component;
use bevy_ecs::schedule::common_conditions::on_message;
use bevy_ecs::schedule::common_conditions::resource_exists;
use bevy_ecs::schedule::common_conditions::resource_exists_and_changed;
use bevy_ecs::system::Commands;
use bevy_ecs::system::Res;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_engine::column_change::ColumnChange;
use mcrs_minecraft_engine_entity_motion::MotionPass;
use mcrs_minecraft_engine_world::block_update::BlockSetRequest;
use mcrs_minecraft_engine_world::tick::TickSet;
use mcrs_minecraft_falling_block::PackConcretePairs;
use mcrs_minecraft_falling_block::Settings;
use mcrs_minecraft_item::Items;
use mcrs_minecraft_registries::CurrentRegistries;

use crate::Falling;
use crate::Landed;
use crate::PendingFall;
use crate::apply_landings::apply_landings;
use crate::begin_falls::begin_falls;
use crate::begin_ticks::begin_ticks;
use crate::decide_falls::decide_falls;
use crate::fall_request::FallRequest;
use crate::fall_request::propose_falls;
use crate::falling_block_rules::FallingBlockRules;
use crate::falling_block_rules::build_rules;

#[derive(Clone, Debug, Default)]
pub struct FallingBlockPlugin {
    pub settings: Settings,
    pub concrete_pairs: Vec<PackConcretePairs>,
}

#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FallingBlockSet {
    StartReview,
    LandingReview,
}

impl Plugin for FallingBlockPlugin {
    fn build(&self, app: &mut App) {
        let settings = self.settings;
        let concrete_pairs = self.concrete_pairs.clone();
        let rules = move |mut commands: Commands,
                          blocks: Res<Blocks>,
                          items: Res<Items>,
                          registries: Res<CurrentRegistries>| {
            build_rules(
                settings,
                &concrete_pairs,
                commands.reborrow(),
                &blocks,
                &items,
                &registries,
            );
        };
        app.add_message::<FallRequest>()
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
                rules
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
                    begin_ticks.run_if(any_with_component::<Falling>),
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
                        .run_if(any_with_component::<Falling>),
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
