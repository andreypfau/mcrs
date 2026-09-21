use std::fmt;

use mcrs_minecraft_core::codec::{self, Validate, is_default};
use mcrs_minecraft_core::{ResourceKey, validated};
use mcrs_minecraft_registry::ItemReg;
use serde::de::{Error as _, MapAccess, Visitor, value};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::component::common::map_only;
use crate::hash_ops;
use crate::kind::ItemComponentKind;
use crate::patch::ComponentPatch;

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
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Repr {
            #[serde(default, deserialize_with = "present")]
            id: Option<ResourceKey<ItemReg>>,
            #[serde(default, deserialize_with = "present")]
            count: Option<codec::Bounded<1, 99, 1>>,
            #[serde(default, deserialize_with = "present")]
            components: Option<ComponentPatch>,
        }

        let repr: Repr = map_only(d)?;
        let item = match repr {
            Repr {
                id: None,
                count: None,
                components: None,
            } => return Ok(None),
            Repr { id: Some(id), .. } => id,
            Repr { id: None, .. } => return Err(D::Error::missing_field("id")),
        };
        let stack = ItemStackValue {
            item,
            count: repr.count.unwrap_or_default(),
            components: repr.components.unwrap_or_default(),
        };
        stack.validate().map_err(D::Error::custom)?;
        Ok(Some(stack))
    }
}

/// A field that is present reads as itself, `null` included.
fn present<'de, D: Deserializer<'de>, T: Deserialize<'de>>(d: D) -> Result<Option<T>, D::Error> {
    T::deserialize(d).map(Some)
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
            .map(|value| Ok((value.kind(), hash_ops::hash(value)?)))
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
            hash_ops::hash(value).is_ok_and(|actual| actual == *expected)
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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SlotRepr {
    #[serde(rename = "Slot", default, deserialize_with = "codec::int_value")]
    slot: i32,
    id: ResourceKey<ItemReg>,
    #[serde(default)]
    count: codec::Bounded<1, 99, 1>,
    #[serde(default)]
    components: ComponentPatch,
}

impl<'de> Deserialize<'de> for ItemStackWithSlot {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let repr: SlotRepr = map_only(d)?;
        let stack = ItemStackValue {
            item: repr.id,
            count: repr.count,
            components: repr.components,
        };
        stack.validate().map_err(D::Error::custom)?;
        Ok(ItemStackWithSlot {
            slot: (repr.slot & 0xFF) as u8,
            stack,
        })
    }
}
