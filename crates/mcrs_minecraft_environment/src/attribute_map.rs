use std::collections::BTreeMap;
use std::fmt;

use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_resource::ResourceLocation;
use serde::de::{self, MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::attribute_value::AttributeValue;
use crate::environment_attribute_keys::EnvironmentAttribute;
use crate::kind::{AttributeType, info};
use crate::operation::{Argument, Operation};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AttributeError {
    #[error("unknown environment attribute `{0}`")]
    UnknownAttribute(String),
    #[error("`{attribute}`: {message}")]
    Refused {
        attribute: &'static str,
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModifierEntry {
    pub modifier: Operation,
    pub argument: Argument,
}

impl ModifierEntry {
    #[must_use]
    pub fn apply(&self, subject: AttributeValue) -> AttributeValue {
        match self.modifier.apply(&subject, &self.argument) {
            Some(value) => value,
            None => subject,
        }
    }
}

pub(crate) fn check_range(
    attribute: EnvironmentAttribute,
    value: &AttributeValue,
) -> Result<(), String> {
    match (value, info(attribute).range) {
        (AttributeValue::Float(value), Some(range)) if !range.contains(*value) => Err(format!(
            "{value} is not in range [{}; {}]",
            range.min, range.max
        )),
        _ => Ok(()),
    }
}

pub fn read_argument(
    attribute: EnvironmentAttribute,
    modifier: Operation,
    tag: &NbtTag,
) -> Result<Argument, String> {
    let kind: AttributeType = info(attribute).kind;
    let argument_kind = modifier
        .argument_kind(kind)
        .ok_or_else(|| format!("the modifier `{modifier}` does not apply to this attribute"))?;
    let argument = Argument::read(argument_kind, tag)?;
    if let (Operation::Override, Argument::Value(value)) = (modifier, &argument) {
        check_range(attribute, value)?;
    }
    Ok(argument)
}

fn read_entry(attribute: EnvironmentAttribute, tag: &NbtTag) -> Result<ModifierEntry, String> {
    let kind = info(attribute).kind;
    if let Ok(value) = AttributeValue::read(kind, tag) {
        check_range(attribute, &value)?;
        return Ok(ModifierEntry {
            modifier: Operation::Override,
            argument: Argument::Value(value),
        });
    }
    let NbtTag::Compound(fields) = tag else {
        return Err(format!(
            "expected a value or an object with `modifier` and `argument`, for a {kind:?} attribute"
        ));
    };
    let modifier: Operation = match fields.get("modifier") {
        Some(NbtTag::String(name)) => name.parse()?,
        Some(_) => return Err("`modifier` is not a string".to_owned()),
        None => return Err("missing `modifier`".to_owned()),
    };
    let argument = fields.get("argument").ok_or("missing `argument`")?;
    Ok(ModifierEntry {
        modifier,
        argument: read_argument(attribute, modifier, argument)?,
    })
}

impl Serialize for ModifierEntry {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.modifier {
            Operation::Override => self.argument.serialize(serializer),
            modifier => {
                let mut map = serializer.serialize_map(Some(2))?;
                map.serialize_entry("argument", &self.argument)?;
                map.serialize_entry("modifier", modifier.name())?;
                map.end()
            }
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ModifierMap(BTreeMap<EnvironmentAttribute, ModifierEntry>);

impl ModifierMap {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[must_use]
    pub fn get(&self, attribute: EnvironmentAttribute) -> Option<&ModifierEntry> {
        self.0.get(&attribute)
    }

    pub fn iter(&self) -> impl Iterator<Item = (EnvironmentAttribute, &ModifierEntry)> {
        self.0.iter().map(|(attribute, entry)| (*attribute, entry))
    }

    #[must_use]
    pub fn filter_syncable(&self) -> ModifierMap {
        ModifierMap(
            self.0
                .iter()
                .filter(|(attribute, _)| info(**attribute).syncable)
                .map(|(attribute, entry)| (*attribute, entry.clone()))
                .collect(),
        )
    }

    pub fn modify(
        &mut self,
        id: ResourceLocation<'_>,
        modifier: Operation,
        argument: impl Serialize,
    ) -> Result<(), AttributeError> {
        let attribute = EnvironmentAttribute::get(id)
            .ok_or_else(|| AttributeError::UnknownAttribute(id.to_string()))?;
        let refused = |message: String| AttributeError::Refused {
            attribute: attribute.as_static_str(),
            message,
        };
        let tag = mcrs_minecraft_nbt::to_nbt_tag(&argument)
            .map_err(|error| refused(error.to_string()))?;
        let argument = read_argument(attribute, modifier, &tag).map_err(refused)?;
        self.0
            .insert(attribute, ModifierEntry { modifier, argument });
        Ok(())
    }
}

impl Serialize for ModifierMap {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut entries: Vec<(&EnvironmentAttribute, &ModifierEntry)> = self.0.iter().collect();
        entries.sort_by_key(|(attribute, _)| attribute.as_static_str());
        serializer.collect_map(entries)
    }
}

impl<'de> Deserialize<'de> for ModifierMap {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct MapVisitor;

        impl<'de> Visitor<'de> for MapVisitor {
            type Value = ModifierMap;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a map from environment attribute ids to values or modifiers")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<ModifierMap, A::Error> {
                let mut entries = BTreeMap::new();
                while let Some(attribute) = map.next_key::<EnvironmentAttribute>()? {
                    let tag: NbtTag = map.next_value()?;
                    let entry = read_entry(attribute, &tag).map_err(|message| {
                        de::Error::custom(format!("{}: {message}", attribute.as_static_str()))
                    })?;
                    if entries.insert(attribute, entry).is_some() {
                        return Err(de::Error::custom(format!(
                            "duplicate attribute `{}`",
                            attribute.as_static_str()
                        )));
                    }
                }
                Ok(ModifierMap(entries))
            }
        }

        deserializer.deserialize_map(MapVisitor)
    }
}
