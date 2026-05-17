#![allow(
    clippy::type_complexity,
    clippy::needless_borrow,
    clippy::too_many_arguments
)]

pub mod codec;

pub mod common;
pub mod block_light;
pub mod sky_light;

pub mod plugin;

// Re-export the moved infrastructure modules so existing `crate::bfs::*` etc.
// import paths inside this crate keep resolving without per-file rewrites.
// The canonical path going forward is `crate::common::*`.
pub use common::{
    bfs, bitset, converge, distribute, emit_dirty, enqueue, geom, heightmap, heightmap_update,
    lifecycle, propagate, table, telemetry,
};
#[cfg(any(test, debug_assertions))]
pub use common::invariants;

// Flat re-exports of the per-channel public types and the channel-composer
// plugins so external callers can refer to them at the crate root in
// addition to the canonical per-channel module paths.
pub use block_light::BlockLightPlugin;
pub use block_light::components::{
    BlockBfsPending, BlockOutbox, BlockInbox, BlockLight, BlockBfsQueues,
    BlockNeedsInitialSeed, BlockParkedEgress,
};
pub use common::components::{IsAllAir, NeedsFullReseed, CrossChunkWavefront};
pub use sky_light::SkyLightPlugin;
pub use sky_light::components::{
    SkyBfsPending, SkyOutbox, SkyInbox, SkyLight, WasTopmostAtSeed, SkyBfsQueues,
    SkyNeedsInitialSeed, SkyParkedEgress,
};

// Aggregate-style `components` module re-exporting all per-channel and
// shared component types for callers that prefer the flat `crate::components::*`
// import path.
pub mod components {
    pub use crate::block_light::components::*;
    pub use crate::common::components::*;
    pub use crate::sky_light::components::*;
}

pub use codec::{nibble, sets, storage};
pub use codec::codec::{BlockLightDirty, ColumnLightUpdate, SkyLightDirty};
pub use codec::sets::LightingSet;
pub use lifecycle::ColumnHeightmapScan;
pub use plugin::LightingPlugin;

#[cfg(any(feature = "test-bench", feature = "bench-helpers"))]
pub use common::test_bench;
