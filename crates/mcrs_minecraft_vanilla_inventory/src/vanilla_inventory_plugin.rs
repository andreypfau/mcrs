use bevy_app::App;
use bevy_app::FixedUpdate;
use bevy_app::Plugin;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::schedule::SystemCondition;
use bevy_ecs::schedule::common_conditions::on_message;
use mcrs_minecraft_engine::bus::OutboundPlayerPacket;
use mcrs_minecraft_engine_item::Given;
use mcrs_minecraft_engine_item::RequestDrop;
use mcrs_minecraft_engine_item::RequestResult;
use mcrs_minecraft_engine_world::birth_clock::BirthClock;
use mcrs_minecraft_engine_world::tick::TickSet;

use crate::InventoryClick;
use crate::InventorySet;
use crate::apply_clicks::apply_clicks;
use crate::click_backlog::ClickBacklog;
use crate::click_backlog::any_backlog;
use crate::close_container::close_container;
use crate::decode_click::decode_click;
use crate::forget_rejected_swap::forget_rejected_swap;
use crate::full_sends::FullSends;
use crate::full_sends::any_full_send;
use crate::hand_action::hand_action;
use crate::launch_drops::launch_drops;
use crate::menu_queue::MenuQueue;
use crate::menu_queue::any_queued;
use crate::restore_holders::hold_new_player;
use crate::send_full_state::send_full_state;
use crate::set_carried_item::set_carried_item;
use crate::set_creative_mode_slot::set_creative_mode_slot;
use crate::sync_menus::sync_menus;

/// Gives each new player its four holders (INV-SLT-002). After each apply of the holders, the
/// next tick sends the player its selected slot and its inventory menu (INV-SEL-003,
/// INV-SYN-005); the `set_carried_item` packet sets the selected slot (INV-SEL-002).
///
/// Each `container_click` packet becomes an [`InventoryClick`], which
/// [`InventorySet::Input`] turns into a request of the engine (ITX-JAV-001 to ITX-JAV-005).
/// [`InventorySet::Sync`] then sends each stack of the inventory menu that the client does not
/// hold (INV-SYN-003, INV-SYN-004). The drop keys and the hand swap of the `player_action`
/// packet are requests too (INV-SEL-004, INV-SEL-005), and each stack that a drop takes out of a
/// holder of a player becomes an item entity. The `container_close` packet closes the inventory
/// menu by a request (INV-CLS-001), and the `set_creative_mode_slot` packet sets an entry or
/// throws a stack (INV-CRE-003, INV-CRE-004).
///
/// Add it to each dimension world, beside
/// [`ItemPlugin`](mcrs_minecraft_engine_item::ItemPlugin), which it needs. The player file and a
/// handover reach the holders through [`InventoryPersistencePlugin`](crate::InventoryPersistencePlugin).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VanillaInventoryPlugin;

impl Plugin for VanillaInventoryPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<OutboundPlayerPacket>()
            .add_message::<InventoryClick>()
            .init_resource::<FullSends>()
            .init_resource::<ClickBacklog>()
            .init_resource::<MenuQueue>()
            .init_resource::<BirthClock>()
            .configure_sets(
                FixedUpdate,
                (InventorySet::Input, InventorySet::Sync)
                    .chain()
                    .after(TickSet::Project)
                    .before(TickSet::Output),
            )
            .add_systems(
                FixedUpdate,
                (
                    apply_clicks
                        .run_if(on_message::<InventoryClick>.or_eager(any_backlog))
                        .in_set(InventorySet::Input),
                    launch_drops
                        .run_if(on_message::<RequestDrop>)
                        .in_set(InventorySet::Sync),
                    (
                        send_full_state.run_if(any_full_send),
                        sync_menus.run_if(
                            on_message::<RequestResult>
                                .or_eager(on_message::<Given>)
                                .or_eager(any_queued),
                        ),
                    )
                        .chain()
                        .in_set(InventorySet::Sync),
                ),
            )
            .add_observer(hold_new_player)
            .add_observer(set_carried_item)
            .add_observer(decode_click)
            .add_observer(hand_action)
            .add_observer(close_container)
            .add_observer(set_creative_mode_slot)
            .add_observer(forget_rejected_swap);
    }
}
