//! The data layer. It keeps the last version of each chunk of cold space by key, in an
//! in-memory cache over a [`DiskStore`] that the game supplies. The parts around it reach it
//! only through a [`SharedStore`], by key and version, which lets several parts reach one
//! data layer.

// std: mcrs_engine_link

mod chunk_version;
mod disk_store;
mod memory_disk;
mod shared_store;
mod store_answer;
mod store_input;
mod store_output;
mod store_stage;
mod versioned_chunk;

pub use chunk_version::ChunkVersion;
pub use disk_store::DiskStore;
pub use memory_disk::MemoryDisk;
pub use shared_store::SharedStore;
pub use store_answer::StoreAnswer;
pub use store_input::StoreInput;
pub use store_output::StoreOutput;
pub use versioned_chunk::{VersionedChunk, keep_latest};
