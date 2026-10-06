use mcrs_minecraft_item::damage_type::DamageType;
use mcrs_minecraft_registry::HolderSet;
use serde::{Deserialize, Serialize};

use crate::entity::EntityPredicate;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DamageSourcePredicate {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<TagPredicate<DamageType>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direct_entity: Option<EntityPredicate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_entity: Option<EntityPredicate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_direct: Option<bool>,
}

/// Whether a value is in a set of its registry, or is not.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, bound(serialize = "", deserialize = ""))]
pub struct TagPredicate<T: 'static> {
    pub id: HolderSet<T>,
    pub expected: bool,
}
