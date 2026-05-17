#![allow(
    clippy::type_complexity,
    clippy::needless_borrow,
    clippy::too_many_arguments
)]

pub mod codec;

pub mod common;
pub mod block_light;
pub mod sky_light;

// `bundle`, `enqueue`, `propagate`, `distribute`, `emit_dirty`, `invariants`,
// and `plugin` still hold the channel-shared system bodies at the crate
// root. The per-channel modules under `block_light/` and `sky_light/`
// re-export each channel's surface as the canonical import path; the
// flat files here remain the implementation home until a later refactor
// moves the bodies (the channel boundary in the existing chained
// schedule tuples still crosses).
pub mod bundle;
pub mod enqueue;
pub mod invariants;
pub mod propagate;
pub mod plugin;
pub mod distribute;
pub mod emit_dirty;

// Re-export the moved infrastructure modules so existing `crate::bfs::*` etc.
// import paths inside this crate keep resolving without per-file rewrites.
// The canonical path going forward is `crate::common::bfs::*`; the aliases
// here are bridging plumbing and may be tightened in later refactors.
pub use common::{
    bfs, bitset, converge, geom, heightmap, heightmap_update, lifecycle, table, telemetry,
};

// Flat re-exports of the per-channel public types and the channel-composer
// plugins so external callers can refer to them at the crate root in
// addition to the canonical per-channel module paths.
pub use block_light::BlockLightPlugin;
pub use block_light::components::{
    BlockBfsPending, BlockEgress, BlockIncoming, BlockLight, BlockLightWorkspace,
    BlockNeedsInitialSeed, BlockPendingEgress,
};
pub use common::components::{IsAllAir, NeedsFullReseed, Wavefront};
pub use sky_light::SkyLightPlugin;
pub use sky_light::components::{
    SkyBfsPending, SkyEgress, SkyIncoming, SkyLight, SkyLightSeededAsTopmost, SkyLightWorkspace,
    SkyNeedsInitialSeed, SkyPendingEgress,
};

pub use codec::{components, nibble, sets, storage};
pub use codec::codec::{BlockLightDirty, ColumnLightUpdate, SkyLightDirty};
pub use codec::sets::LightingSet;
pub use lifecycle::ColumnHeightmapScan;
pub use plugin::LightingPlugin;

#[cfg(any(feature = "test-bench", feature = "bench-helpers"))]
pub use common::test_bench;
