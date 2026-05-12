#![allow(
    dead_code,
    unused_variables,
    unused_imports,
    clippy::type_complexity,
    clippy::needless_borrow,
    clippy::too_many_arguments
)]

pub mod bfs;
pub mod enqueue;
pub mod nibble;
pub mod storage;
pub mod components;
pub mod bundle;
pub mod table;
pub mod lifecycle;
pub mod heightmap_update;
pub mod plugin;
pub mod sets;

#[cfg(any(test, debug_assertions))]
pub mod invariants;

pub use plugin::LightingPlugin;

#[cfg(feature = "test-bench")]
pub mod stub;
#[cfg(feature = "test-bench")]
pub mod test_bench;
