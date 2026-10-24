#![expect(
    clippy::indexing_slicing,
    clippy::unwrap_used,
    reason = "a test reports a failure by panicking"
)]
mod feature_sort;
mod placed_feature_set;
