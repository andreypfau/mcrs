#![expect(clippy::unwrap_used, reason = "a test reports a failure by panicking")]

mod alloc_free;
mod block;
