use bevy_ecs::entity::Entity;
use mcrs_minecraft_item_stack::MaxStackSize;

use crate::Stacks;

/// The slot limit of a slot whose holder gives none (ITX-LIM-001).
pub(crate) const SLOT_LIMIT: u8 = 99;

/// The smaller of the slot limit and the maximum stack size, which is 1 without the component
/// (ITX-LIM-002, STK-CMP-001).
pub(crate) fn capacity(stacks: &mut Stacks, stack: Entity) -> u8 {
    let max = stacks.get::<MaxStackSize>(stack).map_or(1, |max| max.0);
    u8::try_from(max.clamp(1, i32::from(SLOT_LIMIT))).unwrap_or(1)
}
