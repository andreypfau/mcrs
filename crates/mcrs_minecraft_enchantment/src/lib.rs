#[rustfmt::skip]
pub mod keys;
pub mod effects;
pub mod file;

mcrs_minecraft_registries::read_with_serde!(crate::file::EnchantmentFile);
