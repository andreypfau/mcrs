use bevy_app::App;
use bevy_app::FixedUpdate;
use bevy_app::Plugin;
use bevy_ecs::schedule::IntoScheduleConfigs;
use mcrs_minecraft_engine::bus::OutboundPlayerPacket;
use mcrs_minecraft_engine_world::tick::TickSet;

use crate::InventorySet;
use crate::full_sends::FullSends;
use crate::full_sends::any_full_send;
use crate::restore_holders::hold_new_player;
use crate::send_full_state::send_full_state;
use crate::set_carried_item::set_carried_item;

/// Gives each new player its four holders (INV-SLT-002). After each apply of the holders, the
/// next tick sends the player its selected slot and its inventory menu (INV-SEL-003,
/// INV-SYN-005); the `set_carried_item` packet sets the selected slot (INV-SEL-002).
///
/// Add it to each dimension world, beside
/// [`ItemPlugin`](mcrs_minecraft_engine_item::ItemPlugin), which it needs. The player file and a
/// handover reach the holders through [`InventoryPersistencePlugin`](crate::InventoryPersistencePlugin).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VanillaInventoryPlugin;

impl Plugin for VanillaInventoryPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<OutboundPlayerPacket>()
            .init_resource::<FullSends>()
            .configure_sets(
                FixedUpdate,
                InventorySet.after(TickSet::Project).before(TickSet::Output),
            )
            .add_systems(
                FixedUpdate,
                send_full_state.run_if(any_full_send).in_set(InventorySet),
            )
            .add_observer(hold_new_player)
            .add_observer(set_carried_item);
    }
}
