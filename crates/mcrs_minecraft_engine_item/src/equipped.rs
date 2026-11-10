use bevy_ecs::component::Component;

/// The stack is in the [`Selected`](crate::Selected) slot of its holder.
///
/// The projection in [`ItemSet`](crate::ItemSet) is its only writer. It is current after that set
/// and its command flush; other code reads it and never inserts or removes it.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[component(immutable)]
pub struct Equipped;
