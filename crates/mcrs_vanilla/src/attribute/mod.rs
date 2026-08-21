use std::collections::BTreeMap;
use std::sync::Arc;

use serde::de::DeserializeOwned;
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

use crate::ResourceLocation;

/// Attribute ids the client is allowed to see that are not under `visual/` or
/// `audio/`; every attribute in those two groups is syncable.
const SYNCABLE_GAMEPLAY_ATTRIBUTES: &[&str] = &[
    "gameplay/sky_light_level",
    "gameplay/water_evaporates",
    "gameplay/fast_lava",
    "gameplay/piglins_zombify",
    "gameplay/creaking_active",
];

/// `EnvironmentAttributeMap`: attribute id to a modifier applied to that attribute.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(transparent)]
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
                .filter(|(id, _)| is_syncable(id))
                .map(|(id, entry)| (id.clone(), entry.clone()))
                .collect(),
        )
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

pub fn is_syncable(id: &ResourceLocation<Arc<str>>) -> bool {
    let path = id.path();
    path.starts_with("visual/")
        || path.starts_with("audio/")
        || SYNCABLE_GAMEPLAY_ATTRIBUTES.contains(&path)
}

/// One entry of an [`EnvironmentAttributeMap`], mirroring `EnvironmentAttributeMap.Entry`.
///
/// The JSON has two shapes: a bare value, which implies the `override` modifier,
/// or `{"argument": …, "modifier": …}` naming the modifier explicitly.
#[derive(Debug, Clone, PartialEq)]
pub struct AttributeEntry {
    pub argument: Value,
    /// `None` means `override`, the modifier the bare-value shape implies.
    pub modifier: Option<Arc<str>>,
}

impl AttributeEntry {
    pub fn override_value(argument: Value) -> Self {
        AttributeEntry { argument, modifier: None }
    }
}

impl Serialize for AttributeEntry {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match &self.modifier {
            None => self.argument.serialize(serializer),
            Some(modifier) => {
                let mut map = serializer.serialize_map(Some(2))?;
                map.serialize_entry("argument", &self.argument)?;
                map.serialize_entry("modifier", modifier.as_ref())?;
                map.end()
            }
        }
    }
}

impl<'de> Deserialize<'de> for AttributeEntry {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = Value::deserialize(deserializer)?;
        // ponytail: the reference picks the shape by trying the attribute's own value
        // codec first (Codec.either), which needs the attribute registry we don't have
        // yet; until then the two-key structural match separates them. Ceiling: an
        // attribute whose value is an object with exactly `argument` and `modifier`
        // would be misread. Upgrade with the EnvironmentAttributes registry.
        if let Value::Object(fields) = &value
            && fields.len() == 2
            && let Some(Value::String(modifier)) = fields.get("modifier")
            && let Some(argument) = fields.get("argument")
        {
            return Ok(AttributeEntry {
                argument: argument.clone(),
                modifier: Some(Arc::from(modifier.as_str())),
            });
        }
        Ok(AttributeEntry::override_value(value))
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

        assert_eq!(map.get("minecraft:visual/sky_color").unwrap().modifier, None);
        assert_eq!(
            map.get("minecraft:visual/water_fog_end_distance").unwrap().modifier.as_deref(),
            Some("multiply")
        );
        // a value that is itself an object must not be mistaken for the full shape
        assert_eq!(map.get("minecraft:audio/background_music").unwrap().modifier, None);

        assert_eq!(serde_json::to_value(&map).unwrap(), json);

        let syncable = map.filter_syncable();
        assert!(syncable.get("minecraft:gameplay/increased_fire_burnout").is_none());
        assert_eq!(syncable.0.len(), 3);
    }
}
