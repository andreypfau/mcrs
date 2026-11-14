use bevy::prelude::*;

/// Marks an entity whose tick values are values of the frame: each tick record of the entity is
/// fresh, so the GPU does not interpolate from the record before it.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FramePosed;
