//! The rows of the server entities of a dimension world, by tile, with a dense column for the hot
//! state of the mobs that moves with its row. Each tick the pass steps every row with the physics
//! kernel, in parallel over tiles, against the blocks of the last tick and the body grid from the
//! start of the tick. Items and experience orbs are rows only. The store has no Bevy; the `bevy`
//! feature makes it a resource and the compute pool its executor.

mod body_grid;
mod changes;
mod crossing;
mod effect_buffer;
mod executor;
mod front_grid;
mod keyed;
mod mob_intent;
mod mob_rows;
mod pass;
mod placed;
mod row_store;
mod tile_bodies;
mod tile_queues;
mod tile_rows;
mod tile_world;
mod writer_key;
mod yzx_pos;

pub use body_grid::BodyGrid;
pub use changes::Changes;
pub use crossing::Crossing;
pub use effect_buffer::EffectBuffer;
pub use executor::Executor;
pub use keyed::Keyed;
pub use mob_intent::MobIntent;
pub use mob_rows::MobRow;
pub use mob_rows::MobRows;
pub use pass::Pass;
pub use placed::Placed;
pub use row_store::RowStore;
pub use row_store::tiles_per_task;
pub use tile_bodies::TileBodies;
pub use tile_queues::TileQueues;
pub use tile_rows::Row;
pub use tile_rows::RowRef;
pub use tile_rows::TileRows;
pub use writer_key::WriterKey;
pub use yzx_pos::YzxPos;
