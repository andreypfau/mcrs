use mcrs_minecraft_resource::{ResourceKey, rl};

pub const CLASSIC: ResourceKey<'static, crate::variant::ChickenSoundVariant> =
    ResourceKey::new(rl!("minecraft:classic"));
pub const PICKY: ResourceKey<'static, crate::variant::ChickenSoundVariant> =
    ResourceKey::new(rl!("minecraft:picky"));
