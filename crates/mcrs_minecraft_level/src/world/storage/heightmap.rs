use bevy_ecs::prelude::Component;
use mcrs_minecraft_chunk::ColumnHeights;

/// Topmost non-air block. The upper bound of every other map.
#[derive(Component, Debug, Clone)]
pub struct SurfaceHeightmap(pub ColumnHeights);

/// Topmost block that blocks motion, leaves included, fluids excluded.
#[derive(Component, Debug, Clone)]
pub struct SolidHeightmap(pub ColumnHeights);

/// Topmost block that blocks motion or holds a fluid.
#[derive(Component, Debug, Clone)]
pub struct MotionHeightmap(pub ColumnHeights);

/// Topmost block that blocks motion without being leaves, or holds a fluid.
#[derive(Component, Debug, Clone)]
pub struct NoLeavesHeightmap(pub ColumnHeights);
