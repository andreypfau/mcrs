use mcrs_minecraft_resource::{Registered, RegistryKey, rl};

pub const ITEM: RegistryKey<crate::item_keys::Item> = RegistryKey::new(rl!("minecraft:item"));
impl Registered for crate::item_keys::Item {
    const REGISTRY: RegistryKey<Self> = ITEM;
}
