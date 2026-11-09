//! The read-only block world of the goals of mobs, which a host implements on its block snapshot.

#![no_std]

mod goal_world;
mod ground_cell;
mod straight_route;

pub use goal_world::*;
pub use ground_cell::*;
pub use straight_route::*;
