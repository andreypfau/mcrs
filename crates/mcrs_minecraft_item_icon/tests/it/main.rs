#![expect(clippy::expect_used, reason = "a test reports a failure by panicking")]

mod alloc_free;
mod caps;
mod corpus;
mod exhaustive;
mod meshes;
mod resolve;
