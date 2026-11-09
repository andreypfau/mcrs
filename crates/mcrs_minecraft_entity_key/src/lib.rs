//! The keys of an entity: its id, its birth key, its birth, its UUID at birth, its keyed random
//! stream and the tile that holds it. Physics, the brain, the save and the muxer share them
//! without the physics kernel.

// std: none of its own; glam and mcrs_voxel enable it in the workspace

#![no_std]

mod birth;
mod birth_key;
mod birth_source;
mod birth_uuid;
mod entity_id;
mod keyed_stream;
mod source_kind;
mod tile_key;

pub use birth::Birth;
pub use birth_key::BirthKey;
pub use birth_source::BirthSource;
pub use birth_uuid::birth_uuid;
pub use entity_id::EntityId;
pub use keyed_stream::KeyedStream;
pub use source_kind::SourceKind;
pub use tile_key::TILE_WIDTH;
pub use tile_key::TileKey;
