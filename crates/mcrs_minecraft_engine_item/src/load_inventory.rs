use alloc::vec::Vec;

use bevy_ecs::entity::Entity;
use bevy_ecs::world::World;
use mcrs_minecraft_item_stack::ItemStack;

use crate::InSlot;
use crate::Slots;
use crate::spawn_stack::spawn_stack;

/// Gives `holder` one slot for each value and a stack in each slot whose value is `Some`. The
/// stacks of the slots that it had before despawn.
pub(crate) fn load_inventory(world: &mut World, holder: Entity, values: Vec<Option<ItemStack>>) {
    let Ok(mut entity) = world.get_entity_mut(holder) else {
        return;
    };
    entity.insert(Slots::new(values.len()));
    for (index, value) in values.into_iter().enumerate() {
        let Ok(index) = u16::try_from(index) else {
            break;
        };
        let Some(value) = value else {
            continue;
        };
        let entity = world.spawn_empty();
        let stack = entity.id();
        spawn_stack(entity, value);
        if let Ok(mut stack) = world.get_entity_mut(stack) {
            stack.insert(InSlot::new(holder, index));
        }
    }
}
