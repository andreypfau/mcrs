//! The state of an entity that a viewer draws: its placement and interpolation, head and body
//! yaw, walk, hurt and death, entity data and equipment, the tick that advances that state, and
//! the values that a renderer reads in a frame.
// std: bevy_ecs

pub mod angle;
mod body_turn;
mod client_entity;
mod entity_light;
mod entity_tick_plugin;
mod equipment;
mod head_turn;
mod host_entity;
mod interpolation;
mod living;
mod motion;
mod path;
mod placement;
mod render_inputs;
mod server_ticking;
mod synced_data;
mod tick;
mod ticking_area;
pub mod walk;

pub use body_turn::*;
pub use client_entity::*;
pub use entity_light::*;
pub use entity_tick_plugin::*;
pub use equipment::*;
pub use head_turn::*;
pub use host_entity::*;
pub use interpolation::*;
pub use living::*;
pub use motion::*;
pub use path::*;
pub use placement::*;
pub use render_inputs::*;
pub use server_ticking::*;
pub use synced_data::*;
pub use tick::{ClientEntityTick, in_ticking_area, tick_entity};
pub use ticking_area::*;
