use bevy_ecs::entity::Entity;
use mcrs_minecraft_item_stack::MaxStackSize;

use crate::Stacks;

pub(crate) const SLOT_LIMIT: u8 = 99;

pub(crate) fn capacity(stacks: &mut Stacks, stack: Entity) -> u8 {
    let max = stacks.get::<MaxStackSize>(stack).map_or(1, |max| max.0);
    u8::try_from(max.clamp(1, i32::from(SLOT_LIMIT))).unwrap_or(1)
}
