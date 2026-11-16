use bevy_app::App;
use bevy_app::FixedUpdate;
use bevy_app::Plugin;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::schedule::common_conditions::on_message;
use mcrs_minecraft_engine_item::Gate;
use mcrs_minecraft_engine_item::ItemAppExt;
use mcrs_minecraft_engine_item::RequestDrop;
use mcrs_minecraft_engine_world::tick::TickSet;
use mcrs_minecraft_inventory_player::OffHandRule;

use crate::InventorySet;
use crate::launch_drops::launch_drops;
use crate::off_hand_gate::OffHandGate;
use crate::off_hand_gate::tagged_item_gate;
use crate::restore_holders::hold_new_player;

/// Gives each new player its four holders (INV-SLT-002) with their slot policies, and launches
/// each stack that a drop takes out of a holder of a player as an item entity (IEN-SPN-001). The
/// actions of a player reach the holders through
/// [`PlayerInventoryCommandsExt`](crate::PlayerInventoryCommandsExt).
///
/// Add it to each dimension world, beside
/// [`ItemPlugin`](mcrs_minecraft_engine_item::ItemPlugin), which it needs. It also needs the
/// [`CurrentRegistries`](mcrs_minecraft_registries::CurrentRegistries) resource in the dimension
/// world, which the registry snapshot brings. The player file and a
/// handover reach the holders through [`InventoryPersistencePlugin`](crate::InventoryPersistencePlugin).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VanillaInventoryPlugin {
    /// The value of the game rule `mcrs:offhand_accepts_any_item` (INV-SLT-005).
    pub off_hand: OffHandRule,
}

impl Plugin for VanillaInventoryPlugin {
    fn build(&self, app: &mut App) {
        let off_hand = match self.off_hand {
            OffHandRule::AnyItem => Gate::Permits,
            OffHandRule::TaggedItems => match app.register_slot_gate(tagged_item_gate) {
                Ok(id) => Gate::Ask(id),
                Err(error) => {
                    tracing::error!(%error, "the off-hand gate is not registered; the off-hand slot refuses each place");
                    Gate::Refuses
                }
            },
        };
        app.insert_resource(OffHandGate(off_hand));
        app.configure_sets(
            FixedUpdate,
            (
                InventorySet::Input.in_set(TickSet::ClientInput),
                InventorySet::Drops
                    .after(TickSet::Project)
                    .before(TickSet::Output),
            )
                .chain(),
        )
        .add_systems(
            FixedUpdate,
            launch_drops
                .run_if(on_message::<RequestDrop>)
                .in_set(InventorySet::Drops),
        )
        .add_observer(hold_new_player);
    }
}
