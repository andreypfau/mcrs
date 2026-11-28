use mcrs_minecraft_resource::{ResourceKey, rl};

pub const COLD: ResourceKey<'static, crate::variant::PigVariant> =
    ResourceKey::new(rl!("minecraft:cold"));
pub const TEMPERATE: ResourceKey<'static, crate::variant::PigVariant> =
    ResourceKey::new(rl!("minecraft:temperate"));
pub const WARM: ResourceKey<'static, crate::variant::PigVariant> =
    ResourceKey::new(rl!("minecraft:warm"));
