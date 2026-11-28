use mcrs_minecraft_resource::{ResourceKey, rl};

pub const CLASSIC: ResourceKey<'static, crate::variant::CowSoundVariant> =
    ResourceKey::new(rl!("minecraft:classic"));
pub const MOODY: ResourceKey<'static, crate::variant::CowSoundVariant> =
    ResourceKey::new(rl!("minecraft:moody"));
