use mcrs_minecraft_resource::{Registered, RegistryKey, rl};

pub const GAME_RULE: RegistryKey<crate::game_rule_keys::GameRule> =
    <crate::game_rule_keys::GameRule as Registered>::REGISTRY;
impl Registered for crate::game_rule_keys::GameRule {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:game_rule"));
}
