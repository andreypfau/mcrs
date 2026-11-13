use bevy_ecs::entity::Entity;
use mcrs_minecraft_engine_item::Stacks;
use mcrs_minecraft_item_stack::MaxStackSize;

/// The largest value of the component `max_stack_size`.
const LARGEST: i32 = 99;

/// The maximum stack size of `stack`: 1 without the component.
pub(crate) fn max_count(stacks: &mut Stacks, stack: Entity) -> u8 {
    let max = stacks.get::<MaxStackSize>(stack).map_or(1, |max| max.0);
    u8::try_from(max.clamp(1, LARGEST)).unwrap_or(1)
}
