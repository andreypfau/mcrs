use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::change_detection::Mut;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::MessageReader;
use bevy_ecs::message::Messages;
use bevy_ecs::system::Commands;
use bevy_ecs::world::World;

use crate::InventoryInput;
use crate::apply_click::apply_click;
use crate::click_drag::ClickDrag;
use crate::set_creative_mode_slot::apply_creative_packet;
use mcrs_minecraft_vanilla_inventory::PlayerInventoryCommandsExt;

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
        let applied = match *input {
            InventoryInput::Click { .. } => {
                if let Err(error) = apply_click(world, input) {
                    tracing::warn!(%error, "an inventory input did not apply");
                }
                Ok(())
            }
            InventoryInput::CreativeSet { .. } => {
                world.run_system_cached_with(apply_creative_packet, input)
            }
            InventoryInput::Close { player } => {
                end_drag(world, player);
                world.commands().entity(player).close_menu();
                Ok(())
            }
            InventoryInput::Hand { player, action } => {
                world.commands().entity(player).hand_action(action);
                Ok(())
            }
            InventoryInput::SelectSlot { player, slot } => {
                if let Ok(slot) = u8::try_from(slot) {
                    world.commands().entity(player).select_slot(slot);
                }
                Ok(())
            }
        };
        world.flush();
        if let Err(error) = applied {
            tracing::warn!(%error, "an inventory input did not apply");
        }
    });
}

fn end_drag(world: &mut World, player: Entity) {
    if let Some(mut drag) = world.get_mut::<ClickDrag>(player) {
        drag.set_if_neq(ClickDrag::default());
    }
}
