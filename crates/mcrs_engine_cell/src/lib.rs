//! Worlds that tick in lockstep. Each [`CellWorld`] takes one owned input and gives one owned
//! output for each [`Tick`], and its [`Placement`] decides the thread it runs on.

// std: mcrs_engine_link

mod cell_id;
mod cell_output;
mod cell_stage;
mod cell_world;
mod cells;
mod lent;
#[cfg(feature = "bevy")]
mod sub_app_world;
mod tick;
mod tick_error;

pub use cell_id::CellId;
pub use cell_output::CellOutput;
pub use cell_world::CellWorld;
pub use cells::Cells;
pub use lent::Lent;
// A user places a world without a dependency on the link that carries it.
pub use mcrs_engine_link::Placement;
#[cfg(feature = "bevy")]
pub use sub_app_world::SubAppWorld;
pub use tick::Tick;
pub use tick_error::TickError;
