use std::fmt;
use std::io::Write;

use anyhow::{Context, ensure};
use bytes::Bytes;
use mcrs_minecraft_core::codec::{self, Validate, is_default};
use mcrs_minecraft_core::{ResourceKey, validated};
use mcrs_minecraft_registry::{ItemId, RegistryLookup};
use serde::de::{Error as _, MapAccess, Visitor, value};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::item::component::{ItemReg, RegistryName};
use crate::item::ctx::{DecodeCtx, EncodeCtx, Opaque, nested};
use crate::item::hash_ops;
use crate::item::kind::ItemComponentKind;
use crate::item::patch::{ComponentPatch, PersistentValue};
use crate::{Decode, Encode, VarInt};

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

impl EncodeCtx for Template {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        self.0.item.encode_ctx(ctx, &mut w)?;
        VarInt(self.0.count.0).encode(&mut w)?;
        self.0.components.encode_ctx(ctx, w)
    }
}

impl<'a> DecodeCtx<'a> for Template {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &'a [u8]) -> anyhow::Result<Self> {
        nested(|| {
            let item = ResourceKey::decode_ctx(ctx, r)?;
            let count = VarInt::decode(r)?.0;
            let components = ComponentPatch::decode_ctx(ctx, r)?;
            Template::new(item, count, components).map_err(anyhow::Error::msg)
        })
    }
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Slot {
    pub id: ItemId,
    pub count: i32,
    pub components: ComponentPatch,
}

impl Slot {
    pub const EMPTY: Slot = Slot {
        id: ItemId(0),
        count: 0,
        components: ComponentPatch::EMPTY,
    };

    #[must_use]
    pub const fn new(item: ItemId, count: i32, components: ComponentPatch) -> Self {
        Self {
            id: item,
            count,
            components,
        }
    }

    #[must_use]
    pub const fn with_count(mut self, count: i32) -> Self {
        self.count = count;
        self
    }

    #[must_use]
    pub const fn with_item(mut self, item: ItemId) -> Self {
        self.id = item;
        self
    }

    /// No items, or the air item.
    pub const fn is_empty(&self) -> bool {
        self.count <= 0 || self.id.0 == 0
    }

    pub fn from_value(value: &ItemStackValue, ctx: &dyn RegistryLookup) -> anyhow::Result<Self> {
        let id = ctx
            .id(ItemReg::NAME, value.item.location())
            .with_context(|| format!("{} is not in registry item", value.item))?;
        Ok(Slot {
            id: ItemId(u16::try_from(id).with_context(|| format!("item id {id} is out of range"))?),
            count: value.count.0,
            components: value.components.clone(),
        })
    }

    pub fn to_value(&self, ctx: &dyn RegistryLookup) -> anyhow::Result<ItemStackValue> {
        ensure!(!self.is_empty(), "an empty stack has no persistent form");
        let name = ctx
            .name(ItemReg::NAME, self.id.0 as u32)
            .with_context(|| format!("registry item has no id {}", self.id.0))?;
        let value = ItemStackValue {
            item: ResourceKey::from_location(name.clone()),
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

    fn encode_with(
        &self,
        ctx: &dyn RegistryLookup,
        mut w: impl Write,
        patch: impl FnOnce(&ComponentPatch, &dyn RegistryLookup, &mut dyn Write) -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        if self.is_empty() {
            return VarInt(0).encode(w);
        }
        VarInt(self.count).encode(&mut w)?;
        self.id.encode(&mut w)?;
        patch(&self.components, ctx, &mut w)
    }

    fn decode_with<'a>(
        ctx: &dyn RegistryLookup,
        r: &mut &'a [u8],
        patch: impl FnOnce(&dyn RegistryLookup, &mut &'a [u8]) -> anyhow::Result<ComponentPatch>,
    ) -> anyhow::Result<Self> {
        let count = VarInt::decode(r)?.0;
        if count <= 0 {
            return Ok(Slot::EMPTY);
        }
        let slot = Slot {
            id: ItemId::decode(r)?,
            count,
            components: patch(ctx, r)?,
        };
        Ok(if slot.is_empty() { Slot::EMPTY } else { slot })
    }

    pub fn encode_delimited_ctx(
        &self,
        ctx: &dyn RegistryLookup,
        w: impl Write,
    ) -> anyhow::Result<()> {
        self.encode_with(ctx, w, |patch, ctx, w| patch.encode_delimited_ctx(ctx, w))
    }

    pub fn decode_delimited_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        Self::decode_with(ctx, r, ComponentPatch::decode_delimited_ctx)
    }
}

impl EncodeCtx for Slot {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, w: impl Write) -> anyhow::Result<()> {
        self.encode_with(ctx, w, |patch, ctx, w| patch.encode_ctx(ctx, w))
    }
}

impl<'a> DecodeCtx<'a> for Slot {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &'a [u8]) -> anyhow::Result<Self> {
        Self::decode_with(ctx, r, ComponentPatch::decode_ctx)
    }
}

/// The exact bytes of one wire stack, kept so packets can
/// carry stacks without the registries; the value walks the layout to find
/// its length and is resolved on demand.
// ponytail: every stack is parsed twice, once to measure and once to resolve; fine at inventory
// sizes, replace with a macro-generated skip when it shows up in a profile.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RawStack(pub Bytes);

impl Default for RawStack {
    fn default() -> Self {
        RawStack::EMPTY
    }
}

impl RawStack {
    pub const EMPTY: RawStack = RawStack(Bytes::from_static(&[0]));

    pub fn resolve(&self, ctx: &dyn RegistryLookup) -> anyhow::Result<Slot> {
        let mut r = &self.0[..];
        let slot = Slot::decode_ctx(ctx, &mut r)?;
        ensure!(r.is_empty(), "{} trailing bytes after a stack", r.len());
        Ok(slot)
    }

    pub fn from_slot(slot: &Slot, ctx: &dyn RegistryLookup) -> anyhow::Result<RawStack> {
        let mut bytes = Vec::new();
        slot.encode_ctx(ctx, &mut bytes)?;
        Ok(RawStack(bytes.into()))
    }
}

impl Encode for RawStack {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        Ok(w.write_all(&self.0)?)
    }
}

impl Decode<'_> for RawStack {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        let start = *r;
        Slot::decode_ctx(&Opaque, r)?;
        Ok(RawStack(Bytes::copy_from_slice(
            &start[..start.len() - r.len()],
        )))
    }
}

/// A stack whose component values carry a length prefix, validated on resolve.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RawDelimitedStack(pub Bytes);

impl Default for RawDelimitedStack {
    fn default() -> Self {
        RawDelimitedStack::EMPTY
    }
}

impl RawDelimitedStack {
    pub const EMPTY: RawDelimitedStack = RawDelimitedStack(Bytes::from_static(&[0]));

    /// Vanilla validates by re-encoding through the persistent codec; here every
    /// codec's checks sit on the read side, so the stack is read back from its
    /// persistent form instead.
    pub fn resolve(&self, ctx: &dyn RegistryLookup) -> anyhow::Result<Slot> {
        let mut r = &self.0[..];
        let slot = Slot::decode_delimited_ctx(ctx, &mut r)?;
        ensure!(r.is_empty(), "{} trailing bytes after a stack", r.len());
        if !slot.is_empty() {
            let value = slot.to_value(ctx)?;
            let persistent = mcrs_minecraft_nbt::to_nbt_compound(&value)?;
            mcrs_minecraft_nbt::from_tag::<ItemStackValue>(persistent.into())?;
        }
        Ok(slot)
    }

    pub fn from_slot(slot: &Slot, ctx: &dyn RegistryLookup) -> anyhow::Result<RawDelimitedStack> {
        let mut bytes = Vec::new();
        slot.encode_delimited_ctx(ctx, &mut bytes)?;
        Ok(RawDelimitedStack(bytes.into()))
    }
}

impl Encode for RawDelimitedStack {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        Ok(w.write_all(&self.0)?)
    }
}

impl Decode<'_> for RawDelimitedStack {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        let start = *r;
        Slot::decode_delimited_ctx(&Opaque, r)?;
        Ok(RawDelimitedStack(Bytes::copy_from_slice(
            &start[..start.len() - r.len()],
        )))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct HashedSlot {
    pub id: ItemId,
    pub count: i32,
    pub components: HashedPatchMap,
}

impl HashedSlot {
    pub fn create(slot: &Slot) -> anyhow::Result<Option<Self>> {
        if slot.is_empty() {
            return Ok(None);
        }
        Ok(Some(HashedSlot {
            id: slot.id,
            count: slot.count,
            components: HashedPatchMap::create(&slot.components)?,
        }))
    }

    pub fn matches(&self, slot: &Slot) -> bool {
        self.count == slot.count && self.id == slot.id && self.components.matches(&slot.components)
    }
}

impl Encode for HashedSlot {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        self.id.encode(&mut w)?;
        VarInt(self.count).encode(&mut w)?;
        self.components.encode(w)
    }
}

impl Decode<'_> for HashedSlot {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(HashedSlot {
            id: ItemId::decode(r)?,
            count: VarInt::decode(r)?.0,
            components: HashedPatchMap::decode(r)?,
        })
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

impl Encode for HashedPatchMap {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        ensure!(
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
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        let count = |r: &mut &[u8]| -> anyhow::Result<usize> {
            let n = VarInt::decode(r)?.0;
            ensure!(
                (0..=MAX_HASHED_COMPONENTS as i32).contains(&n),
                "hashed component count {n} is outside 0..={MAX_HASHED_COMPONENTS}"
            );
            Ok(n as usize)
        };
        let added = count(r)?;
        let mut map = HashedPatchMap::default();
        for _ in 0..added {
            let kind = ItemComponentKind::decode(r)?;
            let hash = i32::decode(r)?;
            match map.added.iter_mut().find(|(k, _)| *k == kind) {
                Some(slot) => slot.1 = hash,
                None => map.added.push((kind, hash)),
            }
        }
        let removed = count(r)?;
        for _ in 0..removed {
            let kind = ItemComponentKind::decode(r)?;
            if !map.removed.contains(&kind) {
                map.removed.push(kind);
            }
        }
        Ok(map)
    }
}
