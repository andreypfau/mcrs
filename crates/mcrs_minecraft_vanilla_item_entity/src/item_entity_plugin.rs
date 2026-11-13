use core::marker::PhantomData;

use bevy_app::App;
use bevy_app::FixedUpdate;
use bevy_app::Plugin;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::schedule::SystemSet;
use bevy_ecs::schedule::common_conditions::any_with_component;
use mcrs_minecraft_engine::tile_events::TileEvents;
use mcrs_minecraft_engine_physics::EntityPass;
use mcrs_minecraft_engine_physics::MobPublish;
use mcrs_minecraft_engine_world::block_drops::ItemDropped;
use mcrs_minecraft_engine_world::tick::TickSet;

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
use crate::publish_items::publish_items;
use crate::release_row::forget_item_values;
use crate::release_row::release_row;
use crate::spawn_rows::spawn_rows;
use crate::step_items::Stepped;
use crate::step_items::age_items;
use crate::step_items::project_positions;
use crate::step_items::step_items;

/// The item entity in a dimension world whose rows are a `RowStore<M>`: each dropped stack moves
/// as a row of the entity pass, ages, merges and is picked up by each [`ItemCollector`].
#[derive(Debug)]
pub struct ItemEntityPlugin<M> {
    column: PhantomData<fn() -> M>,
}

/// The systems of the item entities in `Simulate`, after the entity pass: the counters, the
/// merges and the pickups. The rows of the new item entities spawn before the pass.
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ItemEntitySet;

impl<M> Default for ItemEntityPlugin<M> {
    fn default() -> Self {
        Self {
            column: PhantomData,
        }
    }
}

impl<M: Send + Sync + 'static> Plugin for ItemEntityPlugin<M> {
    fn build(&self, app: &mut App) {
        app.init_resource::<Stepped>()
            .init_resource::<Released>()
            .init_resource::<TileEvents>()
            .init_resource::<Claimed>()
            .init_resource::<Merged>()
            .add_message::<PickedUp>()
            .add_message::<ItemSpawnRequest>()
            .configure_sets(
                FixedUpdate,
                ItemEntitySet.in_set(TickSet::Simulate).after(EntityPass),
            )
            .add_systems(
                FixedUpdate,
                (
                    step_items::<M>,
                    merge_items,
                    age_items,
                    collect_items.run_if(any_with_component::<ItemCollector>),
                )
                    .chain()
                    .in_set(ItemEntitySet),
            )
            .add_message::<ItemDropped>()
            .add_systems(
                FixedUpdate,
                (launch_block_drops, spawn_requested, spawn_rows::<M>)
                    .chain()
                    .in_set(TickSet::Simulate)
                    .before(EntityPass),
            )
            .add_systems(
                FixedUpdate,
                // The events of other subjects may come in any order: the take of the tile events
                // sorts them by subject.
                publish_items::<M>
                    .in_set(MobPublish)
                    .ambiguous_with(MobPublish),
            )
            .add_systems(
                FixedUpdate,
                project_positions.in_set(TickSet::Project).after(MobPublish),
            )
            .add_observer(release_row::<M>)
            .add_observer(forget_item_values);
    }
}
