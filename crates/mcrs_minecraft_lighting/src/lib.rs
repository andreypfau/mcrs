#![allow(
    clippy::type_complexity,
    clippy::needless_borrow,
    clippy::too_many_arguments
)]

pub mod block_light;
pub mod common;
pub mod nibble;
pub mod sets;
pub mod sky_light;
pub mod storage;

pub mod plugin;

// Re-export the moved infrastructure modules so existing `crate::bfs::*` etc.
// import paths inside this crate keep resolving without per-file rewrites.
// The canonical path going forward is `crate::common::*`.
#[cfg(any(test, debug_assertions))]
pub use common::invariants;
pub use common::{
    bfs, bitset, converge, distribute, emit_dirty, enqueue, geom, heightmap, heightmap_update,
    lifecycle, metrics, propagate, table,
};

// Flat re-exports of the per-channel public types and the channel-composer
// plugins so external callers can refer to them at the crate root in
// addition to the canonical per-channel module paths.
pub use block_light::BlockLightPlugin;
pub use block_light::components::{
    BlockBfsPending, BlockBfsQueues, BlockInbox, BlockLight, BlockNeedsInitialSeed, BlockOutbox,
    BlockOutboxDirty, BlockParkedEgress,
};
pub use common::components::{CrossChunkWavefront, IsAllAir, NeedsFullReseed};
pub use sky_light::SkyLightPlugin;
pub use sky_light::components::{
    SkyBfsPending, SkyBfsQueues, SkyInbox, SkyLight, SkyNeedsInitialSeed, SkyOutbox,
    SkyOutboxDirty, SkyParkedEgress, WasTopmostAtSeed,
};

// Aggregate-style `components` module re-exporting all per-channel and
// shared component types for callers that prefer the flat `crate::components::*`
// import path.
pub mod components {
    pub use crate::block_light::components::*;
    pub use crate::common::components::*;
    pub use crate::sky_light::components::*;
}

pub use common::emit_dirty::{BlockLightDirty, SkyLightDirty};
pub use lifecycle::ColumnHeightmapScan;
pub use plugin::LightingPlugin;
pub use sets::LightingSet;

#[cfg(any(feature = "test-bench", feature = "bench-helpers"))]
pub use common::test_bench;
