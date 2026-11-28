#![expect(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "a test reports a failure by panicking"
)]
#![expect(
    clippy::disallowed_methods,
    reason = "the corpus and the proptest case count come from the environment"
)]

mod channel_class;
mod corpus;
mod differential;
#[path = "../entity_queries/mod.rs"]
mod entity_queries;
#[path = "../generator/mod.rs"]
mod generator;
mod spec;
