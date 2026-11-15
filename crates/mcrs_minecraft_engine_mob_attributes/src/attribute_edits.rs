use bevy_ecs::schedule::SystemSet;

/// The application of the [`AttributeEdit`](crate::AttributeEdit)s of the tick, in `Project`. A
/// system that writes edits runs before it; a system that reads
/// [`AttributeChanged`](crate::AttributeChanged) runs after it.
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AttributeEdits;
