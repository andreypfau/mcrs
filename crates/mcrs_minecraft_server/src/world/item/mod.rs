pub mod chest;
pub mod click;
pub mod menu;
pub mod sync;

use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::schedule::{IntoScheduleConfigs, SystemSet};
use mcrs_minecraft_inventory::{
    ContainerClickRequest, handle_container_clicks, tick_drop_throttles,
};

/// Every stack mutation of a tick runs in `Mutate`; `Sync` then reads what
/// changed once, so readers outside these sets see a table that is at most
/// one tick stale, never half-applied.
#[derive(SystemSet, Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum StackSet {
    Mutate,
    Sync,
}

pub struct ItemPlugin;

impl Plugin for ItemPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(FixedUpdate, (StackSet::Mutate, StackSet::Sync).chain())
            .add_message::<ContainerClickRequest>()
            .add_message::<click::CreativeSlotRequest>()
            .add_message::<click::CloseContainerRequest>()
            .add_message::<chest::OpenContainerRequest>()
            .add_observer(click::decode_container_click)
            .add_observer(click::decode_creative_slot)
            .add_observer(click::decode_container_close)
            .add_systems(
                FixedUpdate,
                (
                    menu::open_menus,
                    chest::close_dead_menus,
                    chest::open_containers,
                    tick_drop_throttles,
                    handle_container_clicks,
                    click::handle_creative_slots,
                    click::close_menus,
                    click::handle_drop_actions,
                )
                    .chain()
                    .in_set(StackSet::Mutate),
            )
            .add_systems(FixedUpdate, sync::sync_stack_slots.in_set(StackSet::Sync));
    }
}
