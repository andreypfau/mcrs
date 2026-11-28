use bevy_app::App;
use bevy_app::FixedUpdate;
use bevy_app::Plugin;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::schedule::common_conditions::any_with_component;
use bevy_ecs::schedule::common_conditions::on_message;
use mcrs_minecraft_engine::bus::OutboundPlayerPacket;
use mcrs_minecraft_engine_inventory_click::MenuClickPlugin;
use mcrs_minecraft_engine_inventory_menu::MenuPlugin;
use mcrs_minecraft_engine_world::birth_clock::BirthClock;
use mcrs_minecraft_engine_world::tick::TickSet;

use crate::InventoryInput;
use crate::InventoryServerSet;
use crate::MenuSync;
use crate::apply_inventory_input::apply_inventory_input;
use crate::close_container::close_container;
use crate::close_menu_sync::close_menu_sync;
use crate::decode_click::decode_click;
use crate::decode_hand_action::decode_hand_action;
use crate::forget_rejected_swap::forget_rejected_swap;
use crate::full_sends::FullSends;
use crate::full_sends::any_full_send;
use crate::menu_queue::MenuQueue;
use crate::open_menu_sync::open_menu_sync;
use crate::send_full_state::send_full_state;
use crate::sent_selected::SentSelected;
use crate::set_carried_item::set_carried_item;
use crate::set_creative_mode_slot::set_creative_mode_slot;
use crate::sync_held_slot::sync_held_slot;
use crate::sync_menus::sync_menus;
use mcrs_minecraft_vanilla_inventory::InventorySet;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InventoryServerPlugin;

impl Plugin for InventoryServerPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<MenuPlugin>() {
            app.add_plugins(MenuPlugin);
        }
        if !app.is_plugin_added::<MenuClickPlugin>() {
            app.add_plugins(MenuClickPlugin);
        }
        app.add_message::<OutboundPlayerPacket>()
            .add_message::<InventoryInput>()
            .init_resource::<FullSends>()
            .init_resource::<MenuQueue>()
            .init_resource::<BirthClock>()
            .configure_sets(
                FixedUpdate,
                InventoryServerSet::Sync
                    .after(TickSet::Project)
                    .after(InventorySet::Drops)
                    .before(TickSet::Output),
            )
            .add_systems(
                FixedUpdate,
                (
                    apply_inventory_input
                        .run_if(on_message::<InventoryInput>)
                        .in_set(InventorySet::Input),
                    (
                        send_full_state.run_if(any_full_send),
                        sync_held_slot.run_if(any_with_component::<SentSelected>),
                        sync_menus.run_if(any_with_component::<MenuSync>),
                    )
                        .chain()
                        .in_set(InventoryServerSet::Sync),
                ),
            )
            .add_observer(set_carried_item)
            .add_observer(decode_click)
            .add_observer(decode_hand_action)
            .add_observer(close_container)
            .add_observer(set_creative_mode_slot)
            .add_observer(forget_rejected_swap)
            .add_observer(open_menu_sync)
            .add_observer(close_menu_sync);
    }
}
