#![cfg_attr(
    test,
    expect(clippy::disallowed_types, reason = "test input is written as JSON")
)]

pub mod bounds;
#[rustfmt::skip]
pub mod keys;
mod bounded_int_provider;
mod constant_or_dispatch;
mod float_provider;
mod height_context;
mod height_provider;
mod int_provider;
mod sample;
#[cfg(test)]
mod tests;
mod vertical_anchor;
mod weighted;
mod weighted_distribution;

pub use bounded_int_provider::BoundedIntProvider;
pub use float_provider::{DispatchedFloatProvider, FloatProvider};
pub use height_context::HeightContext;
pub use height_provider::{DispatchedHeightProvider, HeightProvider};
pub use int_provider::{DispatchedIntProvider, IntProvider};
pub use vertical_anchor::VerticalAnchor;
pub use weighted::Weighted;
pub use weighted_distribution::{WeightedDistribution, pick_weighted_by};
