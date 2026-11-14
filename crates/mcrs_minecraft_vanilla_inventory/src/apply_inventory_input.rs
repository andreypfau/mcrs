use bevy_ecs::change_detection::Mut;
use bevy_ecs::message::MessageReader;
use bevy_ecs::message::Messages;
use bevy_ecs::system::Commands;
use bevy_ecs::world::World;

use crate::InventoryInput;
use crate::apply_click::apply_click;
use crate::close_container::apply_close;
use crate::hand_action::apply_hand_action;
use crate::set_carried_item::apply_selected_slot;
use crate::set_creative_mode_slot::apply_creative_set;

/// Applies each [`InventoryInput`] in the order of arrival, as one queued command each. A command
/// commits its request before the next command reads the slots (ITX-JAV-003, ITX-MOD-005).
pub(crate) fn apply_inventory_input(
    mut inputs: MessageReader<InventoryInput>,
    mut commands: Commands,
) {
    for (_, id) in inputs.read_with_id() {
        let id = id.id;
        commands.queue(move |world: &mut World| apply_input(world, id));
    }
}

fn apply_input(world: &mut World, id: usize) {
    world.try_resource_scope(|world, inputs: Mut<Messages<InventoryInput>>| {
        let Some((input, _)) = inputs.get_message(id) else {
            return;
        };
        let applied = match input {
            InventoryInput::Click { .. } => world.run_system_cached_with(apply_click, input),
            InventoryInput::Close { .. } => world.run_system_cached_with(apply_close, input),
            InventoryInput::Hand { .. } => world.run_system_cached_with(apply_hand_action, input),
            InventoryInput::CreativeSet { .. } => {
                world.run_system_cached_with(apply_creative_set, input)
            }
            InventoryInput::SelectSlot { .. } => {
                world.run_system_cached_with(apply_selected_slot, input)
            }
        };
        if let Err(error) = applied {
            tracing::warn!(%error, "an inventory input did not apply");
        }
    });
}
