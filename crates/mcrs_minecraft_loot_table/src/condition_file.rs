use std::collections::BTreeMap;

use mcrs_minecraft_loot::condition::LootCondition;
use mcrs_minecraft_resource::{ResourceLocationBuf, TagKeyBuf};
use serde::Deserialize;
use serde::de::IgnoredAny;

use crate::number_file::NumberFile;
use crate::one_or_many::OneOrMany;

/// A loot condition as a data pack writes it: the key of a registered predicate, or an inline
/// object.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ConditionFile {
    Key(ResourceLocationBuf),
    Inline(Box<TypedCondition>),
}

impl mcrs_minecraft_resource::RegistryValue for ConditionFile {
    type Registry = mcrs_minecraft_loot::condition::LootCondition;
}

mcrs_minecraft_registries::resolve_as_is!(ConditionFile);

/// The terms of `any_of` and `all_of`: a `#` tag of predicates, or conditions.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Terms {
    Tag(PredicateTag),
    List(OneOrMany<ConditionFile>),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(try_from = "String")]
pub struct PredicateTag(pub TagKeyBuf<LootCondition>);

impl TryFrom<String> for PredicateTag {
    type Error = String;

    fn try_from(text: String) -> Result<Self, String> {
        let tag = text.strip_prefix('#').ok_or("not a #tag")?;
        let tag = ResourceLocationBuf::read(tag).map_err(|error| error.to_string())?;
        Ok(PredicateTag(TagKeyBuf::from_location(tag)))
    }
}

/// The fields a part of a predicate does not name.
pub type OtherFields = BTreeMap<String, IgnoredAny>;

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum TypedCondition {
    #[serde(rename = "minecraft:inverted", alias = "inverted")]
    Inverted { term: ConditionFile },
    #[serde(rename = "minecraft:any_of", alias = "any_of")]
    AnyOf { terms: Terms },
    #[serde(rename = "minecraft:all_of", alias = "all_of")]
    AllOf { terms: Terms },
    #[serde(rename = "minecraft:survives_explosion", alias = "survives_explosion")]
    SurvivesExplosion,
    #[serde(rename = "minecraft:random_chance", alias = "random_chance")]
    RandomChance { chance: NumberFile },
    #[serde(rename = "minecraft:table_bonus", alias = "table_bonus")]
    TableBonus {
        enchantment: ResourceLocationBuf,
        #[serde(deserialize_with = "non_empty")]
        chances: Vec<f32>,
    },
    #[serde(rename = "minecraft:match_block", alias = "match_block")]
    MatchBlock(BlockPredicateFile),
    #[serde(rename = "minecraft:match_tool", alias = "match_tool")]
    MatchTool {
        predicate: Option<ItemPredicateFile>,
    },
    #[serde(rename = "minecraft:location_check", alias = "location_check")]
    LocationCheck {
        #[serde(rename = "offsetX", default)]
        offset_x: i32,
        #[serde(rename = "offsetY", default)]
        offset_y: i32,
        #[serde(rename = "offsetZ", default)]
        offset_z: i32,
        predicate: Option<LocationPredicateFile>,
    },
    #[serde(rename = "minecraft:entity_properties", alias = "entity_properties")]
    EntityProperties {
        entity: String,
        predicate: Option<OtherFields>,
    },
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BlockPredicateFile {
    pub blocks: Option<OneOrMany<String>>,
    pub state: Option<BTreeMap<String, StateValue>>,
    #[serde(flatten)]
    pub other: OtherFields,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum StateValue {
    Exact(String),
    Other(IgnoredAny),
}

#[derive(Debug, Clone, Deserialize)]
pub struct LocationPredicateFile {
    pub block: Option<BlockPredicateFile>,
    #[serde(flatten)]
    pub other: OtherFields,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ItemPredicateFile {
    pub items: Option<OneOrMany<String>>,
    pub predicates: Option<ItemSubPredicates>,
    #[serde(flatten)]
    pub other: OtherFields,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ItemSubPredicates {
    #[serde(rename = "minecraft:enchantments")]
    pub enchantments: Option<Vec<EnchantmentPredicateFile>>,
    #[serde(flatten)]
    pub other: OtherFields,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EnchantmentPredicateFile {
    pub enchantments: Option<OneOrMany<String>>,
    pub levels: Option<LevelsFile>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(untagged)]
pub enum LevelsFile {
    Exact(u32),
    Range { min: Option<u32>, max: Option<u32> },
}

fn non_empty<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Vec<f32>, D::Error> {
    let chances = Vec::<f32>::deserialize(deserializer)?;
    if chances.is_empty() {
        return Err(serde::de::Error::invalid_length(0, &"at least one chance"));
    }
    Ok(chances)
}
