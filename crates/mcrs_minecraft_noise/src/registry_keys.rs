use mcrs_minecraft_resource::{Registered, RegistryKey, rl};

pub const NOISE: RegistryKey<crate::proto::NoiseParam> =
    RegistryKey::new(rl!("minecraft:worldgen/noise"));
impl Registered for crate::proto::NoiseParam {
    const REGISTRY: RegistryKey<Self> = NOISE;
}
