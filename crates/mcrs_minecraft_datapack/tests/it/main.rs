mod stack;

use mcrs_minecraft_core::ResourceLocation;

pub const META: &str = r#"{"pack":{"description":"x","min_format":122,"max_format":122}}"#;

pub fn rl(text: &str) -> ResourceLocation {
    ResourceLocation::read(text).unwrap()
}
