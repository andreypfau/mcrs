//! The controls of a mob in each tick: the reflexes that turn the decisions of the AI stage into
//! the steering and the rotations of the walker row (AIT-RFX-001, PATH-MOV, GOAL-CTL). The host
//! keeps one [`MobControl`] for each mob, applies the decisions to it, and calls
//! [`MobControl::before_travel`] before the step of the row and [`MobControl::after_travel`] after
//! it.

mod angles;
mod body_rotation;
mod control_input;
mod control_output;
mod follower;
mod look_control;
mod mob_control;
mod move_control;

pub use angles::*;
pub use body_rotation::*;
pub use control_input::*;
pub use control_output::*;
pub use follower::*;
pub use look_control::*;
pub use mob_control::*;
pub use move_control::*;
