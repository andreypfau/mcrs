//! The AI stage of mobs: a kernel without Bevy and without std. A host publishes a versioned
//! snapshot of its mobs, players and facts; the stage wakes only the mobs that a timer or a fact
//! names, runs their goal selection per tile, and gives decisions that the host applies 1 to
//! `max_lag` ticks later. Random values are keyed by the world seed, the order key of the mob, the
//! issue tick and the part.

#![no_std]

extern crate alloc;

mod ai_delay;
mod ai_driver;
mod brain;
mod cadence;
mod control_flags;
mod decision;
mod decision_pipeline;
mod fact;
mod goal_spec;
mod intent;
mod keyed_random;
mod mob_view;
mod player_row;
mod player_table;
mod replica;
mod selector;
mod stage;
mod stage_config;
mod tile_brains;

pub use ai_delay::*;
pub use ai_driver::*;
pub use brain::*;
pub use cadence::*;
pub use control_flags::*;
pub use decision::*;
pub use decision_pipeline::*;
pub use fact::*;
pub use goal_spec::*;
pub use intent::*;
pub use keyed_random::*;
pub use mob_view::*;
pub use player_row::*;
pub use player_table::*;
pub use replica::*;
pub use selector::*;
pub use stage::*;
pub use stage_config::*;
pub use tile_brains::*;
