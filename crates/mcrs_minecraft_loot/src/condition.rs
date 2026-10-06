use std::fmt;

use mcrs_minecraft_core::codec::Validate;
use mcrs_minecraft_core::registry_key::RegistryValue;
use mcrs_minecraft_environment::attribute::Operation;
use mcrs_minecraft_environment::attribute::spec::{AttributeSpec, AttributeValue, attribute};
use mcrs_minecraft_environment::keys::EnvironmentAttribute;
use mcrs_minecraft_environment::world_clock::WorldClock;
use mcrs_minecraft_item::component::predicate::{BlockPredicate, ItemPredicate};
use mcrs_minecraft_item::enchantment::EnchantmentData;
use mcrs_minecraft_item::enchantment::value::LevelBasedValue;
use mcrs_minecraft_predicate::{DamageSourcePredicate, EntityPredicate, LocationPredicate};
use mcrs_minecraft_registry::{Holder, HolderList, Id};
use serde::de::{Error as _, MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::buffer::{MapEntries, read_seeded};
use crate::number::{FloatExpression, FloatRangePredicate, IntExpression, IntRangePredicate};

/// A test of the loot context, registered in `predicate` or written inline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum LootCondition {
    #[serde(rename = "minecraft:inverted", alias = "inverted")]
    Inverted(Inverted),
    #[serde(rename = "minecraft:any_of", alias = "any_of")]
    AnyOf(Terms),
    #[serde(rename = "minecraft:all_of", alias = "all_of")]
    AllOf(Terms),
    #[serde(rename = "minecraft:random_chance", alias = "random_chance")]
    RandomChance(RandomChance),
    #[serde(
        rename = "minecraft:random_chance_with_enchanted_bonus",
        alias = "random_chance_with_enchanted_bonus"
    )]
    RandomChanceWithEnchantedBonus(RandomChanceWithEnchantedBonus),
    #[serde(rename = "minecraft:entity_properties", alias = "entity_properties")]
    EntityProperties(Box<EntityProperties>),
    #[serde(rename = "minecraft:killed_by_player", alias = "killed_by_player")]
    KilledByPlayer,
    #[serde(rename = "minecraft:entity_scores", alias = "entity_scores")]
    EntityScores(EntityScores),
    #[serde(rename = "minecraft:match_block", alias = "match_block")]
    MatchBlock(BlockPredicate),
    #[serde(rename = "minecraft:match_tool", alias = "match_tool")]
    MatchTool(MatchTool),
    #[serde(rename = "minecraft:table_bonus", alias = "table_bonus")]
    TableBonus(TableBonus),
    #[serde(rename = "minecraft:survives_explosion", alias = "survives_explosion")]
    SurvivesExplosion,
    #[serde(
        rename = "minecraft:damage_source_properties",
        alias = "damage_source_properties"
    )]
    DamageSourceProperties(Box<DamageSourceProperties>),
    #[serde(rename = "minecraft:location_check", alias = "location_check")]
    LocationCheck(Box<LocationCheck>),
    #[serde(rename = "minecraft:weather_check", alias = "weather_check")]
    WeatherCheck(WeatherCheck),
    #[serde(rename = "minecraft:time_check", alias = "time_check")]
    TimeCheck(TimeCheck),
    #[serde(rename = "minecraft:int_value_check", alias = "int_value_check")]
    IntValueCheck(IntValueCheck),
    #[serde(rename = "minecraft:float_value_check", alias = "float_value_check")]
    FloatValueCheck(FloatValueCheck),
    #[serde(
        rename = "minecraft:enchantment_active_check",
        alias = "enchantment_active_check"
    )]
    EnchantmentActiveCheck(EnchantmentActiveCheck),
    #[serde(
        rename = "minecraft:environment_attribute_check",
        alias = "environment_attribute_check"
    )]
    EnvironmentAttributeCheck(EnvironmentAttributeCheck),
}

impl RegistryValue for LootCondition {
    type Registry = Self;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityTarget {
    This,
    Attacker,
    DirectAttacker,
    AttackingPlayer,
    TargetEntity,
    InteractingEntity,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Inverted {
    pub term: Box<Holder<LootCondition>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Terms {
    pub terms: HolderList<LootCondition>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RandomChance {
    pub chance: Holder<FloatExpression>,
}

mcrs_minecraft_core::validated!(RandomChanceWithEnchantedBonus);

// chisle: an enchantment is named by id only, where the game also takes one written inline; an inline enchantment needs the enchantment file type, which sits above this crate
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub struct RandomChanceWithEnchantedBonus {
    pub unenchanted_chance: f32,
    pub enchanted_chance: LevelBasedValue,
    pub enchantment: Id<EnchantmentData>,
}

impl Validate for RandomChanceWithEnchantedBonus {
    fn validate(&self) -> Result<(), String> {
        if (0.0..=1.0).contains(&self.unenchanted_chance) {
            Ok(())
        } else {
            Err(format!(
                "Value {} outside of range [0.0:1.0]",
                self.unenchanted_chance
            ))
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EntityProperties {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub predicate: Option<EntityPredicate>,
    pub entity: EntityTarget,
}

/// Scores the entity must hold, by objective, in the order read.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EntityScores {
    pub scores: Scores,
    pub entity: EntityTarget,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Scores(pub Vec<(String, IntRangePredicate)>);

impl Serialize for Scores {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(Some(self.0.len()))?;
        for (objective, range) in &self.0 {
            map.serialize_entry(objective, range)?;
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for Scores {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct ScoresVisitor;

        impl<'de> Visitor<'de> for ScoresVisitor {
            type Value = Scores;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a map of objectives to ranges")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Scores, A::Error> {
                let mut scores: Vec<(String, IntRangePredicate)> = Vec::new();
                while let Some(objective) = map.next_key::<String>()? {
                    if scores.iter().any(|(seen, _)| *seen == objective) {
                        return Err(A::Error::custom(format_args!(
                            "Duplicate key '{objective}'"
                        )));
                    }
                    scores.push((objective, map.next_value()?));
                }
                Ok(Scores(scores))
            }
        }

        d.deserialize_map(ScoresVisitor)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MatchTool {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub predicate: Option<ItemPredicate>,
}

mcrs_minecraft_core::validated!(TableBonus);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub struct TableBonus {
    pub enchantment: Id<EnchantmentData>,
    pub chances: Vec<f32>,
}

impl Validate for TableBonus {
    fn validate(&self) -> Result<(), String> {
        if self.chances.is_empty() {
            Err("List must have contents".into())
        } else {
            Ok(())
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DamageSourceProperties {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub predicate: Option<DamageSourcePredicate>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocationCheck {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub predicate: Option<LocationPredicate>,
    #[serde(rename = "offsetX", default, skip_serializing_if = "is_zero")]
    pub offset_x: i32,
    #[serde(rename = "offsetY", default, skip_serializing_if = "is_zero")]
    pub offset_y: i32,
    #[serde(rename = "offsetZ", default, skip_serializing_if = "is_zero")]
    pub offset_z: i32,
}

fn is_zero(value: &i32) -> bool {
    *value == 0
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WeatherCheck {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raining: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thundering: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TimeCheck {
    pub clock: Holder<WorldClock>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub period: Option<i64>,
    pub value: IntRangePredicate,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntValueCheck {
    pub value: Holder<IntExpression>,
    pub test: IntRangePredicate,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FloatValueCheck {
    pub value: Holder<FloatExpression>,
    pub test: FloatRangePredicate,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnchantmentActiveCheck {
    pub active: bool,
}

/// An environment attribute and the value it must hold, read with the
/// attribute's own value codec.
#[derive(Debug, Clone, PartialEq)]
pub struct EnvironmentAttributeCheck {
    pub attribute: EnvironmentAttribute,
    pub value: AttributeValue,
}

fn spec_of(attribute: EnvironmentAttribute) -> &'static AttributeSpec {
    self::attribute(attribute.as_static_str())
        .expect("every environment attribute has a spec, which the attribute table checks")
}

impl Serialize for EnvironmentAttributeCheck {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        struct Value<'a>(&'a EnvironmentAttributeCheck);

        impl Serialize for Value<'_> {
            fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                spec_of(self.0.attribute).serialize_argument(Operation::Override, &self.0.value, s)
            }
        }

        let mut map = s.serialize_map(Some(2))?;
        map.serialize_entry("attribute", &self.attribute)?;
        map.serialize_entry("value", &Value(self))?;
        map.end()
    }
}

impl<'de> Deserialize<'de> for EnvironmentAttributeCheck {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct CheckVisitor;

        impl<'de> Visitor<'de> for CheckVisitor {
            type Value = EnvironmentAttributeCheck;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("an attribute and its value")
            }

            fn visit_map<A: MapAccess<'de>>(
                self,
                map: A,
            ) -> Result<EnvironmentAttributeCheck, A::Error> {
                let mut entries = MapEntries::read(map)?;
                let attribute = entries
                    .take("attribute")
                    .ok_or_else(|| A::Error::missing_field("attribute"))?;
                let attribute: EnvironmentAttribute =
                    read_seeded(std::marker::PhantomData::<EnvironmentAttribute>, attribute)?;
                let value = entries
                    .take("value")
                    .ok_or_else(|| A::Error::missing_field("value"))?;
                let value = read_seeded(spec_of(attribute).value_seed(), value)?;
                let rest: std::collections::BTreeMap<String, serde::de::IgnoredAny> =
                    entries.into_value()?;
                if let Some(unknown) = rest.keys().next() {
                    return Err(A::Error::unknown_field(unknown, &["attribute", "value"]));
                }
                Ok(EnvironmentAttributeCheck { attribute, value })
            }
        }

        d.deserialize_map(CheckVisitor)
    }
}
