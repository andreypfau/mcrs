use bevy_ecs::component::Component;
use mcrs_minecraft_item::ProtoStack;
use mcrs_minecraft_registries::Wire;

pub(crate) const ITEM_STACK_INDEX: u8 = 8;

#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct ItemEntityStack(pub ProtoStack<Wire>);
