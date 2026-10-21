#[rustfmt::skip]
pub mod keys;
pub mod effects;
pub mod file;

mcrs_minecraft_registries::resolve_as_is!(crate::effects::EnchantmentEffects);

mcrs_minecraft_registries::declare_registries!(
    crate::file::EnchantmentFile => mcrs_minecraft_item::keys::ENCHANTMENT, World;
);
