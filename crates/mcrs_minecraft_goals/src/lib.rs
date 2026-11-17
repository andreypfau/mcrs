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
