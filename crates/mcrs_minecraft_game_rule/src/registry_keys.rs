// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_resource::{Registered, RegistryKey, rl};

pub const GAME_RULE: RegistryKey<crate::game_rule_keys::GameRule> =
    RegistryKey::new(rl!("minecraft:game_rule"));
impl Registered for crate::game_rule_keys::GameRule {
    const REGISTRY: RegistryKey<Self> = GAME_RULE;
}
