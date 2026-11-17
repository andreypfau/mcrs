use alloc::vec::Vec;

use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::world::World;
use mcrs_minecraft_item_stack::ItemStack;

use crate::InSlot;
use crate::SlotChanged;
use crate::SlotRef;
use crate::Slots;
use crate::slot_changed::write_slot_changed;
use crate::spawn_stack::spawn_stack;

pub(crate) fn load_inventory(world: &mut World, holder: Entity, values: Vec<Option<ItemStack>>) {
    let Ok(mut entity) = world.get_entity_mut(holder) else {
        return;
    };
    let emptied: Vec<u16> = entity.get::<Slots>().map_or_else(Vec::new, |slots| {
        (slots.iter().enumerate())
            .filter(|(index, stack)| {
                stack.is_some() && values.get(*index).is_none_or(Option::is_none)
            })
            .filter_map(|(index, _)| u16::try_from(index).ok())
            .collect()
    });
    entity.insert(Slots::new(values.len()));
    for index in emptied {
        let slot = SlotRef { holder, index };
        write_slot_changed(world.get_resource_mut::<Messages<SlotChanged>>(), slot);
    }
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
