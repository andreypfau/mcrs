use mcrs_minecraft_resource::{TagKey, rl};

pub const PLACEABLE: TagKey<'static, crate::PaintingVariantValue> =
    TagKey::new(rl!("minecraft:placeable"));
