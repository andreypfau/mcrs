use mcrs_minecraft_resource::{Registered, RegistryKey, rl};

pub const ITEM: RegistryKey<crate::item_keys::Item> =
    <crate::item_keys::Item as Registered>::REGISTRY;
impl Registered for crate::item_keys::Item {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:item"));
}
