use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::registry_key::RegistryValue;

use crate::keys::{ContextFloatProviderType, ContextIntProviderType, ContextKeySet};

/// The part of a loot table an item can name: the context it is rolled in and
/// the random sequence it draws from. Its pools are a column beside the
/// registry.
#[derive(Debug, Clone, PartialEq)]
pub struct LootTable {
    pub context: ContextKeySet,
    pub random_sequence: Option<ResourceLocation>,
}

impl RegistryValue for LootTable {
    type Registry = Self;
}

/// A registered context int provider as an item names it: the type of the
/// expression at its root. The expression is a column beside the registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContextIntProvider {
    pub kind: ContextIntProviderType,
}

impl RegistryValue for ContextIntProvider {
    type Registry = Self;
}

/// A registered context float provider as an item names it: the type of the
/// expression at its root. The expression is a column beside the registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContextFloatProvider {
    pub kind: ContextFloatProviderType,
}

impl RegistryValue for ContextFloatProvider {
    type Registry = Self;
}
