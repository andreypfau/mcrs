use std::fmt;

use serde::de::{DeserializeSeed, Error as _, MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::component::deserialize_unit;
use crate::kind::{ItemComponentKind, ItemComponentValue, ItemDataComponent};

/// Values set on top of an item's prototype and kinds removed from it; a kind
/// appears at most once across both lists.
#[derive(Clone, Debug, Default)]
pub struct ComponentPatch {
    pub added: Vec<ItemComponentValue>,
    pub removed: Vec<ItemComponentKind>,
}

/// Order is not part of the value.
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
}

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

    pub fn apply(&self, patch: &ComponentPatch) -> ComponentMap {
        let mut map = self.clone();
        for added in &patch.added {
            map.set_value(added.clone());
        }
        map.0.retain(|value| !patch.removed.contains(&value.kind()));
        map
    }

    /// The patch that turns this prototype into `other`, normalised: no addition
    /// equal to the prototype value, no removal of a kind the prototype lacks.
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
