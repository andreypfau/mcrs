use mcrs_minecraft_resource::{Registered, RegistryKey, rl};

pub const POTION: RegistryKey<crate::potion_keys::Potion> =
    RegistryKey::new(rl!("minecraft:potion"));
impl Registered for crate::potion_keys::Potion {
    const REGISTRY: RegistryKey<Self> = POTION;
}
