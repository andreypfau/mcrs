#![expect(
    clippy::panic,
    clippy::unwrap_used,
    reason = "a test reports a failure by panicking"
)]
mod ore_vein_parity;
mod ore_vein_pins;
mod tree_geometry_parity;
mod tree_geometry_pins;
