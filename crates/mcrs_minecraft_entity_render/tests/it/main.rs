#![expect(clippy::unwrap_used, reason = "a test reports a failure by panicking")]

mod figure_view;
#[expect(dead_code, reason = "the tests use a part of the test support")]
#[path = "../support/mod.rs"]
mod support;
mod view_mask;
