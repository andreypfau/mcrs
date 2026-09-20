use bevy_ecs::prelude::Component;
use mcrs_minecraft_protocol::item::ItemDataComponent;

/// One patch entry of a stack: `Some` is a patched value, `None` a tombstone
/// over the prototype's value; a stack without the component inherits the
/// prototype.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct Patch<K: ItemDataComponent>(pub Option<K>);
