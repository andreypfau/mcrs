#![allow(
    dead_code,
    unused_variables,
    unused_imports,
    clippy::type_complexity,
    clippy::needless_borrow,
    clippy::too_many_arguments
)]

pub mod nibble;
pub mod storage;
pub mod components;
pub mod bundle;
pub mod table;
pub mod lifecycle;
pub mod plugin;

pub use plugin::LightingPlugin;

#[cfg(feature = "test-bench")]
pub mod stub;
#[cfg(feature = "test-bench")]
pub mod test_bench;
