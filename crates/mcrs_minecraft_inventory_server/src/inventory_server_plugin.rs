use bevy_app::App;
use bevy_app::FixedUpdate;
use bevy_app::Plugin;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::schedule::common_conditions::any_with_component;
use bevy_ecs::schedule::common_conditions::on_message;
use mcrs_minecraft_engine::bus::OutboundPlayerPacket;
use mcrs_minecraft_engine_world::birth_clock::BirthClock;
use mcrs_minecraft_engine_world::tick::TickSet;

use crate::InventoryInput;
use crate::InventoryServerSet;
use crate::MenuSync;
use crate::apply_inventory_input::apply_inventory_input;
use crate::close_container::close_container;
use crate::decode_click::decode_click;
use crate::decode_hand_action::decode_hand_action;
use crate::forget_rejected_swap::forget_rejected_swap;
use crate::full_sends::FullSends;
use crate::full_sends::any_full_send;
use crate::menu_queue::MenuQueue;
use crate::queue_full_send::queue_full_send;
use crate::send_full_state::send_full_state;
use crate::set_carried_item::set_carried_item;
use crate::set_creative_mode_slot::set_creative_mode_slot;
use crate::sync_menus::sync_menus;
use mcrs_minecraft_vanilla_inventory::InventorySet;

/// Decodes the inventory packets of each player and keeps its client in agreement with its
/// holders. After each restore of the holders, the next sync sends the player its selected slot
/// and its inventory menu (INV-SEL-003, INV-SYN-005).
///
/// Each inventory packet becomes an [`InventoryInput`], which [`InventorySet::Input`] applies in
/// the order of arrival (ITX-JAV-003): a `container_click` packet becomes a request of the engine
/// (ITX-JAV-001 to ITX-JAV-005), and the other packets become the commands of
/// [`PlayerInventoryCommandsExt`](mcrs_minecraft_vanilla_inventory::PlayerInventoryCommandsExt): the drop keys and the hand
/// swap of the `player_action` packet (INV-SEL-004, INV-SEL-005), the `set_carried_item` packet
/// (INV-SEL-002), the `container_close` packet (INV-CLS-001), and the `set_creative_mode_slot`
/// packet (INV-CRE-003, INV-CRE-004). In each tick, [`InventoryServerSet::Sync`] then sends each
/// stack of the inventory menu that the client does not hold, whatever wrote it (INV-SYN-003,
/// INV-SYN-004).
///
/// Add it beside [`VanillaInventoryPlugin`](mcrs_minecraft_vanilla_inventory::VanillaInventoryPlugin), which it needs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InventoryServerPlugin;

impl Plugin for InventoryServerPlugin {
    fn build(&self, app: &mut App) {
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
                        sync_menus.run_if(any_with_component::<MenuSync>),
                    )
                        .chain()
                        .in_set(InventoryServerSet::Sync),
                ),
            )
            .add_observer(queue_full_send)
            .add_observer(set_carried_item)
            .add_observer(decode_click)
            .add_observer(decode_hand_action)
            .add_observer(close_container)
            .add_observer(set_creative_mode_slot)
            .add_observer(forget_rejected_swap);
    }
}
