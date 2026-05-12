#![allow(
    dead_code,
    unused_variables,
    unused_imports,
    clippy::type_complexity,
    clippy::needless_borrow,
    clippy::too_many_arguments
)]

#[cfg(any(test, feature = "test-bench"))]
pub mod stub;

#[cfg(any(test, feature = "test-bench"))]
pub mod test_bench;
