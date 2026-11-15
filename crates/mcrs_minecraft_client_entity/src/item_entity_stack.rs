use bevy_ecs::component::Component;
use mcrs_minecraft_item::ProtoStack;
use mcrs_minecraft_registries::Wire;

pub(crate) const ITEM_STACK_INDEX: u8 = 8;

/// The stack of an item entity, as the entity data of the server last gave it. It is empty until
/// the first entity data, and the client removes an item entity whose stack is empty.
#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct ItemEntityStack(pub ProtoStack<Wire>);
