#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

mod access;
mod block;
mod block_state_id;
mod bounds;
mod box_volume;
mod cursor;
mod paletted;
mod restrict;
mod sparse;
mod visit;
mod voxels;

pub use access::{Access, Denied, Unavailable, Volume};
pub use block::{
    BlockVolume, BlockVolumeMut, Palette, PaletteBuffer, SectionBlocks, SectionBlocksMut,
};
pub use block_state_id::BlockStateId;
pub use bounds::{ColumnLayout, SectionBox};
pub use box_volume::BoxVolume;
pub use cursor::Cursor;
pub use paletted::{PalettedSection, PalettedSectionMut};
pub use restrict::{AccessPolicy, Restricted};
pub use sparse::SparseVolume;
pub use visit::{Missing, SectionVolume, VisitOptions, VisitOrder};
pub use voxels::{VoxelSection, VoxelSectionMut, VoxelStore, VoxelStoreMut};
