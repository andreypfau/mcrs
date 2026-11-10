//! The client state of the entities that the server adds: their movement and interpolation,
//! head and body yaw, walk, hurt and death, and the values that a renderer reads in a frame.
// std: bevy_ecs

pub mod angle;
mod body_turn;
mod client_entity;
pub mod delta;
mod equipment;
mod head_turn;
mod interpolation;
mod living;
mod motion;
mod path;
mod placement;
mod plugin;
mod receive;
mod render_inputs;
mod server_ticking;
mod synced_data;
mod tick;
pub mod walk;

pub use body_turn::*;
pub use client_entity::*;
pub use equipment::*;
pub use head_turn::*;
pub use interpolation::*;
pub use living::*;
pub use motion::*;
pub use path::*;
pub use placement::*;
pub use plugin::*;
pub use receive::{DEATH_EVENT, SNAP_DISTANCE, WORLD_LIMIT};
pub use render_inputs::*;
pub use server_ticking::*;
pub use synced_data::*;
pub use tick::{ClientEntityTick, in_ticking_area, tick_entity};
