use bevy_ecs::schedule::SystemSet;

/// The systems of the item plugins, for other plugins to order against.
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ItemSet;
