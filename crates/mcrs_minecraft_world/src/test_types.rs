use std::collections::BTreeMap;
use std::fmt;

use mcrs_minecraft_core::Rotation;
use mcrs_minecraft_core::codec::{Bounded, default_true, int_value, is_default, is_true};
use mcrs_minecraft_dimension::Dimension;
use mcrs_minecraft_environment::timeline::Timeline;
use mcrs_minecraft_environment::world_clock::WorldClock;
use mcrs_minecraft_game_rule::{GameRule, GameRuleValueType};
use mcrs_minecraft_registries::{Ref, Refs, Resolved, dispatch, registry_value};
use mcrs_minecraft_resource::{ResourceKey, ResourceLocation};
use serde::de::{DeserializeSeed, Error as _, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub type EnvironmentRef<M = Resolved> = Ref<M, TestEnvironment, TestEnvironment<M>>;

#[registry_value]
#[dispatch(registry = mcrs_minecraft_environment::keys::TestEnvironmentDefinitionType)]
#[derive(Debug, Clone, PartialEq)]
pub enum TestEnvironment<M: Refs = Resolved> {
    AllOf {
        definitions: Vec<EnvironmentRef<M>>,
    },
    ClockTime {
        clock: M::Id<WorldClock>,
        time: Bounded<0, { i32::MAX }>,
    },
    #[dispatch(Difficulty)]
    SetDifficulty {
        difficulty: Difficulty,
    },
    #[dispatch(Function)]
    Functions {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        setup: Option<ResourceLocation>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        teardown: Option<ResourceLocation>,
    },
    #[dispatch(GameRules)]
    SetGameRules {
        rules: GameRuleMap,
    },
    #[dispatch(TimelineAttributes)]
    Timelines {
        timelines: Vec<M::Id<Timeline>>,
    },
    Weather {
        weather: Weather,
    },
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

#[derive(Debug, Clone, Copy)]
struct RuleKind(GameRuleValueType);

impl<'de> DeserializeSeed<'de> for RuleKind {
    type Value = GameRuleValue;

    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<GameRuleValue, D::Error> {
        match self.0 {
            GameRuleValueType::Bool { .. } => bool::deserialize(d).map(GameRuleValue::Bool),
            GameRuleValueType::Int { min, max, .. } => {
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
pub struct GameRuleMap(pub BTreeMap<GameRule, GameRuleValue>);

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
                    let rule = GameRule::find(name.as_str())
                        .ok_or_else(|| A::Error::custom(format_args!("{name} is no GameRule")))?;
                    let value = map
                        .next_value_seed(RuleKind(rule.definition().value))
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
    mcrs_minecraft_dimension::keys::dimension::OVERWORLD.into()
}

fn is_overworld(dimension: &ResourceKey<Dimension>) -> bool {
    *dimension == mcrs_minecraft_dimension::keys::dimension::OVERWORLD
}

fn unrotated() -> Rotation {
    Rotation::None
}

fn is_unrotated(rotation: &Rotation) -> bool {
    *rotation == Rotation::None
}

macro_rules! test_instance {
    ($name:ident { $($head:tt)* }) => {
        #[registry_value]
        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
        pub struct $name<M: Refs = Resolved> {
            $($head)*
            pub environment: EnvironmentRef<M>,
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
    pub function: ResourceLocation,
});

#[registry_value]
#[dispatch(registry = mcrs_minecraft_environment::keys::TestInstanceType)]
#[derive(Debug, Clone, PartialEq)]
pub enum TestInstance<M: Refs = Resolved> {
    BlockBased(BlockBasedTest<M>),
    Function(FunctionTest<M>),
}
