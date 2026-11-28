use mcrs_minecraft_resource::{ResourceKey, rl};

pub const DEFAULT: ResourceKey<'static, crate::test_types::TestEnvironment> =
    ResourceKey::new(rl!("minecraft:default"));
