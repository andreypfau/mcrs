// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

pub mod game_rule;

pub use game_rule::GameRule;

use mcrs_minecraft_core::{RegistryKey, TypeBinding, rl};
use mcrs_minecraft_registry::Registered;

pub const GAME_RULE: RegistryKey<crate::keys::GameRule> = RegistryKey::new(rl!("minecraft:game_rule"));
impl Registered for crate::keys::GameRule {
    const REGISTRY: RegistryKey<Self> = GAME_RULE;
}

pub fn bindings() -> [TypeBinding; 1] {
    [
        GAME_RULE.binding(),
    ]
}
