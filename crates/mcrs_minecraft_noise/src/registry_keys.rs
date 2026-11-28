use mcrs_minecraft_resource::{Registered, RegistryKey, rl};

pub const NOISE: RegistryKey<crate::proto::NoiseParam> =
    <crate::proto::NoiseParam as Registered>::REGISTRY;
impl Registered for crate::proto::NoiseParam {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:worldgen/noise"));
}
