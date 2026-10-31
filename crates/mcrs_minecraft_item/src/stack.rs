use mcrs_minecraft_resource::ResourceKey;
use std::fmt;

use anyhow::ensure;
use mcrs_minecraft_core::codec::{self, Validate, is_default};
use mcrs_minecraft_registries::{Named, Namer, Refs, Resolve, Resolved, WireId};
use mcrs_minecraft_resource::Id;
use serde::de::{Error as _, MapAccess, Visitor, value};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::component::common::map_only;
use crate::hash_ops;
use crate::keys::DataComponentType;
use crate::keys::Item;
use crate::patch::ComponentPatch;
use crate::patch::{decode_delimited_patch, encode_delimited_patch};
use mcrs_minecraft_registries::{FromWire, Wire, WireNames};
use mcrs_minecraft_wire::nested;
use mcrs_minecraft_wire::{Decode, Encode, Error, Result, VarInt};
use std::io::Write;

/// Before prototype normalisation: the patch is re-emitted as read.
#[derive(Clone, Debug, PartialEq, Resolve, mcrs_minecraft_registries::FromWire)]
pub struct ItemStackValue<M: Refs = Resolved> {
    pub item: M::Id<Item>,
    pub count: codec::Bounded<1, 99, 1>,
    pub components: ComponentPatch<M>,
}

pub trait ItemRef {
    fn is_air(&self) -> bool;
}

impl ItemRef for ResourceKey<Item> {
    fn is_air(&self) -> bool {
        self.location().as_str() == Item::Air.as_static_str()
    }
}

impl ItemRef for Id<Item> {
    fn is_air(&self) -> bool {
        *self == Item::Air.id()
    }
}

impl ItemRef for WireId<Item> {
    fn is_air(&self) -> bool {
        self.number() == Item::Air.id().number()
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
    id: ResourceKey<Item>,
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
#[derive(Clone, Debug, PartialEq, Resolve, mcrs_minecraft_registries::FromWire)]
pub struct Template<M: Refs = Resolved>(pub ItemStackValue<M>);

impl<M: Refs> mcrs_minecraft_text::TextTypes for Template<M> {
    type EntityKey = M::Id<mcrs_minecraft_entity::keys::EntityType>;
    type Dialog = crate::dialog::DialogRef<M>;
}

impl mcrs_minecraft_text::TextSerde for Template<Named> {}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TemplateRepr {
    id: ResourceKey<Item>,
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
                let item = ResourceKey::deserialize(value::StrDeserializer::<E>::new(id))?;
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

/// A stack as a slot holds it: a count of zero or the air item is the empty stack.
#[derive(Clone, Debug, PartialEq, mcrs_minecraft_registries::FromWire)]
pub struct ProtoStack<M: Refs = Resolved> {
    pub id: M::Id<Item>,
    pub count: i32,
    pub components: ComponentPatch<M>,
}

impl<M: Refs> Default for ProtoStack<M> {
    fn default() -> Self {
        ProtoStack {
            id: M::id_of(Item::Air),
            count: 0,
            components: ComponentPatch::default(),
        }
    }
}

impl<M: Refs> ProtoStack<M>
where
    M::Id<Item>: ItemRef,
{
    /// No items, or the air item.
    pub fn is_empty(&self) -> bool {
        self.count <= 0 || self.id.is_air()
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
    id: ResourceKey<Item>,
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

impl Encode for Template<Wire> {
    fn encode(&self, mut w: impl Write) -> Result<()> {
        self.0.item.encode(&mut w)?;
        VarInt(self.0.count.0).encode(&mut w)?;
        self.0.components.encode(w)
    }
}

impl Decode<'_> for Template<Wire> {
    fn decode(r: &mut &[u8]) -> Result<Self> {
        nested(|| {
            let item = WireId::<Item>::decode(r)?;
            let count = VarInt::decode(r)?.0;
            let components = ComponentPatch::decode(r)?;
            Template::new(item, count, components).map_err(Error::invalid)
        })
    }
}

fn encode_with(
    stack: &ProtoStack<Wire>,
    mut w: impl Write,
    patch: impl FnOnce(&ComponentPatch<Wire>, &mut dyn Write) -> Result<()>,
) -> Result<()> {
    if stack.is_empty() {
        return VarInt(0).encode(w);
    }
    VarInt(stack.count).encode(&mut w)?;
    stack.id.encode(&mut w)?;
    patch(&stack.components, &mut w)
}

fn decode_with<'a>(
    r: &mut &'a [u8],
    patch: impl FnOnce(&mut &'a [u8]) -> Result<ComponentPatch<Wire>>,
) -> Result<ProtoStack<Wire>> {
    let count = VarInt::decode(r)?.0;
    if count <= 0 {
        return Ok(ProtoStack::default());
    }
    let stack = ProtoStack {
        id: WireId::<Item>::decode(r)?,
        count,
        components: patch(r)?,
    };
    Ok(if stack.is_empty() {
        ProtoStack::default()
    } else {
        stack
    })
}

impl Encode for ProtoStack<Wire> {
    fn encode(&self, w: impl Write) -> Result<()> {
        encode_with(self, w, |patch, w| patch.encode(w))
    }
}

impl Decode<'_> for ProtoStack<Wire> {
    fn decode(r: &mut &[u8]) -> Result<Self> {
        decode_with(r, ComponentPatch::decode)
    }
}

/// A stack whose component values carry a length prefix.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct DelimitedStack(pub ProtoStack<Wire>);

impl DelimitedStack {
    /// Vanilla validates by re-encoding through the persistent codec; here every
    /// codec's checks sit on the read side, so the stack is read back from its
    /// persistent form instead.
    pub fn check(self, lookup: &impl WireNames) -> anyhow::Result<ProtoStack> {
        let stack = ProtoStack::from_wire(self.0, lookup)?;
        if !stack.is_empty() {
            let value = stack.to_value()?.name(lookup)?;
            let persistent = mcrs_minecraft_nbt::to_nbt_compound(&value)?;
            mcrs_minecraft_nbt::from_tag::<ItemStackValue<Named>>(persistent.into())?;
        }
        Ok(stack)
    }
}

impl Encode for DelimitedStack {
    fn encode(&self, w: impl Write) -> Result<()> {
        encode_with(&self.0, w, |patch, w| encode_delimited_patch(patch, w))
    }
}

impl Decode<'_> for DelimitedStack {
    fn decode(r: &mut &[u8]) -> Result<Self> {
        decode_with(r, decode_delimited_patch).map(DelimitedStack)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HashedStack {
    pub id: Id<Item>,
    pub count: i32,
    pub components: HashedPatchMap,
}

impl HashedStack {
    pub fn create(stack: &ProtoStack, names: &impl Namer) -> anyhow::Result<Option<Self>> {
        if stack.is_empty() {
            return Ok(None);
        }
        Ok(Some(HashedStack {
            id: stack.id,
            count: stack.count,
            components: HashedPatchMap::create(&stack.components, names)?,
        }))
    }

    pub fn matches(&self, stack: &ProtoStack, names: &impl Namer) -> bool {
        self.count == stack.count
            && self.id == stack.id
            && self.components.matches(&stack.components, names)
    }
}

impl Encode for HashedStack {
    fn encode(&self, mut w: impl Write) -> Result<()> {
        self.id.encode(&mut w)?;
        VarInt(self.count).encode(&mut w)?;
        self.components.encode(w)
    }
}

impl Decode<'_> for HashedStack {
    fn decode(r: &mut &[u8]) -> Result<Self> {
        Ok(HashedStack {
            id: Item::decode(r)?.id(),
            count: VarInt::decode(r)?.0,
            components: HashedPatchMap::decode(r)?,
        })
    }
}

impl Encode for HashedPatchMap {
    fn encode(&self, mut w: impl Write) -> Result<()> {
        mcrs_minecraft_wire::ensure!(
            self.added.len() <= MAX_HASHED_COMPONENTS
                && self.removed.len() <= MAX_HASHED_COMPONENTS,
            "more than {MAX_HASHED_COMPONENTS} hashed components"
        );
        VarInt(self.added.len() as i32).encode(&mut w)?;
        for (kind, hash) in &self.added {
            kind.encode(&mut w)?;
            hash.encode(&mut w)?;
        }
        VarInt(self.removed.len() as i32).encode(&mut w)?;
        for kind in &self.removed {
            kind.encode(&mut w)?;
        }
        Ok(())
    }
}

impl Decode<'_> for HashedPatchMap {
    fn decode(r: &mut &[u8]) -> Result<Self> {
        let count = |r: &mut &[u8]| -> Result<usize> {
            let n = VarInt::decode(r)?.0;
            mcrs_minecraft_wire::ensure!(
                (0..=MAX_HASHED_COMPONENTS as i32).contains(&n),
                "hashed component count {n} is outside 0..={MAX_HASHED_COMPONENTS}"
            );
            Ok(n as usize)
        };
        let added = count(r)?;
        let mut map = HashedPatchMap::default();
        for _ in 0..added {
            let kind = DataComponentType::decode(r)?;
            let hash = i32::decode(r)?;
            match map.added.iter_mut().find(|(k, _)| *k == kind) {
                Some(entry) => entry.1 = hash,
                None => map.added.push((kind, hash)),
            }
        }
        let removed = count(r)?;
        for _ in 0..removed {
            let kind = DataComponentType::decode(r)?;
            if !map.removed.contains(&kind) {
                map.removed.push(kind);
            }
        }
        Ok(map)
    }
}
