use bevy_ecs::entity::Entity;
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_item_stack::MaxStackSize;

use crate::Stacks;

pub(crate) const SLOT_LIMIT: u8 = 99;

pub(crate) fn capacity(stacks: &mut Stacks, stack: Entity) -> u8 {
    Count::capacity(stacks.get::<MaxStackSize>(stack))
}
