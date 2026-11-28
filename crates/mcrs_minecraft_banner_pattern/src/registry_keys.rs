use mcrs_minecraft_resource::{Registered, RegistryKey, rl};

pub const BANNER_PATTERN: RegistryKey<crate::BannerPattern> =
    <crate::BannerPattern as Registered>::REGISTRY;
impl Registered for crate::BannerPattern {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:banner_pattern"));
}
