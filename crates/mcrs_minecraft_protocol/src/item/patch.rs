use std::fmt;
use std::io::Write;

use anyhow::ensure;
use mcrs_minecraft_registry::RegistryLookup;
use serde::de::{DeserializeSeed, Error as _, MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::item::component::deserialize_unit;
use crate::item::ctx::{DecodeCtx, EncodeCtx};
use crate::item::kind::{ItemComponentKind, ItemComponentValue, ItemDataComponent};
use crate::{Decode, Encode, VarInt};

/// `DataComponentPatch`: values set on top of an item's prototype and kinds
/// removed from it. A kind appears at most once across both lists.
#[derive(Clone, Debug, Default)]
pub struct ComponentPatch {
    pub added: Vec<ItemComponentValue>,
    pub removed: Vec<ItemComponentKind>,
}

/// Map equality, as `DataComponentPatch.equals`: order is not part of the
/// value.
impl PartialEq for ComponentPatch {
    fn eq(&self, other: &Self) -> bool {
        self.added.len() == other.added.len()
            && self.removed.len() == other.removed.len()
            && self
                .added
                .iter()
                .all(|value| other.get_value(value.kind()) == Some(value))
            && self.removed.iter().all(|kind| other.is_removed(*kind))
    }
}

impl ComponentPatch {
    pub const EMPTY: Self = Self {
        added: Vec::new(),
        removed: Vec::new(),
    };

    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty()
    }

    pub fn get<T: ItemDataComponent>(&self) -> Option<&T> {
        self.added.iter().find_map(T::from_value)
    }

    pub fn get_value(&self, kind: ItemComponentKind) -> Option<&ItemComponentValue> {
        self.added.iter().find(|value| value.kind() == kind)
    }

    pub fn is_removed(&self, kind: ItemComponentKind) -> bool {
        self.removed.contains(&kind)
    }

    pub fn set<T: ItemDataComponent>(&mut self, value: T) {
        self.set_value(value.into_value());
    }

    pub fn set_value(&mut self, value: ItemComponentValue) {
        let kind = value.kind();
        self.removed.retain(|removed| *removed != kind);
        match self.added.iter_mut().find(|added| added.kind() == kind) {
            Some(slot) => *slot = value,
            None => self.added.push(value),
        }
    }

    pub fn remove(&mut self, kind: ItemComponentKind) {
        self.added.retain(|added| added.kind() != kind);
        if !self.removed.contains(&kind) {
            self.removed.push(kind);
        }
    }

    fn encode_with(
        &self,
        ctx: &dyn RegistryLookup,
        mut w: impl Write,
        mut value: impl FnMut(
            &ItemComponentValue,
            &dyn RegistryLookup,
            &mut dyn Write,
        ) -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        VarInt(self.added.len() as i32).encode(&mut w)?;
        VarInt(self.removed.len() as i32).encode(&mut w)?;
        for added in &self.added {
            added.kind().encode(&mut w)?;
            value(added, ctx, &mut w)?;
        }
        for removed in &self.removed {
            removed.encode(&mut w)?;
        }
        Ok(())
    }

    fn decode_with<'a>(
        ctx: &dyn RegistryLookup,
        r: &mut &'a [u8],
        mut value: impl FnMut(
            ItemComponentKind,
            &dyn RegistryLookup,
            &mut &'a [u8],
        ) -> anyhow::Result<ItemComponentValue>,
    ) -> anyhow::Result<Self> {
        let added = VarInt::decode(r)?.0;
        let removed = VarInt::decode(r)?.0;
        ensure!(added >= 0 && removed >= 0, "negative component count");
        let mut patch = ComponentPatch::EMPTY;
        for _ in 0..added {
            let kind = ItemComponentKind::decode(r)?;
            patch.set_value(value(kind, ctx, r)?);
        }
        for _ in 0..removed {
            patch.remove(ItemComponentKind::decode(r)?);
        }
        Ok(patch)
    }

    /// `DELIMITED_STREAM_CODEC`: every value is preceded by its byte length.
    pub fn encode_delimited_ctx(
        &self,
        ctx: &dyn RegistryLookup,
        w: impl Write,
    ) -> anyhow::Result<()> {
        self.encode_with(ctx, w, |value, ctx, w| {
            let mut bytes = Vec::new();
            value.encode_ctx_value(ctx, &mut bytes)?;
            VarInt(bytes.len() as i32).encode(&mut *w)?;
            Ok(w.write_all(&bytes)?)
        })
    }

    /// The bytes a value leaves unread inside its length prefix are skipped,
    /// as `lengthPrefixed` advances by the declared size unconditionally.
    pub fn decode_delimited_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        Self::decode_with(ctx, r, |kind, ctx, r| {
            let len = VarInt::decode(r)?.0;
            ensure!(len >= 0, "negative component length");
            let len = len as usize;
            ensure!(
                len <= r.len(),
                "component {kind} declares {len} bytes but {} remain",
                r.len()
            );
            let (mut slice, rest) = r.split_at(len);
            let value = ItemComponentValue::decode_ctx_value(kind, ctx, &mut slice)?;
            *r = rest;
            Ok(value)
        })
    }
}

impl EncodeCtx for ComponentPatch {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, w: impl Write) -> anyhow::Result<()> {
        self.encode_with(ctx, w, |value, ctx, w| value.encode_ctx_value(ctx, w))
    }
}

impl<'a> DecodeCtx<'a> for ComponentPatch {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &'a [u8]) -> anyhow::Result<Self> {
        Self::decode_with(ctx, r, ItemComponentValue::decode_ctx_value)
    }
}

/// The persistent form of one value, `serialize_value` as a `Serialize`.
pub struct PersistentValue<'a>(pub &'a ItemComponentValue);

impl Serialize for PersistentValue<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.0.serialize_value(s)
    }
}

struct ValueSeed(ItemComponentKind);

impl<'de> DeserializeSeed<'de> for ValueSeed {
    type Value = ItemComponentValue;

    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
        ItemComponentValue::deserialize_value(self.0, d)
    }
}

struct EmptyMapSeed;

impl<'de> DeserializeSeed<'de> for EmptyMapSeed {
    type Value = ();

    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        deserialize_unit(d)
    }
}

fn persistent_kind<E: serde::de::Error>(id: &str) -> Result<ItemComponentKind, E> {
    let kind = ItemComponentKind::from_id(id)
        .ok_or_else(|| E::custom(ItemComponentKind::unknown_id_error(id)))?;
    if !kind.is_persistent() {
        return Err(E::custom(format_args!(
            "'{}' is not a persistent component",
            kind.id()
        )));
    }
    Ok(kind)
}

impl Serialize for ComponentPatch {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(None)?;
        for added in &self.added {
            if added.kind().is_persistent() {
                map.serialize_entry(added.kind().id().as_str(), &PersistentValue(added))?;
            }
        }
        for removed in &self.removed {
            if removed.is_persistent() {
                map.serialize_entry(&format!("!{}", removed.id()), &EmptyMap)?;
            }
        }
        map.end()
    }
}

struct EmptyMap;

impl Serialize for EmptyMap {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_map(Some(0))?.end()
    }
}

impl<'de> Deserialize<'de> for ComponentPatch {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct PatchVisitor;

        impl<'de> Visitor<'de> for PatchVisitor {
            type Value = ComponentPatch;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a map of component ids to values")
            }

            /// `"x"` and `"!x"` are distinct keys to the dispatched map, and the
            /// later one wins when the map collapses to a patch.
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut patch = ComponentPatch::EMPTY;
                let mut seen = Vec::new();
                while let Some(key) = map.next_key::<String>()? {
                    let (removed, id) = match key.strip_prefix('!') {
                        Some(id) => (true, id),
                        None => (false, key.as_str()),
                    };
                    let kind = persistent_kind(id)?;
                    if seen.contains(&(kind, removed)) {
                        return Err(A::Error::custom(format_args!("Duplicate key '{key}'")));
                    }
                    seen.push((kind, removed));
                    if removed {
                        map.next_value_seed(EmptyMapSeed)?;
                        patch.remove(kind);
                    } else {
                        patch.set_value(map.next_value_seed(ValueSeed(kind))?);
                    }
                }
                Ok(patch)
            }
        }

        d.deserialize_map(PatchVisitor)
    }
}

/// `DataComponentMap`: one value per kind, an item's prototype.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct ComponentMap(pub Vec<ItemComponentValue>);

impl ComponentMap {
    pub fn get<T: ItemDataComponent>(&self) -> Option<&T> {
        self.0.iter().find_map(T::from_value)
    }

    pub fn get_value(&self, kind: ItemComponentKind) -> Option<&ItemComponentValue> {
        self.0.iter().find(|value| value.kind() == kind)
    }

    pub fn set_value(&mut self, value: ItemComponentValue) {
        match self.0.iter_mut().find(|v| v.kind() == value.kind()) {
            Some(slot) => *slot = value,
            None => self.0.push(value),
        }
    }

    /// The map with `patch` applied on top of this prototype.
    pub fn apply(&self, patch: &ComponentPatch) -> ComponentMap {
        let mut map = self.clone();
        for added in &patch.added {
            map.set_value(added.clone());
        }
        map.0.retain(|value| !patch.removed.contains(&value.kind()));
        map
    }

    /// The patch that turns this prototype into `other`, normalised as
    /// `PatchedDataComponentMap` keeps it: no addition equal to the prototype
    /// value, no removal of a kind the prototype lacks.
    pub fn diff(&self, other: &ComponentMap) -> ComponentPatch {
        let mut patch = ComponentPatch::EMPTY;
        for value in &other.0 {
            if self.get_value(value.kind()) != Some(value) {
                patch.added.push(value.clone());
            }
        }
        for value in &self.0 {
            if other.get_value(value.kind()).is_none() {
                patch.removed.push(value.kind());
            }
        }
        patch
    }
}

impl Serialize for ComponentMap {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(None)?;
        for value in &self.0 {
            if value.kind().is_persistent() {
                map.serialize_entry(value.kind().id().as_str(), &PersistentValue(value))?;
            }
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for ComponentMap {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct MapVisitor;

        impl<'de> Visitor<'de> for MapVisitor {
            type Value = ComponentMap;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a map of component ids to values")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut values = ComponentMap::default();
                while let Some(key) = map.next_key::<String>()? {
                    let kind = persistent_kind(&key)?;
                    if values.get_value(kind).is_some() {
                        return Err(A::Error::custom(format_args!("Duplicate key '{key}'")));
                    }
                    values.0.push(map.next_value_seed(ValueSeed(kind))?);
                }
                Ok(values)
            }
        }

        d.deserialize_map(MapVisitor)
    }
}
