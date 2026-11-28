#![expect(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "a test reports a failure by panicking, and the report reads top to bottom"
)]
#![expect(
    clippy::disallowed_methods,
    reason = "the corpus and the program count come from the environment"
)]

mod corpus;
mod device;
mod gpu_fuzz;
mod lowering;
mod twin;
