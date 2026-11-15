use bevy_ecs::schedule::SystemSet;

/// The application of the edits of the players, inside [`AttributeEdits`](crate::AttributeEdits).
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PlayerAttributeEdits;
