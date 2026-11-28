use mcrs_minecraft_resource::{Registered, RegistryKey, rl};

pub const MENU: RegistryKey<crate::menu_keys::MenuType> =
    <crate::menu_keys::MenuType as Registered>::REGISTRY;
impl Registered for crate::menu_keys::MenuType {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:menu"));
}
