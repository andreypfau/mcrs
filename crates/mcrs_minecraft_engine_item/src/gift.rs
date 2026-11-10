use bevy_ecs::entity::Entity;
use mcrs_minecraft_item_stack::ItemStack;

/// What a holder gets: a stack entity, or a value that becomes a new stack.
#[derive(Clone, Debug, PartialEq)]
pub enum Gift {
    Stack(Entity),
    Value(ItemStack),
}

impl From<Entity> for Gift {
    fn from(stack: Entity) -> Self {
        Self::Stack(stack)
    }
}

impl From<ItemStack> for Gift {
    fn from(value: ItemStack) -> Self {
        Self::Value(value)
    }
}
