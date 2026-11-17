use mcrs_minecraft_resource::{ResourceKey, rl};

pub const CLASSIC: ResourceKey<'static, crate::variant::CatSoundVariant> =
    ResourceKey::new(rl!("minecraft:classic"));
pub const ROYAL: ResourceKey<'static, crate::variant::CatSoundVariant> =
    ResourceKey::new(rl!("minecraft:royal"));
