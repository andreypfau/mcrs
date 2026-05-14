use crate::entity::EntityType;
use mcrs_core::tag::key::TagKey;

// Tags referenced by enchantments via `requirements.predicate.type`.
pub const ARROWS: TagKey<EntityType> = TagKey::new(mcrs_core::rl!("minecraft:arrows"));
pub const SENSITIVE_TO_BANE_OF_ARTHROPODS: TagKey<EntityType> =
    TagKey::new(mcrs_core::rl!("minecraft:sensitive_to_bane_of_arthropods"));
pub const SENSITIVE_TO_IMPALING: TagKey<EntityType> =
    TagKey::new(mcrs_core::rl!("minecraft:sensitive_to_impaling"));
pub const SENSITIVE_TO_SMITE: TagKey<EntityType> =
    TagKey::new(mcrs_core::rl!("minecraft:sensitive_to_smite"));

pub const ALL_ENTITY_TYPE_TAGS: &[TagKey<EntityType>] = &[
    ARROWS,
    SENSITIVE_TO_BANE_OF_ARTHROPODS,
    SENSITIVE_TO_IMPALING,
    SENSITIVE_TO_SMITE,
];
