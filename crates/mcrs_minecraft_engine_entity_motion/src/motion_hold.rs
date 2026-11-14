use bevy_ecs::component::Component;

/// An entity with it keeps its [`MotionState`](crate::MotionState), but the pass does not step
/// it. A plugin that must see an entity before it moves inserts it and removes it in
/// [`MotionRelease`](crate::MotionRelease).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MotionHold;
