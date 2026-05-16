#![allow(
    dead_code,
    unused_variables,
    unused_imports,
    clippy::type_complexity,
    clippy::needless_borrow,
    clippy::too_many_arguments
)]

pub mod codec;

pub mod bundle;
pub mod table;
mod bitset;
pub mod lifecycle;
pub mod heightmap_update;
pub mod bfs;
pub mod enqueue;
pub mod invariants;
pub mod propagate;
pub mod plugin;
pub mod converge;
pub mod distribute;
pub mod emit_dirty;
pub mod telemetry;

pub use codec::{components, nibble, sets, storage};
pub use codec::codec::{BlockLightDirty, ColumnLightUpdate, SkyLightDirty};
pub use codec::sets::LightingSet;
pub use lifecycle::ColumnHeightmapScan;
pub use plugin::LightingPlugin;

#[cfg(feature = "test-bench")]
pub mod stub;
#[cfg(any(feature = "test-bench", feature = "bench-helpers"))]
pub mod test_bench;
