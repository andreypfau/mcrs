#[rustfmt::skip]
pub mod density_function_keys;
#[rustfmt::skip]
pub mod density_function_type_keys;
#[rustfmt::skip]
pub mod registry_keys;
pub mod aquifer;
pub mod aquifer_cells;
pub mod beta;
pub mod bounds;
mod branch;
pub mod cell;
pub mod column_bound;
pub mod compile;
pub mod node;
pub mod program;
pub mod proto;
pub mod router;
pub mod router_key;
pub mod surface_levels;
mod window;

pub const FAST_PROFILE: bool = cfg!(feature = "fast");

mcrs_minecraft_registries::resolve_as_is!(
    crate::proto::DensityFunctionHolder,
    crate::router::NoiseGeneratorSettings
);
