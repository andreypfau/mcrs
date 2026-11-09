//! The goals of the passive animals for the AI stage of `mcrs_minecraft_brain`, the goal list of
//! the pig, and [`VanillaBrain`], the brain of each mob type with goals. The goals read the world
//! through [`GoalWorld`](mcrs_minecraft_brain::GoalWorld), which the host implements on its block
//! snapshot; a path search replaces the straight-line route of that trait later.

#![no_std]

extern crate alloc;

#[cfg(feature = "testing")]
mod flat_ground;
mod float;
mod gate;
mod look_around;
mod look_at_player;
mod panic;
mod parts;
mod path_track;
mod pig_brain;
#[cfg(feature = "testing")]
mod pig_pen;
mod random_position;
mod stroll;
mod vanilla_brain;

#[cfg(feature = "testing")]
pub use flat_ground::*;
pub use float::*;
pub use gate::*;
pub use look_around::*;
pub use look_at_player::*;
pub use panic::*;
pub use parts::*;
pub use path_track::*;
pub use pig_brain::*;
#[cfg(feature = "testing")]
pub use pig_pen::*;
pub use random_position::*;
pub use stroll::*;
pub use vanilla_brain::*;
