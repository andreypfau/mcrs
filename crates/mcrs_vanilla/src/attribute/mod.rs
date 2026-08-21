pub mod modifier;
pub mod registry;

use std::collections::BTreeMap;
use std::sync::Arc;

use serde::de::{DeserializeOwned, IntoDeserializer};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

use crate::ResourceLocation;

pub use modifier::{ModifierError, Operation, apply};
pub use registry::{
    AttributeError, AttributeRange, AttributeSpec, AttributeType, AttributeValue,
    ENVIRONMENT_ATTRIBUTES, attribute, is_syncable,
};

/// `EnvironmentAttributeMap`: attribute id to a modifier applied to that attribute.
///
/// The raw JSON of each argument is kept verbatim so the map serializes back
/// one-to-one; the typed value is derived through the registry on demand and
/// never cached here.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EnvironmentAttributeMap(pub BTreeMap<ResourceLocation<Arc<str>>, AttributeEntry>);

impl EnvironmentAttributeMap {
    pub fn get(&self, id: &str) -> Option<&AttributeEntry> {
        self.0.get(id)
    }

    pub fn argument<T: DeserializeOwned>(&self, id: &str) -> serde_json::Result<Option<T>> {
        match self.get(id) {
            Some(entry) => serde_json::from_value(entry.argument.clone()).map(Some),
            None => Ok(None),
        }
    }

    pub fn filter_syncable(&self) -> Self {
        EnvironmentAttributeMap(
            self.0
                .iter()
                .filter(|(id, _)| is_syncable(id.as_str()))
                .map(|(id, entry)| (id.clone(), entry.clone()))
                .collect(),
        )
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// One entry of an [`EnvironmentAttributeMap`], mirroring `EnvironmentAttributeMap.Entry`.
///
/// The JSON has two shapes: a bare value, which implies the `override`
/// modifier, or `{"argument": …, "modifier": …}` naming the modifier
/// explicitly. Which one it is comes from the attribute's registered type, not
/// from the shape of the JSON.
#[derive(Debug, Clone, PartialEq)]
pub struct AttributeEntry {
    pub argument: Value,
    pub modifier: Operation,
}

impl AttributeEntry {
    pub fn override_value(argument: Value) -> Self {
        AttributeEntry { argument, modifier: Operation::Override }
    }

    /// The argument as a typed value, parsed on demand through the registry.
    ///
    /// Not materialized: the argument is already a `serde_json::Value`, so the
    /// `visual/*` types a frame walks cost an enum match and at worst a
    /// `from_str_radix` over eight hex digits, with no allocation. The types
    /// that do allocate here — `ambient_particles`, `natural_mob_spawns`, the
    /// opaque payloads — are not on the per-frame path. If one lands there,
    /// measure before storing a parsed form beside the raw JSON.
    pub fn value(&self, spec: &AttributeSpec) -> Result<AttributeValue, AttributeError> {
        spec.parse_argument(self.modifier, &self.argument)
    }

    /// Split the JSON into (argument, modifier) the way
    /// `Codec.either(attribute.valueCodec(), fullCodec)` does: the attribute's
    /// own value codec is tried first, so a value that is itself an object can
    /// never be mistaken for the `{argument, modifier}` shape.
    pub fn parse(spec: &AttributeSpec, value: Value) -> Result<Self, AttributeError> {
        if spec.ty.matches_value(&value) {
            spec.parse_value(&value)?;
            return Ok(AttributeEntry::override_value(value));
        }

        let Value::Object(mut fields) = value else {
            return Err(registry::malformed(
                spec.id,
                format!("{value} is neither a {:?} value nor a modifier entry", spec.ty),
            ));
        };
        let modifier = match fields.remove("modifier") {
            Some(Value::String(name)) => Operation::deserialize(name.as_str().into_deserializer())
                .map_err(|_: serde::de::value::Error| {
                    registry::malformed(spec.id, format!("`{name}` is not a modifier operation"))
                })?,
            Some(other) => {
                return Err(registry::malformed(
                    spec.id,
                    format!("`modifier` must be a string, got {other}"),
                ));
            }
            None => return Err(registry::malformed(spec.id, "entry is missing `modifier`")),
        };
        let argument = fields
            .remove("argument")
            .ok_or_else(|| registry::malformed(spec.id, "entry is missing `argument`"))?;
        if !fields.is_empty() {
            let unexpected: Vec<_> = fields.keys().cloned().collect();
            return Err(registry::malformed(
                spec.id,
                format!("entry has unexpected fields {unexpected:?}"),
            ));
        }
        spec.parse_argument(modifier, &argument)?;
        Ok(AttributeEntry { argument, modifier })
    }
}

impl Serialize for AttributeEntry {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.modifier {
            Operation::Override => self.argument.serialize(serializer),
            modifier => {
                let mut map = serializer.serialize_map(Some(2))?;
                map.serialize_entry("argument", &self.argument)?;
                map.serialize_entry("modifier", &modifier)?;
                map.end()
            }
        }
    }
}

impl Serialize for EnvironmentAttributeMap {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for EnvironmentAttributeMap {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error;

        let raw = BTreeMap::<ResourceLocation<Arc<str>>, Value>::deserialize(deserializer)?;
        raw.into_iter()
            .map(|(id, value)| {
                let spec = attribute(id.as_str()).ok_or_else(|| {
                    D::Error::custom(AttributeError::UnknownAttribute(id.as_str().to_owned()))
                })?;
                let entry = AttributeEntry::parse(spec, value).map_err(D::Error::custom)?;
                Ok((id, entry))
            })
            .collect::<Result<_, D::Error>>()
            .map(EnvironmentAttributeMap)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_both_entry_shapes() {
        let json = serde_json::json!({
            "minecraft:visual/sky_color": "#78a7ff",
            "minecraft:visual/water_fog_end_distance": {"argument": 0.85, "modifier": "multiply"},
            "minecraft:audio/background_music": {"default": {"sound": "minecraft:music.game"}},
            "minecraft:gameplay/increased_fire_burnout": true,
        });
        let map: EnvironmentAttributeMap = serde_json::from_value(json.clone()).unwrap();

        assert_eq!(
            map.get("minecraft:visual/sky_color").unwrap().modifier,
            Operation::Override
        );
        assert_eq!(
            map.get("minecraft:visual/water_fog_end_distance").unwrap().modifier,
            Operation::Multiply
        );
        // a value that is itself an object must not be mistaken for the full shape
        assert_eq!(
            map.get("minecraft:audio/background_music").unwrap().modifier,
            Operation::Override
        );

        assert_eq!(serde_json::to_value(&map).unwrap(), json);

        let syncable = map.filter_syncable();
        assert!(syncable.get("minecraft:gameplay/increased_fire_burnout").is_none());
        assert_eq!(syncable.0.len(), 3);
    }

    #[test]
    fn an_object_valued_attribute_beats_the_entry_shape() {
        // `background_music` has no modifier but `override`, so even a payload
        // whose keys look exactly like the entry shape is read as the value.
        let json = serde_json::json!({
            "minecraft:audio/background_music": {"argument": {}, "modifier": "override"},
        });
        let map: EnvironmentAttributeMap = serde_json::from_value(json.clone()).unwrap();
        let entry = map.get("minecraft:audio/background_music").unwrap();
        assert_eq!(entry.modifier, Operation::Override);
        assert_eq!(entry.argument, serde_json::json!({"argument": {}, "modifier": "override"}));
        assert_eq!(serde_json::to_value(&map).unwrap(), json);
    }

    #[test]
    fn unknown_attributes_are_loud() {
        let err = serde_json::from_value::<EnvironmentAttributeMap>(serde_json::json!({
            "minecraft:visual/sky_colour": "#78a7ff",
        }))
        .unwrap_err();
        assert!(err.to_string().contains("unknown environment attribute"), "{err}");
    }

    #[test]
    fn invalid_modifiers_are_rejected() {
        let err = serde_json::from_value::<EnvironmentAttributeMap>(serde_json::json!({
            "minecraft:gameplay/piglins_zombify": {"argument": true, "modifier": "multiply"},
        }))
        .unwrap_err();
        assert!(err.to_string().contains("not a valid modifier"), "{err}");
    }

    #[test]
    fn typed_values_come_from_the_registry() {
        let map: EnvironmentAttributeMap = serde_json::from_value(serde_json::json!({
            "minecraft:visual/sky_color": "#78a7ff",
        }))
        .unwrap();
        let spec = attribute("minecraft:visual/sky_color").unwrap();
        assert_eq!(
            map.get("minecraft:visual/sky_color").unwrap().value(spec).unwrap(),
            AttributeValue::Color(0xFF78_A7FF)
        );
    }
}
