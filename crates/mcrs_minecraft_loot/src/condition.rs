use std::fmt;

use mcrs_minecraft_core::codec::{Validate, is_default};
use mcrs_minecraft_core::registry_key::RegistryValue;
use mcrs_minecraft_environment::attribute::Operation;
use mcrs_minecraft_environment::attribute::spec::{AttributeSpec, AttributeValue, attribute};
use mcrs_minecraft_environment::keys::EnvironmentAttribute;
use mcrs_minecraft_environment::world_clock::WorldClock;
use mcrs_minecraft_item::component::predicate::{BlockPredicate, ItemPredicate};
use mcrs_minecraft_item::enchantment::EnchantmentData;
use mcrs_minecraft_item::enchantment::value::LevelBasedValue;
use mcrs_minecraft_predicate::{
    DamageSourcePredicate, EntityPredicate, LocationPredicate, UniqueMap,
};
use mcrs_minecraft_registry::{Holder, HolderList, Id};
use serde::de::{DeserializeSeed, Error as _, IntoDeserializer, MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::buffer::MapEntries;
use crate::number::{FloatExpression, FloatRangePredicate, IntExpression, IntRangePredicate};

/// A test of the loot context, registered in `predicate` or written inline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self")]
pub enum LootCondition {
    Inverted(Inverted),
    AnyOf(Terms),
    AllOf(Terms),
    RandomChance(RandomChance),
    RandomChanceWithEnchantedBonus(RandomChanceWithEnchantedBonus),
    EntityProperties(Box<EntityProperties>),
    KilledByPlayer,
    EntityScores(EntityScores),
    MatchBlock(BlockPredicate),
    MatchTool(MatchTool),
    TableBonus(TableBonus),
    SurvivesExplosion,
    DamageSourceProperties(Box<DamageSourceProperties>),
    LocationCheck(Box<LocationCheck>),
    WeatherCheck(WeatherCheck),
    TimeCheck(TimeCheck),
    IntValueCheck(IntValueCheck),
    FloatValueCheck(FloatValueCheck),
    EnchantmentActiveCheck(EnchantmentActiveCheck),
    EnvironmentAttributeCheck(EnvironmentAttributeCheck),
}

mcrs_minecraft_registry::dispatch! {
    LootCondition, key = "type", registry = crate::keys::LootConditionType,
    {
        Inverted => Inverted,
        AnyOf => AnyOf,
        AllOf => AllOf,
        RandomChance => RandomChance,
        RandomChanceWithEnchantedBonus => RandomChanceWithEnchantedBonus,
        EntityProperties => EntityProperties,
        KilledByPlayer => KilledByPlayer,
        EntityScores => EntityScores,
        MatchBlock => MatchBlock,
        MatchTool => MatchTool,
        TableBonus => TableBonus,
        SurvivesExplosion => SurvivesExplosion,
        DamageSourceProperties => DamageSourceProperties,
        LocationCheck => LocationCheck,
        WeatherCheck => WeatherCheck,
        TimeCheck => TimeCheck,
        IntValueCheck => IntValueCheck,
        FloatValueCheck => FloatValueCheck,
        EnchantmentActiveCheck => EnchantmentActiveCheck,
        EnvironmentAttributeCheck => EnvironmentAttributeCheck,
    }
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

pub type Scores = UniqueMap<String, IntRangePredicate>;

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
    #[serde(rename = "offsetX", default, skip_serializing_if = "is_default")]
    pub offset_x: i32,
    #[serde(rename = "offsetY", default, skip_serializing_if = "is_default")]
    pub offset_y: i32,
    #[serde(rename = "offsetZ", default, skip_serializing_if = "is_default")]
    pub offset_z: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WeatherCheck {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "mcrs_minecraft_core::codec::optional_flag"
    )]
    pub raining: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "mcrs_minecraft_core::codec::optional_flag"
    )]
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
    #[serde(deserialize_with = "mcrs_minecraft_core::codec::flag")]
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
                let attribute = EnvironmentAttribute::deserialize(
                    IntoDeserializer::<A::Error>::into_deserializer(attribute),
                )?;
                let value = entries
                    .take("value")
                    .ok_or_else(|| A::Error::missing_field("value"))?;
                let value = spec_of(attribute)
                    .value_seed()
                    .deserialize(IntoDeserializer::<A::Error>::into_deserializer(value))?;
                if let Some((unknown, _)) = entries.0.first() {
                    return Err(A::Error::unknown_field(unknown, &["attribute", "value"]));
                }
                Ok(EnvironmentAttributeCheck { attribute, value })
            }
        }

        d.deserialize_map(CheckVisitor)
    }
}
