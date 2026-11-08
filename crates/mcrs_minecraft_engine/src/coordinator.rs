//! The coordinator: the owner of global state. It owns the dimension worlds and their lockstep
//! tick, the queues that spawn and despawn them, the game time and the world clocks, the
//! operator list, the defaults of a new player and the ranges of global entity ids, and it decides the dimension of each player
//! that joins or changes dimension.
//!
//! The rule of the boundary: a module of this tree never names a module of `muxer`, and no
//! system here reads a resource or a component that the muxer owns. The coordinator tells the
//! muxer what it decides through the messages of `to_muxer`, and learns what the muxer routed
//! from the messages of `to_coordinator`.

pub mod birth_clock;
pub mod clock;
pub mod coordinator_plugin;
pub mod default_game_mode;
pub mod dim;
pub mod dim_cells;
pub mod dimension_order;
pub mod entity_id_ranges;
pub mod join;
pub mod ops;
pub mod roster;
pub mod running_dimensions;
pub mod sub_app_builder;
