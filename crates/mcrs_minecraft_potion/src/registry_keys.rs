use mcrs_minecraft_resource::{Registered, RegistryKey, rl};

pub const POTION: RegistryKey<crate::potion_keys::Potion> =
    <crate::potion_keys::Potion as Registered>::REGISTRY;
impl Registered for crate::potion_keys::Potion {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:potion"));
}
