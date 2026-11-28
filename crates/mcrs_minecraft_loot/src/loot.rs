use mcrs_minecraft_resource::ResourceLocationBuf;

use crate::{
    context_float_provider_type_keys::ContextFloatProviderType,
    context_int_provider_type_keys::ContextIntProviderType, context_key_set_keys::ContextKeySet,
};

#[derive(Debug, Clone, PartialEq)]
pub struct LootTable {
    pub context: ContextKeySet,
    pub random_sequence: Option<ResourceLocationBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContextIntProvider {
    pub kind: ContextIntProviderType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContextFloatProvider {
    pub kind: ContextFloatProviderType,
}
