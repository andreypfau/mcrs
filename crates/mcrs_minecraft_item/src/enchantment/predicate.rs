use std::fmt;

use serde::de::{self, MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::value::{Bounds, HolderSet, NumberProvider};

/// A map whose key selects both the field and the type of its value, the way
/// `Codec.dispatchedMap` does. An unknown key is an error naming it, so a
/// malformed pack fails at load rather than at use.
macro_rules! dispatched_map {
    (
        $(#[$meta:meta])*
        $name:ident { $($key:literal => $field:ident : $ty:ty),+ $(,)? }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Default, PartialEq)]
        pub struct $name {
            $(pub $field: Option<$ty>,)+
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                struct V;

                impl<'de> Visitor<'de> for V {
                    type Value = $name;

                    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                        f.write_str(concat!("a ", stringify!($name), " map"))
                    }

                    fn visit_map<A: MapAccess<'de>>(mut self, mut map: A) -> Result<$name, A::Error> {
                        let _ = &mut self;
                        let mut out = <$name>::default();
                        while let Some(key) = map.next_key::<String>()? {
                            match key.as_str() {
                                $($key => {
                                    if out.$field.is_some() {
                                        return Err(de::Error::custom(
                                            format!("duplicate entry `{}`", $key)));
                                    }
                                    let value: $ty = map.next_value().map_err(|e| {
                                        de::Error::custom(format!("`{}`: {e}", $key))
                                    })?;
                                    out.$field = Some(value);
                                })+
                                unknown => {
                                    return Err(de::Error::custom(format!(
                                        concat!("unknown ", stringify!($name), " entry `{}`"),
                                        unknown
                                    )));
                                }
                            }
                        }
                        Ok(out)
                    }
                }

                d.deserialize_map(V)
            }
        }

        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                let len = 0usize $(+ self.$field.is_some() as usize)+;
                let mut map = s.serialize_map(Some(len))?;
                $(if let Some(value) = &self.$field {
                    map.serialize_entry($key, value)?;
                })+
                map.end()
            }
        }
    };
}

pub(crate) use dispatched_map;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityTarget {
    This,
    Attacker,
    DirectAttacker,
    AttackingPlayer,
    TargetEntity,
    InteractingEntity,
}

/// Java's `LootItemCondition`, in the forms the shipped enchantments state.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum LootCondition {
    #[serde(rename = "minecraft:all_of")]
    AllOf { terms: Vec<LootCondition> },
    #[serde(rename = "minecraft:any_of")]
    AnyOf { terms: Vec<LootCondition> },
    #[serde(rename = "minecraft:inverted")]
    Inverted { term: Box<LootCondition> },
    #[serde(rename = "minecraft:match_tool")]
    MatchTool { predicate: ItemPredicate },
    #[serde(rename = "minecraft:entity_properties")]
    EntityProperties {
        entity: EntityTarget,
        predicate: EntityPredicate,
    },
    #[serde(rename = "minecraft:damage_source_properties")]
    DamageSourceProperties { predicate: DamageSourcePredicate },
    #[serde(rename = "minecraft:location_check")]
    LocationCheck {
        predicate: LocationPredicate,
        #[serde(rename = "offsetX", default, skip_serializing_if = "Option::is_none")]
        offset_x: Option<i32>,
        #[serde(rename = "offsetY", default, skip_serializing_if = "Option::is_none")]
        offset_y: Option<i32>,
        #[serde(rename = "offsetZ", default, skip_serializing_if = "Option::is_none")]
        offset_z: Option<i32>,
    },
    #[serde(rename = "minecraft:weather_check")]
    WeatherCheck {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        raining: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        thundering: Option<bool>,
    },
    #[serde(rename = "minecraft:random_chance")]
    RandomChance { chance: NumberProvider },
    #[serde(rename = "minecraft:enchantment_active_check")]
    EnchantmentActiveCheck { active: bool },
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ItemPredicate {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub items: Option<HolderSet>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<Bounds<i32>>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DamageSourcePredicate {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<TagPredicate>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_direct: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direct_entity: Option<EntityPredicate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_entity: Option<EntityPredicate>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TagPredicate {
    pub id: String,
    pub expected: bool,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LocationPredicate {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block: Option<LocationBlockPredicate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub can_see_sky: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub smokey: Option<bool>,
}

/// The block half of a `LocationPredicate`, which names blocks rather than
/// testing a position the way the worldgen [`BlockPredicate`] does.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LocationBlockPredicate {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocks: Option<HolderSet>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EntityFlagsPredicate {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_on_fire: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_sneaking: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_sprinting: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_swimming: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_baby: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_on_ground: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_flying: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_in_water: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_fall_flying: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MovementPredicate {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x: Option<Bounds<f64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y: Option<Bounds<f64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub z: Option<Bounds<f64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speed: Option<Bounds<f64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub horizontal_speed: Option<Bounds<f64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vertical_speed: Option<Bounds<f64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fall_distance: Option<Bounds<f64>>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PlayerPredicate {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gamemode: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub food: Option<FoodPredicate>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FoodPredicate {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<Bounds<i32>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub saturation: Option<Bounds<f64>>,
}

dispatched_map! {
    /// Java's component-keyed `EntityPredicate`: the key names the sub-predicate
    /// and so chooses the type of its value.
    EntityPredicate {
        "minecraft:entity_type" => entity_type: HolderSet,
        "minecraft:flags" => flags: EntityFlagsPredicate,
        "minecraft:movement" => movement: MovementPredicate,
        "minecraft:movement_affected_by" => movement_affected_by: LocationPredicate,
        "minecraft:location" => location: LocationPredicate,
        "minecraft:periodic_tick" => periodic_tick: i32,
        "minecraft:vehicle" => vehicle: Box<EntityPredicate>,
        "minecraft:type_specific/player" => player: PlayerPredicate,
    }
}

/// Java's worldgen `BlockPredicate`, which tests a position rather than naming
/// a block the way [`LocationBlockPredicate`] does.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum BlockPredicate {
    #[serde(rename = "minecraft:all_of")]
    AllOf { predicates: Vec<BlockPredicate> },
    #[serde(rename = "minecraft:any_of")]
    AnyOf { predicates: Vec<BlockPredicate> },
    #[serde(rename = "minecraft:matching_blocks")]
    MatchingBlocks {
        blocks: HolderSet,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        offset: Option<[i32; 3]>,
    },
    #[serde(rename = "minecraft:matching_block_tag")]
    MatchingBlockTag {
        tag: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        offset: Option<[i32; 3]>,
    },
    #[serde(rename = "minecraft:matching_fluids")]
    MatchingFluids {
        fluids: HolderSet,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        offset: Option<[i32; 3]>,
    },
    #[serde(rename = "minecraft:unobstructed")]
    Unobstructed {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        offset: Option<[i32; 3]>,
    },
}
