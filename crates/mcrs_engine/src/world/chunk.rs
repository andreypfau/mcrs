//! Backward-compat façade for the chunk-related types. The canonical homes
//! are:
//!
//! - geometry: [`crate::geometry::ChunkPos`]
//! - storage:  [`crate::world::storage::chunk`] (Chunk, ChunkBundle, ChunkIndex, ChunkPlugin)
//! - lifecycle markers: [`crate::world::lifecycle::markers`] (ChunkLoaded, ChunkLoading,
//!   ChunkGenerating, ChunkUnloading, ChunkUnloaded)
//! - tickets: [`crate::world::lifecycle::ticket`]
//! - palette: [`crate::world::storage::palette`]

pub use crate::geometry::ChunkPos;
pub use crate::geometry::chunk_pos::BLOCKS;
pub use crate::world::lifecycle::markers::{
    ChunkGenerating, ChunkLoaded, ChunkLoading, ChunkUnloaded, ChunkUnloading,
};
pub use crate::world::lifecycle::ticket;
pub use crate::world::storage::chunk::{Chunk, ChunkBundle, ChunkIndex};
pub(crate) use crate::world::storage::chunk::ChunkPlugin;
pub use crate::world::storage::palette;
