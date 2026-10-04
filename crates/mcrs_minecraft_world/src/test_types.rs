use std::collections::BTreeMap;
use std::fmt;

use mcrs_minecraft_core::codec::{Bounded, default_true, int_value, is_default};
use mcrs_minecraft_core::registry_key::RegistryKey;
use mcrs_minecraft_core::{ResourceKey, ResourceLocation, Rotation, rl};
use mcrs_minecraft_environment::timeline::Timeline;
use mcrs_minecraft_environment::world_clock::WorldClock;
use mcrs_minecraft_registry::key::{Dimension, GameRule, TestFunction};
use mcrs_minecraft_registry::{Holder, Id, Registry};
use serde::de::{DeserializeSeed, Error as _, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum TestEnvironment {
    #[serde(rename = "minecraft:all_of")]
    AllOf {
        definitions: Vec<Holder<TestEnvironment>>,
    },
    #[serde(rename = "minecraft:clock_time")]
    ClockTime {
        clock: Holder<WorldClock>,
        time: Bounded<0, { i32::MAX }>,
    },
    #[serde(rename = "minecraft:difficulty")]
    SetDifficulty { difficulty: Difficulty },
    #[serde(rename = "minecraft:function")]
    Functions {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        setup: Option<ResourceLocation>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        teardown: Option<ResourceLocation>,
    },
    #[serde(rename = "minecraft:game_rules")]
    SetGameRules { rules: GameRuleMap },
    #[serde(rename = "minecraft:timeline_attributes")]
    Timelines { timelines: Vec<Holder<Timeline>> },
    #[serde(rename = "minecraft:weather")]
    Weather { weather: Weather },
}

impl RegistryKey for TestEnvironment {
    const KEY: ResourceLocation<&'static str> = rl!("minecraft:test_environment");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Difficulty {
    Peaceful,
    Easy,
    Normal,
    Hard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Weather {
    Clear,
    Rain,
    Thunder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameRuleValue {
    Bool(bool),
    Int(i32),
}

impl Serialize for GameRuleValue {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match *self {
            GameRuleValue::Bool(value) => s.serialize_bool(value),
            GameRuleValue::Int(value) => s.serialize_i32(value),
        }
    }
}

// chisle: a rule not listed here is read as a boolean, so an integer rule a later game version
// registers is refused until it is added; listing it lifts that.
pub const INTEGER_GAME_RULES: [(&str, i32, i32); 12] = [
    ("minecraft:fire_spread_radius_around_player", -1, i32::MAX),
    ("minecraft:max_block_modifications", 1, i32::MAX),
    ("minecraft:max_command_forks", 0, i32::MAX),
    ("minecraft:max_command_sequence_length", 0, i32::MAX),
    ("minecraft:max_entity_cramming", 0, i32::MAX),
    ("minecraft:max_minecart_speed", 1, 1000),
    ("minecraft:max_snow_accumulation_height", 0, 8),
    (
        "minecraft:players_nether_portal_creative_delay",
        0,
        i32::MAX,
    ),
    ("minecraft:players_nether_portal_default_delay", 0, i32::MAX),
    ("minecraft:players_sleeping_percentage", 0, i32::MAX),
    ("minecraft:random_tick_speed", 0, i32::MAX),
    ("minecraft:respawn_radius", 0, i32::MAX),
];

#[derive(Debug, Clone, Copy)]
enum RuleKind {
    Bool,
    Int { min: i32, max: i32 },
}

impl RuleKind {
    fn of(rule: &str) -> Self {
        INTEGER_GAME_RULES
            .iter()
            .find(|(name, ..)| *name == rule)
            .map_or(RuleKind::Bool, |&(_, min, max)| RuleKind::Int { min, max })
    }
}

impl<'de> DeserializeSeed<'de> for RuleKind {
    type Value = GameRuleValue;

    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<GameRuleValue, D::Error> {
        match self {
            RuleKind::Bool => bool::deserialize(d).map(GameRuleValue::Bool),
            RuleKind::Int { min, max } => {
                let value = int_value(d)?;
                if !(min..=max).contains(&value) {
                    return Err(D::Error::custom(format_args!(
                        "Value {value} outside of range [{min}:{max}]"
                    )));
                }
                Ok(GameRuleValue::Int(value))
            }
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GameRuleMap(pub BTreeMap<Id<GameRule>, GameRuleValue>);

impl Serialize for GameRuleMap {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_map(&self.0)
    }
}

impl<'de> Deserialize<'de> for GameRuleMap {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct RulesVisitor;

        impl<'de> Visitor<'de> for RulesVisitor {
            type Value = GameRuleMap;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a map of game rules to their values")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<GameRuleMap, A::Error> {
                let mut rules = BTreeMap::new();
                while let Some(name) = map.next_key::<ResourceLocation>()? {
                    let rule = Registry::<GameRule>::in_scope("GameRuleMap", |registry| {
                        registry.require(name.as_str())
                    })
                    .map_err(A::Error::custom)?
                    .map_err(A::Error::custom)?;
                    let value = map
                        .next_value_seed(RuleKind::of(name.as_str()))
                        .map_err(|e| A::Error::custom(format_args!("game rule {name}: {e}")))?;
                    rules.insert(rule, value);
                }
                Ok(GameRuleMap(rules))
            }
        }

        d.deserialize_map(RulesVisitor)
    }
}

type Positive = Bounded<1, { i32::MAX }, 1>;
type NonNegative = Bounded<0, { i32::MAX }, 0>;
type Padding = Bounded<0, 128, 0>;

fn overworld() -> ResourceKey<Dimension> {
    ResourceKey::from_location(ResourceLocation::minecraft("overworld"))
}

fn is_overworld(dimension: &ResourceKey<Dimension>) -> bool {
    dimension.as_str() == "minecraft:overworld"
}

fn unrotated() -> Rotation {
    Rotation::None
}

fn is_unrotated(rotation: &Rotation) -> bool {
    *rotation == Rotation::None
}

fn is_true(value: &bool) -> bool {
    *value
}

macro_rules! test_instance {
    ($name:ident { $($head:tt)* }) => {
        #[derive(Debug, Clone, Serialize, Deserialize)]
        #[serde(deny_unknown_fields)]
        pub struct $name {
            $($head)*
            pub environment: Holder<TestEnvironment>,
            #[serde(default = "overworld", skip_serializing_if = "is_overworld")]
            pub dimension: ResourceKey<Dimension>,
            pub structure: ResourceLocation,
            pub max_ticks: Positive,
            #[serde(default, skip_serializing_if = "is_default")]
            pub setup_ticks: NonNegative,
            #[serde(default = "default_true", skip_serializing_if = "is_true")]
            pub required: bool,
            #[serde(default = "unrotated", skip_serializing_if = "is_unrotated")]
            pub rotation: Rotation,
            #[serde(default, skip_serializing_if = "is_default")]
            pub manual_only: bool,
            #[serde(default, skip_serializing_if = "is_default")]
            pub max_attempts: Positive,
            #[serde(default, skip_serializing_if = "is_default")]
            pub required_successes: Positive,
            #[serde(default, skip_serializing_if = "is_default")]
            pub sky_access: bool,
            #[serde(default, skip_serializing_if = "is_default")]
            pub padding: Padding,
        }
    };
}

test_instance!(BlockBasedTest {});
test_instance!(FunctionTest {
    pub function: ResourceKey<TestFunction>,
});

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TestInstance {
    #[serde(rename = "minecraft:block_based")]
    BlockBased(BlockBasedTest),
    #[serde(rename = "minecraft:function")]
    Function(FunctionTest),
}

impl RegistryKey for TestInstance {
    const KEY: ResourceLocation<&'static str> = rl!("minecraft:test_instance");
}
