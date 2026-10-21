use std::fmt;

use anyhow::ensure;
use mcrs_minecraft_core::Id;
use mcrs_minecraft_core::codec::{self, Validate, is_default};
use mcrs_minecraft_registries::{Key, Named, Namer, Refs, Resolve, Resolved};
use serde::de::{Error as _, MapAccess, Visitor, value};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::component::common::map_only;
use crate::hash_ops;
use crate::keys::DataComponentType;
use crate::keys::Item;
use crate::patch::ComponentPatch;

/// Before prototype normalisation: the patch is re-emitted as read.
#[derive(Clone, Debug, PartialEq, Resolve)]
pub struct ItemStackValue<M: Refs = Resolved> {
    pub item: M::Id<Item>,
    pub count: codec::Bounded<1, 99, 1>,
    pub components: ComponentPatch<M>,
}

pub trait ItemRef {
    fn is_air(&self) -> bool;
}

impl ItemRef for Key<Item> {
    fn is_air(&self) -> bool {
        self.location().as_str() == Item::Air.as_static_str()
    }
}

impl ItemRef for Id<Item> {
    fn is_air(&self) -> bool {
        *self == Item::Air.id()
    }
}

impl<M: Refs> Validate for ItemStackValue<M>
where
    M::Id<Item>: ItemRef,
{
    fn validate(&self) -> Result<(), String> {
        if self.item.is_air() {
            return Err("Item must not be minecraft:air".into());
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StackRepr {
    id: Key<Item>,
    #[serde(default)]
    count: codec::Bounded<1, 99, 1>,
    #[serde(default, skip_serializing_if = "ComponentPatch::is_empty")]
    components: ComponentPatch<Named>,
}

impl Serialize for ItemStackValue<Named> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        StackRepr {
            id: self.item.clone(),
            count: self.count,
            components: self.components.clone(),
        }
        .serialize(s)
    }
}

impl<'de> Deserialize<'de> for ItemStackValue<Named> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let repr = StackRepr::deserialize(d)?;
        let stack = ItemStackValue {
            item: repr.id,
            count: repr.count,
            components: repr.components,
        };
        stack.validate().map_err(D::Error::custom)?;
        Ok(stack)
    }
}

/// A stack written as a map, or as the bare item id when it is one plain item.
#[derive(Clone, Debug, PartialEq, Resolve)]
pub struct Template<M: Refs = Resolved>(pub ItemStackValue<M>);

impl<M: Refs> mcrs_minecraft_text::TextTypes for Template<M> {
    type EntityKey = M::Id<mcrs_minecraft_entity::keys::EntityType>;
    type Dialog = crate::dialog::DialogRef<M>;
}

impl mcrs_minecraft_text::TextSerde for Template<Named> {}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TemplateRepr {
    id: Key<Item>,
    #[serde(default, skip_serializing_if = "is_default")]
    count: codec::Bounded<1, 99, 1>,
    #[serde(default, skip_serializing_if = "ComponentPatch::is_empty")]
    components: ComponentPatch<Named>,
}

impl Serialize for Template<Named> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        TemplateRepr {
            id: self.0.item.clone(),
            count: self.0.count,
            components: self.0.components.clone(),
        }
        .serialize(s)
    }
}

impl<'de> Deserialize<'de> for Template<Named> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct TemplateVisitor;

        impl<'de> Visitor<'de> for TemplateVisitor {
            type Value = Template<Named>;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("an item id or an item stack")
            }

            fn visit_str<E: serde::de::Error>(self, id: &str) -> Result<Template<Named>, E> {
                let item = Key::deserialize(value::StrDeserializer::<E>::new(id))?;
                Template::new(item, 1, ComponentPatch::EMPTY).map_err(E::custom)
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Template<Named>, A::Error> {
                let repr = TemplateRepr::deserialize(value::MapAccessDeserializer::new(map))?;
                Template::new(repr.id, repr.count.0, repr.components).map_err(A::Error::custom)
            }
        }

        d.deserialize_any(TemplateVisitor)
    }
}

impl<M: Refs> Template<M>
where
    M::Id<Item>: ItemRef,
{
    pub fn new(
        item: M::Id<Item>,
        count: i32,
        components: ComponentPatch<M>,
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
    pub added: Vec<(DataComponentType, i32)>,
    pub removed: Vec<DataComponentType>,
}

impl HashedPatchMap {
    pub fn create(patch: &ComponentPatch, names: &impl Namer) -> anyhow::Result<Self> {
        let added = patch
            .added
            .iter()
            .map(|value| Ok((value.kind(), hash_ops::hash(&value.name(names)?)?)))
            .collect::<anyhow::Result<_>>()?;
        Ok(HashedPatchMap {
            added,
            removed: patch.removed.clone(),
        })
    }

    pub fn matches(&self, patch: &ComponentPatch, names: &impl Namer) -> bool {
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
            value
                .name(names)
                .ok()
                .and_then(|value| hash_ops::hash(&value).ok())
                .is_some_and(|actual| actual == *expected)
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProtoStack {
    pub id: Id<Item>,
    pub count: i32,
    pub components: ComponentPatch,
}

impl Default for ProtoStack {
    fn default() -> Self {
        Self::EMPTY
    }
}

impl ProtoStack {
    pub const EMPTY: ProtoStack = ProtoStack {
        id: Item::Air.id(),
        count: 0,
        components: ComponentPatch::EMPTY,
    };

    #[must_use]
    pub const fn new(item: Id<Item>, count: i32, components: ComponentPatch) -> Self {
        Self {
            id: item,
            count,
            components,
        }
    }

    /// No items, or the air item.
    pub const fn is_empty(&self) -> bool {
        self.count <= 0 || self.id.number() == 0
    }

    pub fn from_value(value: &ItemStackValue) -> Self {
        ProtoStack {
            id: value.item,
            count: value.count.0,
            components: value.components.clone(),
        }
    }

    pub fn to_value(&self) -> anyhow::Result<ItemStackValue> {
        ensure!(!self.is_empty(), "an empty stack has no persistent form");
        let value = ItemStackValue {
            item: self.id,
            count: codec::Bounded(self.count),
            components: self.components.clone(),
        };
        ensure!(
            (1..=99).contains(&self.count),
            "Value must be within range [1;99]: {}",
            self.count
        );
        value.validate().map_err(anyhow::Error::msg)?;
        Ok(value)
    }
}

/// A stack in a slotted list: the player inventory, the ender chest and a
/// chest's `Items`.
#[derive(Clone, Debug, PartialEq, Resolve)]
pub struct ItemStackWithSlot<M: Refs = Resolved> {
    pub slot: u8,
    pub stack: ItemStackValue<M>,
}

impl Serialize for ItemStackWithSlot<Named> {
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
    id: Key<Item>,
    #[serde(default)]
    count: codec::Bounded<1, 99, 1>,
    #[serde(default)]
    components: ComponentPatch<Named>,
}

impl<'de> Deserialize<'de> for ItemStackWithSlot<Named> {
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
