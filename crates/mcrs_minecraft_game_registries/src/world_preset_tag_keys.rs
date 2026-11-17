use mcrs_minecraft_resource::{TagKey, rl};

pub const EXTENDED: TagKey<'static, crate::worldgen::world_preset::WorldPreset> =
    TagKey::new(rl!("minecraft:extended"));
pub const NORMAL: TagKey<'static, crate::worldgen::world_preset::WorldPreset> =
    TagKey::new(rl!("minecraft:normal"));
