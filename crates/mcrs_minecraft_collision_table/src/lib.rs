//! Builds the [`CollisionTable`](mcrs_minecraft_physics::CollisionTable) of the physics kernel
//! from the block definitions and the block tags: f64 shapes on the pixel grid, the shape
//! classes, the flags and the motion data of each state.

mod collision_table;
mod inside_shape;
mod named_block;
mod widen;

pub use collision_table::collision_table;
pub use widen::on_pixel_grid;
pub use widen::widen;
