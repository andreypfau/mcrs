use std::fmt;

use mcrs_minecraft_core::ResourceLocation;
use serde::de::{MapAccess, Visitor, value};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::condition::EntityTarget;

/// What a loot context object names: an entity of the context, or its block
/// entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityOrBlock {
    This,
    Attacker,
    DirectAttacker,
    AttackingPlayer,
    TargetEntity,
    InteractingEntity,
    BlockEntity,
}

/// What components are copied from: an entity, the block entity or the tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComponentSource {
    This,
    Attacker,
    DirectAttacker,
    AttackingPlayer,
    TargetEntity,
    InteractingEntity,
    BlockEntity,
    Tool,
}

/// Whose scoreboard name a score is read for.
#[derive(Debug, Clone, PartialEq)]
pub enum ScoreboardNameProvider {
    Context(EntityTarget),
    Fixed(String),
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
enum TypedScoreboardName {
    #[serde(rename = "minecraft:fixed", alias = "fixed")]
    Fixed { name: String },
    #[serde(rename = "minecraft:context", alias = "context")]
    Context { target: EntityTarget },
}

impl Serialize for ScoreboardNameProvider {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            ScoreboardNameProvider::Context(target) => target.serialize(s),
            ScoreboardNameProvider::Fixed(name) => {
                TypedScoreboardName::Fixed { name: name.clone() }.serialize(s)
            }
        }
    }
}

impl<'de> Deserialize<'de> for ScoreboardNameProvider {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct NameVisitor;

        impl<'de> Visitor<'de> for NameVisitor {
            type Value = ScoreboardNameProvider;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("an entity target or a scoreboard name provider")
            }

            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
                EntityTarget::deserialize(value::StrDeserializer::new(v))
                    .map(ScoreboardNameProvider::Context)
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                Ok(
                    match TypedScoreboardName::deserialize(value::MapAccessDeserializer::new(map))?
                    {
                        TypedScoreboardName::Fixed { name } => ScoreboardNameProvider::Fixed(name),
                        TypedScoreboardName::Context { target } => {
                            ScoreboardNameProvider::Context(target)
                        }
                    },
                )
            }
        }

        d.deserialize_any(NameVisitor)
    }
}

/// Where NBT is read from: a context object, or command storage.
#[derive(Debug, Clone, PartialEq)]
pub enum NbtProvider {
    Context(EntityOrBlock),
    Storage(ResourceLocation),
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
enum TypedNbtProvider {
    #[serde(rename = "minecraft:storage", alias = "storage")]
    Storage { source: ResourceLocation },
    #[serde(rename = "minecraft:context", alias = "context")]
    Context { target: EntityOrBlock },
}

impl Serialize for NbtProvider {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            NbtProvider::Context(target) => target.serialize(s),
            NbtProvider::Storage(source) => TypedNbtProvider::Storage {
                source: source.clone(),
            }
            .serialize(s),
        }
    }
}

impl<'de> Deserialize<'de> for NbtProvider {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct NbtVisitor;

        impl<'de> Visitor<'de> for NbtVisitor {
            type Value = NbtProvider;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a context target or an NBT provider")
            }

            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
                EntityOrBlock::deserialize(value::StrDeserializer::new(v)).map(NbtProvider::Context)
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                Ok(
                    match TypedNbtProvider::deserialize(value::MapAccessDeserializer::new(map))? {
                        TypedNbtProvider::Storage { source } => NbtProvider::Storage(source),
                        TypedNbtProvider::Context { target } => NbtProvider::Context(target),
                    },
                )
            }
        }

        d.deserialize_any(NbtVisitor)
    }
}
