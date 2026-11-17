use core::num::NonZeroI64;

use mcrs_minecraft_enchantment::level_based_value::LevelBasedValue;
use mcrs_minecraft_item::component::predicate::{BlockPredicate, ItemPredicate};
use mcrs_minecraft_loot::condition::LootCondition;
use mcrs_minecraft_predicate::{DamageSourcePredicate, EntityPredicate, LocationPredicate};
use mcrs_minecraft_registries::Named;
use mcrs_minecraft_resource::{ResourceLocationBuf, TagKeyBuf};
use serde::de::Error;
use serde::{Deserialize, Deserializer};

use crate::attribute_check::AttributeCheck;
use crate::entity_target::EntityTarget;
use crate::float_provider_file::FloatProviderFile;
use crate::int_provider_file::IntProviderFile;
use crate::level_based::checked_level_based;
use crate::one_or_many::OneOrMany;
use crate::range_file::RangeFile;
use crate::scores_file::ScoresFile;

#[derive(Debug, Clone, Deserialize)]
#[serde(transparent)]
pub struct ConditionFile(pub(crate) ConditionForm);

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub(crate) enum ConditionForm {
    Key(ResourceLocationBuf),
    Inline(Box<TypedCondition>),
}

impl mcrs_minecraft_resource::RegistryValue for ConditionFile {
    type Registry = mcrs_minecraft_loot::condition::LootCondition;
}

mcrs_minecraft_registries::resolve_as_is!(ConditionFile);

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub(crate) enum Terms {
    Tag(PredicateTag),
    List(OneOrMany<ConditionFile>),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(try_from = "String")]
pub(crate) struct PredicateTag(pub(crate) TagKeyBuf<LootCondition>);

impl TryFrom<String> for PredicateTag {
    type Error = String;

    fn try_from(text: String) -> Result<Self, String> {
        let tag = text.strip_prefix('#').ok_or("not a #tag")?;
        let tag = ResourceLocationBuf::read(tag).map_err(|error| error.to_string())?;
        Ok(PredicateTag(TagKeyBuf::from_location(tag)))
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub(crate) enum TypedCondition {
    #[serde(rename = "minecraft:inverted", alias = "inverted")]
    Inverted { term: ConditionFile },
    #[serde(rename = "minecraft:any_of", alias = "any_of")]
    AnyOf { terms: Terms },
    #[serde(rename = "minecraft:all_of", alias = "all_of")]
    AllOf { terms: Terms },
    #[serde(rename = "minecraft:random_chance", alias = "random_chance")]
    RandomChance { chance: FloatProviderFile },
    #[serde(
        rename = "minecraft:random_chance_with_enchanted_bonus",
        alias = "random_chance_with_enchanted_bonus"
    )]
    RandomChanceWithEnchantedBonus {
        enchantment: ResourceLocationBuf,
        #[serde(deserialize_with = "unit_interval")]
        unenchanted_chance: f32,
        #[serde(deserialize_with = "checked_level_based")]
        enchanted_chance: LevelBasedValue,
    },
    #[serde(rename = "minecraft:entity_properties", alias = "entity_properties")]
    EntityProperties {
        entity: EntityTarget,
        predicate: Option<Box<EntityPredicate<Named>>>,
    },
    #[serde(rename = "minecraft:killed_by_player", alias = "killed_by_player")]
    KilledByPlayer,
    #[serde(rename = "minecraft:survives_explosion", alias = "survives_explosion")]
    SurvivesExplosion,
    #[serde(rename = "minecraft:entity_scores", alias = "entity_scores")]
    EntityScores {
        entity: EntityTarget,
        scores: ScoresFile,
    },
    #[serde(rename = "minecraft:match_block", alias = "match_block")]
    MatchBlock(Box<BlockPredicate<Named>>),
    #[serde(rename = "minecraft:match_tool", alias = "match_tool")]
    MatchTool {
        predicate: Option<Box<ItemPredicate<Named>>>,
    },
    #[serde(rename = "minecraft:table_bonus", alias = "table_bonus")]
    TableBonus {
        enchantment: ResourceLocationBuf,
        #[serde(deserialize_with = "non_empty")]
        chances: Vec<f32>,
    },
    #[serde(
        rename = "minecraft:damage_source_properties",
        alias = "damage_source_properties"
    )]
    DamageSourceProperties {
        predicate: Option<Box<DamageSourcePredicate<Named>>>,
    },
    #[serde(rename = "minecraft:location_check", alias = "location_check")]
    LocationCheck {
        #[serde(rename = "offsetX")]
        offset_x: Option<i32>,
        #[serde(rename = "offsetY")]
        offset_y: Option<i32>,
        #[serde(rename = "offsetZ")]
        offset_z: Option<i32>,
        predicate: Option<Box<LocationPredicate<Named>>>,
    },
    #[serde(rename = "minecraft:weather_check", alias = "weather_check")]
    WeatherCheck {
        raining: Option<bool>,
        thundering: Option<bool>,
    },
    #[serde(rename = "minecraft:time_check", alias = "time_check")]
    TimeCheck {
        clock: ResourceLocationBuf,
        period: Option<NonZeroI64>,
        value: RangeFile<IntProviderFile>,
    },
    #[serde(rename = "minecraft:int_value_check", alias = "int_value_check")]
    IntValueCheck {
        value: IntProviderFile,
        test: RangeFile<IntProviderFile>,
    },
    #[serde(rename = "minecraft:float_value_check", alias = "float_value_check")]
    FloatValueCheck {
        value: FloatProviderFile,
        test: RangeFile<FloatProviderFile>,
    },
    #[serde(
        rename = "minecraft:enchantment_active_check",
        alias = "enchantment_active_check"
    )]
    EnchantmentActiveCheck { active: bool },
    #[serde(
        rename = "minecraft:environment_attribute_check",
        alias = "environment_attribute_check"
    )]
    EnvironmentAttributeCheck(AttributeCheck),
}

fn non_empty<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<f32>, D::Error> {
    let chances = Vec::<f32>::deserialize(deserializer)?;
    if chances.is_empty() {
        return Err(D::Error::invalid_length(0, &"at least one chance"));
    }
    Ok(chances)
}

fn unit_interval<'de, D: Deserializer<'de>>(deserializer: D) -> Result<f32, D::Error> {
    let chance = f32::deserialize(deserializer)?;
    match (0.0..=1.0).contains(&chance) {
        true => Ok(chance),
        false => Err(D::Error::custom("a chance outside 0 to 1")),
    }
}
