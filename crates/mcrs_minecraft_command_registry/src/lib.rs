//! The command lookup over the registries and the block table that a host lends.

// std: mcrs_minecraft_registries needs it.
mod registry_lookup;

pub use registry_lookup::{DimensionClock, Ids, RegistryLookup};
