use std::fmt;

use mcrs_minecraft_core::codec::{self, Validate, is_default};
use mcrs_minecraft_core::{ResourceKey, validated};
use mcrs_minecraft_registry::ItemReg;
use serde::de::{Error as _, MapAccess, Visitor, value};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::hash_ops;
use crate::kind::ItemComponentKind;
use crate::patch::{ComponentPatch, PersistentValue};

validated!(ItemStackValue);

/// Before prototype normalisation: the patch is re-emitted as read.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub struct ItemStackValue {
    #[serde(rename = "id")]
    pub item: ResourceKey<ItemReg>,
    #[serde(default)]
    pub count: codec::Bounded<1, 99, 1>,
    #[serde(default, skip_serializing_if = "ComponentPatch::is_empty")]
    pub components: ComponentPatch,
}

impl Validate for ItemStackValue {
    fn validate(&self) -> Result<(), String> {
        if self.item.as_str() == "minecraft:air" {
            return Err("Item must not be minecraft:air".into());
        }
        Ok(())
    }
}

/// `{}` stands for no stack.
pub mod optional_stack {
    use super::*;

    pub fn serialize<S: Serializer>(
        value: &Option<ItemStackValue>,
        s: S,
    ) -> Result<S::Ok, S::Error> {
        match value {
            Some(stack) => stack.serialize(s),
            None => serde::ser::SerializeMap::end(s.serialize_map(Some(0))?),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        d: D,
    ) -> Result<Option<ItemStackValue>, D::Error> {
        struct OptionalVisitor;

        impl<'de> Visitor<'de> for OptionalVisitor {
            type Value = Option<ItemStackValue>;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("an item stack or an empty map")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let Some(first) = map.next_key::<String>()? else {
                    return Ok(None);
                };
                ItemStackValue::deserialize(value::MapAccessDeserializer::new(Prefixed {
                    first: Some(first),
                    rest: map,
                }))
                .map(Some)
            }
        }

        d.deserialize_map(OptionalVisitor)
    }

    struct Prefixed<A> {
        first: Option<String>,
        rest: A,
    }

    impl<'de, A: MapAccess<'de>> MapAccess<'de> for Prefixed<A> {
        type Error = A::Error;

        fn next_key_seed<K: serde::de::DeserializeSeed<'de>>(
            &mut self,
            seed: K,
        ) -> Result<Option<K::Value>, A::Error> {
            match self.first.take() {
                Some(first) => seed
                    .deserialize(value::StringDeserializer::<A::Error>::new(first))
                    .map(Some),
                None => self.rest.next_key_seed(seed),
            }
        }

        fn next_value_seed<V: serde::de::DeserializeSeed<'de>>(
            &mut self,
            seed: V,
        ) -> Result<V::Value, A::Error> {
            self.rest.next_value_seed(seed)
        }
    }
}

/// A stack written as a map, or as the bare item id when it is one plain item.
#[derive(Clone, Debug, PartialEq)]
pub struct Template(pub ItemStackValue);

impl mcrs_minecraft_text::HoverItem for Template {}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TemplateRepr {
    id: ResourceKey<ItemReg>,
    #[serde(default, skip_serializing_if = "is_default")]
    count: codec::Bounded<1, 99, 1>,
    #[serde(default, skip_serializing_if = "ComponentPatch::is_empty")]
    components: ComponentPatch,
}

impl Serialize for Template {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        TemplateRepr {
            id: self.0.item.clone(),
            count: self.0.count,
            components: self.0.components.clone(),
        }
        .serialize(s)
    }
}

impl<'de> Deserialize<'de> for Template {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct TemplateVisitor;

        impl<'de> Visitor<'de> for TemplateVisitor {
            type Value = Template;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("an item id or an item stack")
            }

            fn visit_str<E: serde::de::Error>(self, id: &str) -> Result<Template, E> {
                let item = ResourceKey::deserialize(value::StrDeserializer::<E>::new(id))?;
                Template::new(item, 1, ComponentPatch::EMPTY).map_err(E::custom)
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Template, A::Error> {
                let repr = TemplateRepr::deserialize(value::MapAccessDeserializer::new(map))?;
                Template::new(repr.id, repr.count.0, repr.components).map_err(A::Error::custom)
            }
        }

        d.deserialize_any(TemplateVisitor)
    }
}

impl Template {
    pub fn new(
        item: ResourceKey<ItemReg>,
        count: i32,
        components: ComponentPatch,
    ) -> Result<Self, String> {
        if count == 0 {
            return Err("Item must be non-empty".into());
        }
        let stack = ItemStackValue {
            item,
            count: codec::Bounded(count),
            components,
        };
        stack.validate()?;
        Ok(Template(stack))
    }
}

pub const MAX_HASHED_COMPONENTS: usize = 256;

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct HashedPatchMap {
    pub added: Vec<(ItemComponentKind, i32)>,
    pub removed: Vec<ItemComponentKind>,
}

impl HashedPatchMap {
    pub fn create(patch: &ComponentPatch) -> anyhow::Result<Self> {
        let added = patch
            .added
            .iter()
            .map(|value| Ok((value.kind(), hash_ops::hash(&PersistentValue(value))?)))
            .collect::<Result<_, hash_ops::HashError>>()?;
        Ok(HashedPatchMap {
            added,
            removed: patch.removed.clone(),
        })
    }

    pub fn matches(&self, patch: &ComponentPatch) -> bool {
        if self.removed.len() != patch.removed.len()
            || !patch.removed.iter().all(|kind| self.removed.contains(kind))
        {
            return false;
        }
        if self.added.len() != patch.added.len() {
            return false;
        }
        patch.added.iter().all(|value| {
            let Some((_, expected)) = self.added.iter().find(|(kind, _)| *kind == value.kind())
            else {
                return false;
            };
            hash_ops::hash(&PersistentValue(value)).is_ok_and(|actual| actual == *expected)
        })
    }
}

/// A stack in a slotted list: the player inventory, the ender chest and a
/// chest's `Items`.
#[derive(Clone, Debug, PartialEq)]
pub struct ItemStackWithSlot {
    pub slot: u8,
    pub stack: ItemStackValue,
}

impl Serialize for ItemStackWithSlot {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut map = s.serialize_map(None)?;
        map.serialize_entry("Slot", &(self.slot as i8))?;
        map.serialize_entry("id", &self.stack.item)?;
        map.serialize_entry("count", &self.stack.count)?;
        if !self.stack.components.is_empty() {
            map.serialize_entry("components", &self.stack.components)?;
        }
        map.end()
    }
}

/// Written field by field: `flatten` would buffer the map and lose the NBT
/// tag types inside `components`.
impl<'de> Deserialize<'de> for ItemStackWithSlot {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct SlotVisitor;

        impl<'de> Visitor<'de> for SlotVisitor {
            type Value = ItemStackWithSlot;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("an item stack with a Slot")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                const FIELDS: [&str; 4] = ["Slot", "id", "count", "components"];
                let mut slot = None;
                let mut item = None;
                let mut count = None;
                let mut components = None;
                while let Some(key) = map.next_key::<String>()? {
                    let taken = match key.as_str() {
                        "Slot" => slot.is_some(),
                        "id" => item.is_some(),
                        "count" => count.is_some(),
                        "components" => components.is_some(),
                        other => return Err(A::Error::unknown_field(other, &FIELDS)),
                    };
                    if taken {
                        return Err(A::Error::custom(format_args!("Duplicate key '{key}'")));
                    }
                    match key.as_str() {
                        "Slot" => slot = Some((map.next_value_seed(IntSeed)? & 0xFF) as u8),
                        "id" => item = Some(map.next_value()?),
                        "count" => count = Some(map.next_value()?),
                        _ => components = Some(map.next_value()?),
                    }
                }
                let stack = ItemStackValue {
                    item: item.ok_or_else(|| A::Error::missing_field("id"))?,
                    count: count.unwrap_or_default(),
                    components: components.unwrap_or_default(),
                };
                stack.validate().map_err(A::Error::custom)?;
                Ok(ItemStackWithSlot {
                    slot: slot.unwrap_or(0),
                    stack,
                })
            }
        }

        d.deserialize_map(SlotVisitor)
    }
}

struct IntSeed;

impl<'de> serde::de::DeserializeSeed<'de> for IntSeed {
    type Value = i32;

    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<i32, D::Error> {
        codec::int_value(d)
    }
}
