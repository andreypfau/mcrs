// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

pub mod noise;

use mcrs_minecraft_core::{Registered, RegistryKey, rl};

pub const NOISE: RegistryKey<crate::proto::NoiseParam> =
    RegistryKey::new(rl!("minecraft:worldgen/noise"));
impl Registered for crate::proto::NoiseParam {
    const REGISTRY: RegistryKey<Self> = NOISE;
}
