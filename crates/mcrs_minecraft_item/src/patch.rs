use std::collections::BTreeMap;
use std::fmt;

use mcrs_minecraft_registries::{
    FromWire, Named, Namer, Refs, Resolve, ResolveError, Resolved, Resolver, Wire, WireNames,
};
use serde::de::{DeserializeSeed, Error as _, MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::component::deserialize_unit;
use crate::keys::DataComponentType;
use crate::kind::decode_component_value;
use crate::kind::{ItemComponentValue, ItemDataComponent};
use mcrs_minecraft_wire::{Decode, Encode, Result, VarInt, ensure};
use std::io::Write;

/// Values set on top of an item's prototype and kinds removed from it; a kind
/// appears at most once across both lists.
#[derive(Clone, Debug)]
pub struct ComponentPatch<M: Refs = Resolved> {
    pub added: Vec<ItemComponentValue<M>>,
    pub removed: Vec<DataComponentType>,
}

impl<M: Refs> Default for ComponentPatch<M> {
    fn default() -> Self {
        Self::EMPTY
    }
}

/// Order is not part of the value.
impl<M: Refs> PartialEq for ComponentPatch<M> {
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

impl<M: Refs> ComponentPatch<M> {
    pub const EMPTY: Self = Self {
        added: Vec::new(),
        removed: Vec::new(),
    };

    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty()
    }

    pub fn get_value(&self, kind: DataComponentType) -> Option<&ItemComponentValue<M>> {
        self.added.iter().find(|value| value.kind() == kind)
    }

    pub fn is_removed(&self, kind: DataComponentType) -> bool {
        self.removed.contains(&kind)
    }

    pub fn set_value(&mut self, value: ItemComponentValue<M>) {
        let kind = value.kind();
        self.removed.retain(|removed| *removed != kind);
        match self.added.iter_mut().find(|added| added.kind() == kind) {
            Some(slot) => *slot = value,
            None => self.added.push(value),
        }
    }

    pub fn remove(&mut self, kind: DataComponentType) {
        self.added.retain(|added| added.kind() != kind);
        if !self.removed.contains(&kind) {
            self.removed.push(kind);
        }
    }
}

impl ComponentPatch {
    pub fn get<T: ItemDataComponent>(&self) -> Option<&T> {
        self.added.iter().find_map(T::from_value)
    }

    pub fn set<T: ItemDataComponent>(&mut self, value: T) {
        self.set_value(value.into());
    }
}

impl Resolve for ComponentPatch {
    type Named = ComponentPatch<Named>;

    fn resolve<C: Resolver>(named: ComponentPatch<Named>, ctx: &C) -> Result<Self, ResolveError> {
        Ok(ComponentPatch {
            added: named
                .added
                .into_iter()
                .map(|value| ItemComponentValue::resolve(value, ctx))
                .collect::<Result<_, _>>()?,
            removed: named.removed,
        })
    }

    fn name<C: Namer>(&self, names: &C) -> Result<ComponentPatch<Named>, ResolveError> {
        Ok(ComponentPatch {
            added: self
                .added
                .iter()
                .map(|value| value.name(names))
                .collect::<Result<_, _>>()?,
            removed: self.removed.clone(),
        })
    }
}

impl FromWire for ComponentPatch {
    type Wire = ComponentPatch<Wire>;

    fn from_wire<C: WireNames>(wire: ComponentPatch<Wire>, ctx: &C) -> Result<Self, ResolveError> {
        Ok(ComponentPatch {
            added: values_from_wire(wire.added, ctx)?,
            removed: wire.removed,
        })
    }

    fn to_wire<C: WireNames>(&self, ctx: &C) -> Result<ComponentPatch<Wire>, ResolveError> {
        Ok(ComponentPatch {
            added: values_to_wire(&self.added, ctx)?,
            removed: self.removed.clone(),
        })
    }
}

/// Unlike `Vec::from_wire`, an error carries no index, as the `Resolve` impls of a patch do.
fn values_from_wire<C: WireNames>(
    wire: Vec<ItemComponentValue<Wire>>,
    ctx: &C,
) -> Result<Vec<ItemComponentValue>, ResolveError> {
    wire.into_iter()
        .map(|value| ItemComponentValue::from_wire(value, ctx))
        .collect()
}

fn values_to_wire<C: WireNames>(
    values: &[ItemComponentValue],
    ctx: &C,
) -> Result<Vec<ItemComponentValue<Wire>>, ResolveError> {
    values.iter().map(|value| value.to_wire(ctx)).collect()
}

struct EmptyMapSeed;

impl<'de> DeserializeSeed<'de> for EmptyMapSeed {
    type Value = ();

    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        deserialize_unit(d)
    }
}

fn persistent_kind<E: serde::de::Error>(id: &str) -> Result<DataComponentType, E> {
    let kind = DataComponentType::read(id).ok_or_else(|| E::custom(unknown_component_error(id)))?;
    if !kind.is_persistent() {
        return Err(E::custom(format_args!(
            "'{}' is not a persistent component",
            kind.location()
        )));
    }
    Ok(kind)
}

impl Serialize for ComponentPatch<Named> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(None)?;
        for added in &self.added {
            if added.kind().is_persistent() {
                map.serialize_entry(added.kind().as_static_str(), added)?;
            }
        }
        for removed in &self.removed {
            if removed.is_persistent() {
                map.serialize_entry(
                    &format!("!{}", removed.location()),
                    &BTreeMap::<&str, ()>::new(),
                )?;
            }
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for ComponentPatch<Named> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct PatchVisitor;

        impl<'de> Visitor<'de> for PatchVisitor {
            type Value = ComponentPatch<Named>;

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
                        patch.set_value(map.next_value_seed(kind)?);
                    }
                }
                Ok(patch)
            }
        }

        d.deserialize_map(PatchVisitor)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ComponentMap<M: Refs = Resolved>(pub Vec<ItemComponentValue<M>>);

impl<M: Refs> Default for ComponentMap<M> {
    fn default() -> Self {
        ComponentMap(Vec::new())
    }
}

impl ComponentMap {
    pub fn get<T: ItemDataComponent>(&self) -> Option<&T> {
        self.0.iter().find_map(T::from_value)
    }
}

impl Resolve for ComponentMap {
    type Named = ComponentMap<Named>;

    fn resolve<C: Resolver>(named: ComponentMap<Named>, ctx: &C) -> Result<Self, ResolveError> {
        named
            .0
            .into_iter()
            .map(|value| ItemComponentValue::resolve(value, ctx))
            .collect::<Result<_, _>>()
            .map(ComponentMap)
    }

    fn name<C: Namer>(&self, names: &C) -> Result<ComponentMap<Named>, ResolveError> {
        self.0
            .iter()
            .map(|value| value.name(names))
            .collect::<Result<_, _>>()
            .map(ComponentMap)
    }
}

impl FromWire for ComponentMap {
    type Wire = ComponentMap<Wire>;

    fn from_wire<C: WireNames>(wire: ComponentMap<Wire>, ctx: &C) -> Result<Self, ResolveError> {
        values_from_wire(wire.0, ctx).map(ComponentMap)
    }

    fn to_wire<C: WireNames>(&self, ctx: &C) -> Result<ComponentMap<Wire>, ResolveError> {
        values_to_wire(&self.0, ctx).map(ComponentMap)
    }
}

impl<M: Refs> ComponentMap<M> {
    pub fn get_value(&self, kind: DataComponentType) -> Option<&ItemComponentValue<M>> {
        self.0.iter().find(|value| value.kind() == kind)
    }

    pub fn set_value(&mut self, value: ItemComponentValue<M>) {
        match self.0.iter_mut().find(|v| v.kind() == value.kind()) {
            Some(slot) => *slot = value,
            None => self.0.push(value),
        }
    }

    pub fn apply(&self, patch: &ComponentPatch<M>) -> ComponentMap<M> {
        let mut map = self.clone();
        for added in &patch.added {
            map.set_value(added.clone());
        }
        map.0.retain(|value| !patch.removed.contains(&value.kind()));
        map
    }

    /// The patch that turns this prototype into `other`, normalised: no addition
    /// equal to the prototype value, no removal of a kind the prototype lacks.
    pub fn diff(&self, other: &ComponentMap<M>) -> ComponentPatch<M> {
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

impl Serialize for ComponentMap<Named> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(None)?;
        for value in &self.0 {
            if value.kind().is_persistent() {
                map.serialize_entry(value.kind().as_static_str(), value)?;
            }
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for ComponentMap<Named> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct MapVisitor;

        impl<'de> Visitor<'de> for MapVisitor {
            type Value = ComponentMap<Named>;

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
                    values.0.push(map.next_value_seed(kind)?);
                }
                Ok(values)
            }
        }

        d.deserialize_map(MapVisitor)
    }
}

/// Vanilla names the parsed identifier, so a bare path is reported with its
/// default namespace.
fn unknown_component_error(id: &str) -> String {
    let namespace = if id.contains(':') { "" } else { "minecraft:" };
    format!("No component with type: '{namespace}{id}'")
}

fn encode_with(
    patch: &ComponentPatch<Wire>,
    mut w: impl Write,
    mut value: impl FnMut(&ItemComponentValue<Wire>, &mut dyn Write) -> Result<()>,
) -> Result<()> {
    VarInt(patch.added.len() as i32).encode(&mut w)?;
    VarInt(patch.removed.len() as i32).encode(&mut w)?;
    for added in &patch.added {
        added.kind().encode(&mut w)?;
        value(added, &mut w)?;
    }
    for removed in &patch.removed {
        removed.encode(&mut w)?;
    }
    Ok(())
}

fn decode_with<'a>(
    r: &mut &'a [u8],
    mut value: impl FnMut(DataComponentType, &mut &'a [u8]) -> Result<ItemComponentValue<Wire>>,
) -> Result<ComponentPatch<Wire>> {
    let added = VarInt::decode(r)?.0;
    let removed = VarInt::decode(r)?.0;
    ensure!(added >= 0 && removed >= 0, "negative component count");
    let mut patch = ComponentPatch::default();
    for _ in 0..added {
        let kind = DataComponentType::decode(r)?;
        patch.set_value(value(kind, r)?);
    }
    for _ in 0..removed {
        patch.remove(DataComponentType::decode(r)?);
    }
    Ok(patch)
}

/// Every added value behind its byte length.
pub fn encode_delimited_patch(patch: &ComponentPatch<Wire>, w: impl Write) -> Result<()> {
    encode_with(patch, w, |value, w| {
        let mut bytes = Vec::new();
        value.encode(&mut bytes)?;
        VarInt(bytes.len() as i32).encode(&mut *w)?;
        Ok(w.write_all(&bytes)?)
    })
}

/// The bytes a value leaves unread inside its length prefix are skipped:
/// the declared size is advanced unconditionally.
pub fn decode_delimited_patch(r: &mut &[u8]) -> Result<ComponentPatch<Wire>> {
    decode_with(r, |kind, r| {
        let len = VarInt::decode(r)?.0;
        ensure!(len >= 0, "negative component length");
        let len = len as usize;
        ensure!(
            len <= r.len(),
            "component {} declares {len} bytes but {} remain",
            kind.location(),
            r.len()
        );
        let (mut slice, rest) = r.split_at(len);
        let value = decode_component_value(kind, &mut slice)?;
        *r = rest;
        Ok(value)
    })
}

impl Encode for ComponentPatch<Wire> {
    fn encode(&self, w: impl Write) -> Result<()> {
        encode_with(self, w, |value, w| value.encode(w))
    }
}

impl Decode<'_> for ComponentPatch<Wire> {
    fn decode(r: &mut &[u8]) -> Result<Self> {
        decode_with(r, decode_component_value)
    }
}

impl Encode for ComponentMap<Wire> {
    fn encode(&self, mut w: impl Write) -> Result<()> {
        let w: &mut dyn Write = &mut w;
        VarInt(self.0.len() as i32).encode(&mut *w)?;
        for value in &self.0 {
            value.kind().encode(&mut *w)?;
            value.encode(&mut *w)?;
        }
        Ok(())
    }
}

impl Decode<'_> for ComponentMap<Wire> {
    fn decode(r: &mut &[u8]) -> Result<Self> {
        let len = VarInt::decode(r)?.0;
        ensure!(len >= 0, "attempt to decode a list with negative length");
        let mut map = ComponentMap::default();
        for _ in 0..len {
            let kind = DataComponentType::decode(r)?;
            map.set_value(decode_component_value(kind, r)?);
        }
        Ok(map)
    }
}
