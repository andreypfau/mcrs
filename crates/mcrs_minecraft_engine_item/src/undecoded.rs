use alloc::boxed::Box;

use bevy_ecs::component::Component;
use mcrs_minecraft_item_stack::PatchEntry;

/// The patch entries that a stack keeps without a typed component: the types that the value
/// crate does not decode and the custom types that this world did not register.
#[derive(Component, Clone, Debug, PartialEq, Hash)]
pub(crate) struct Undecoded(pub(crate) Box<[PatchEntry]>);
