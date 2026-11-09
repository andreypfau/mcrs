//! The keys of an entity: its id, its UUID, which is also its order key, the sources of a new
//! UUID, its keyed random stream and the tile that holds it. Physics, the brain, the save and the
//! muxer share them without the physics kernel.

// std: none of its own; glam and mcrs_voxel enable it in the workspace. alloc: BTreeMap

#![no_std]

extern crate alloc;

mod birth_sequences;
mod birth_source;
mod entity_id;
mod entity_key;
mod keyed_stream;
mod tile_key;

pub use birth_sequences::BirthSequences;
pub use birth_source::BirthSource;
pub use entity_id::EntityId;
pub use entity_key::EntityKey;
pub use keyed_stream::KeyedStream;
pub use tile_key::TILE_WIDTH;
pub use tile_key::TileKey;
