//! The data layer. It keeps the last version of each chunk of cold space by key, in an
//! in-memory cache over a [`DiskStore`] that the game supplies. The parts around it reach it
//! only through a [`Link`](mcrs_engine_link::Link) to a [`StoreStage`], by key and version. A
//! [`SharedStore`] lets several parts reach one data layer.

// std: mcrs_engine_link

mod chunk_cache;
mod chunk_version;
mod disk_store;
mod disk_write;
mod no_disk;
mod shared_store;
mod store_answer;
mod store_input;
mod store_output;
mod store_stage;
mod versioned_chunk;

pub use chunk_version::ChunkVersion;
pub use disk_store::DiskStore;
pub use disk_write::DiskWrite;
pub use no_disk::NoDisk;
pub use shared_store::SharedStore;
pub use store_answer::StoreAnswer;
pub use store_input::StoreInput;
pub use store_output::StoreOutput;
pub use store_stage::StoreStage;
pub use versioned_chunk::VersionedChunk;
