use bevy_app::App;
use bevy_app::FixedUpdate;
use bevy_app::Plugin;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::schedule::SystemCondition;
use bevy_ecs::schedule::SystemSet;
use bevy_ecs::schedule::common_conditions::any_with_component;
use bevy_ecs::schedule::common_conditions::on_message;
use mcrs_minecraft_engine::tile_events::TileEvents;
use mcrs_minecraft_engine_entity_motion::MotionPass;
use mcrs_minecraft_engine_entity_motion_replication::MotionPublish;
use mcrs_minecraft_engine_physics::MobPublish;
use mcrs_minecraft_engine_world::block_drops::ItemDropped;
use mcrs_minecraft_engine_world::tick::TickSet;
use mcrs_minecraft_item_entity::ItemEntity;

use crate::ItemCollector;
use crate::PickedUp;
use crate::block_drops::launch_block_drops;
use crate::collect_items::Claimed;
use crate::collect_items::collect_items;
use crate::item_spawn_request::ItemSpawnRequest;
use crate::item_spawn_request::spawn_requested;
use crate::merge_items::Merged;
use crate::merge_items::merge_items;
use crate::publish_items::Released;
use crate::publish_items::Takes;
use crate::publish_items::correct_spawn_counts;
use crate::publish_items::publish_stacks;
use crate::publish_items::publish_takes;
use crate::publish_items::takes_pending;
use crate::release_motion::forget_item_values;
use crate::release_motion::release_motion;
use crate::spawn_motions::spawn_motions;
use crate::step_items::Stepped;
use crate::step_items::age_items;
use crate::step_items::any_stepped;
use crate::step_items::project_positions;
use crate::step_items::step_items;

#[derive(Clone, Copy, Debug, Default)]
pub struct ItemEntityPlugin;

#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ItemEntitySet;

impl Plugin for ItemEntityPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Stepped>()
            .init_resource::<Released>()
            .init_resource::<Takes>()
            .init_resource::<TileEvents>()
            .init_resource::<Claimed>()
            .init_resource::<Merged>()
            .add_message::<PickedUp>()
            .add_message::<ItemSpawnRequest>()
            .configure_sets(
                FixedUpdate,
                ItemEntitySet.in_set(TickSet::Simulate).after(MotionPass),
            )
            .add_systems(
                FixedUpdate,
                (
                    step_items.run_if(any_with_component::<ItemEntity>.or_else(any_stepped)),
                    merge_items.run_if(any_stepped),
                    age_items,
                    collect_items.run_if(any_with_component::<ItemCollector>),
                )
                    .chain()
                    .in_set(ItemEntitySet),
            )
            .add_message::<ItemDropped>()
            .add_systems(
                FixedUpdate,
                (
                    launch_block_drops.run_if(on_message::<ItemDropped>),
                    spawn_requested.run_if(on_message::<ItemSpawnRequest>),
                    spawn_motions,
                )
                    .chain()
                    .in_set(TickSet::Simulate)
                    .before(MotionPass),
            )
            .add_systems(
                FixedUpdate,
                (
                    publish_takes.run_if(on_message::<PickedUp>.or_else(takes_pending)),
                    correct_spawn_counts,
                )
                    .in_set(MobPublish)
                    .ambiguous_with(MobPublish),
            )
            .add_systems(
                FixedUpdate,
                (publish_stacks, project_positions.run_if(any_stepped))
                    .chain()
                    .in_set(TickSet::Project)
                    .after(MotionPublish),
            )
            .add_observer(release_motion)
            .add_observer(forget_item_values);
    }
}
