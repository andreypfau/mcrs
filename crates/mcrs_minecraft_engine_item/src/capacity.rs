use bevy_ecs::entity::Entity;
use mcrs_minecraft_item_stack::MaxStackSize;

use crate::Stacks;

pub(crate) const SLOT_LIMIT: u8 = 99;

pub(crate) fn capacity(stacks: &mut Stacks, stack: Entity) -> u8 {
    stack_capacity(stacks.get::<MaxStackSize>(stack))
}

#[must_use]
pub fn stack_capacity(max: Option<&MaxStackSize>) -> u8 {
    let max = max.map_or(1, |max| max.0.clamp(1, i32::from(SLOT_LIMIT)));
    u8::try_from(max).unwrap_or(1)
}
