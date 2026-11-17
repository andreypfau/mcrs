use mcrs_minecraft_resource::{Registered, RegistryKey, rl};

pub const BANNER_PATTERN: RegistryKey<crate::BannerPattern> =
    RegistryKey::new(rl!("minecraft:banner_pattern"));
impl Registered for crate::BannerPattern {
    const REGISTRY: RegistryKey<Self> = BANNER_PATTERN;
}
