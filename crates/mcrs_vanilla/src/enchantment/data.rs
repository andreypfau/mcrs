use std::sync::Arc;

use mcrs_core::tag::key::{TagKey, TaggedRegistry};
use mcrs_core::ResourceLocation;
use serde::{ser::SerializeMap, ser::SerializeSeq, Deserialize, Serialize, Serializer};

use crate::item::Item;

/// Wrapper around a borrowed JSON value that serializes floats as f32
/// (mapping to NBT `Tag_Float`) when the source JSON used decimal notation.
/// Vanilla 26.1 codecs in `EnchantmentEffectComponents` and `LevelBasedValue`
/// expect `Tag_Float`, not the `Tag_Double` that a plain `serde_json::Value`
/// would produce.
struct EffectsValue<'a>(&'a serde_json::Value);

impl<'a> Serialize for EffectsValue<'a> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.0 {
            serde_json::Value::Null => serializer.serialize_unit(),
            serde_json::Value::Bool(b) => serializer.serialize_bool(*b),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    if (i32::MIN as i64..=i32::MAX as i64).contains(&i) {
                        serializer.serialize_i32(i as i32)
                    } else {
                        serializer.serialize_i64(i)
                    }
                } else if let Some(u) = n.as_u64() {
                    if u <= i32::MAX as u64 {
                        serializer.serialize_i32(u as i32)
                    } else {
                        serializer.serialize_i64(u as i64)
                    }
                } else if let Some(f) = n.as_f64() {
                    serializer.serialize_f32(f as f32)
                } else {
                    serializer.serialize_unit()
                }
            }
            serde_json::Value::String(s) => serializer.serialize_str(s),
            serde_json::Value::Array(arr) => {
                let mut seq = serializer.serialize_seq(Some(arr.len()))?;
                for v in arr {
                    seq.serialize_element(&EffectsValue(v))?;
                }
                seq.end()
            }
            serde_json::Value::Object(map) => {
                let mut m = serializer.serialize_map(Some(map.len()))?;
                for (k, v) in map {
                    m.serialize_entry(k, &EffectsValue(v))?;
                }
                m.end()
            }
        }
    }
}

struct EffectsField<'a>(&'a serde_json::Value);

impl<'a> Serialize for EffectsField<'a> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        EffectsValue(self.0).serialize(serializer)
    }
}

/// Raw enchantment data as deserialized from JSON.
///
/// Tag reference fields (`supported_items`, `primary_items`, `exclusive_set`)
/// are raw `"#namespace:path"` strings. Converted to [`EnchantmentData`] by
/// [`ProtoEnchantmentData::resolve`], which parses them into typed `TagKey`s.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ProtoEnchantmentData {
    pub description: serde_json::Value,
    pub min_cost: EnchantmentCost,
    pub max_cost: EnchantmentCost,
    pub anvil_cost: u32,
    pub slots: Vec<String>,
    pub supported_items: String,
    #[serde(default)]
    pub primary_items: Option<String>,
    pub weight: u32,
    pub max_level: u32,
    #[serde(default)]
    pub exclusive_set: Option<String>,
    #[serde(default)]
    pub effects: Option<serde_json::Value>,
}

#[derive(Debug, thiserror::Error)]
pub enum EnchantmentResolveError {
    #[error("tag reference `{0}` does not start with '#'")]
    MissingHashPrefix(String),
    #[error("invalid resource location in tag reference: {0}")]
    InvalidResourceLocation(#[from] mcrs_core::resource_location::ResourceLocationError),
}

fn parse_tag_key<T: TaggedRegistry>(
    raw: &str,
) -> Result<TagKey<T, Arc<str>>, EnchantmentResolveError> {
    let tag_str = raw
        .strip_prefix('#')
        .ok_or_else(|| EnchantmentResolveError::MissingHashPrefix(raw.to_string()))?;
    let rl: ResourceLocation<Arc<str>> = ResourceLocation::parse(tag_str)?;
    Ok(TagKey::from_location(rl))
}

impl ProtoEnchantmentData {
    pub fn resolve(self) -> Result<EnchantmentData, EnchantmentResolveError> {
        let supported_items = parse_tag_key::<Item>(&self.supported_items)?;
        let primary_items = self
            .primary_items
            .as_deref()
            .map(parse_tag_key::<Item>)
            .transpose()?;
        let exclusive_set = self
            .exclusive_set
            .as_deref()
            .map(parse_tag_key::<EnchantmentData>)
            .transpose()?;

        Ok(EnchantmentData {
            description: self.description,
            min_cost: self.min_cost,
            max_cost: self.max_cost,
            anvil_cost: self.anvil_cost,
            slots: self.slots,
            supported_items,
            primary_items,
            weight: self.weight,
            max_level: self.max_level,
            exclusive_set,
            effects: self.effects,
        })
    }
}

/// Runtime enchantment data with typed tag key references.
///
/// Tag reference fields hold parsed `TagKey` values instead of raw strings,
/// enabling type-safe lookups against `TagRegistry<Item>` and
/// `TagRegistry<EnchantmentData>`.
#[derive(Debug, Clone)]
pub struct EnchantmentData {
    pub description: serde_json::Value,
    pub min_cost: EnchantmentCost,
    pub max_cost: EnchantmentCost,
    pub anvil_cost: u32,
    pub slots: Vec<String>,
    pub supported_items: TagKey<Item, Arc<str>>,
    pub primary_items: Option<TagKey<Item, Arc<str>>>,
    pub weight: u32,
    pub max_level: u32,
    pub exclusive_set: Option<TagKey<EnchantmentData, Arc<str>>>,
    pub effects: Option<serde_json::Value>,
}

/// Enchantment data for NETWORK_CODEC — tag key fields serialized as
/// `"#namespace:path"` strings matching the original JSON format.
#[derive(Debug, Clone)]
pub struct NetworkEnchantmentData {
    pub description: serde_json::Value,
    pub min_cost: EnchantmentCost,
    pub max_cost: EnchantmentCost,
    pub anvil_cost: u32,
    pub slots: Vec<String>,
    pub supported_items: String,
    pub primary_items: Option<String>,
    pub weight: u32,
    pub max_level: u32,
    pub exclusive_set: Option<String>,
    pub effects: Option<serde_json::Value>,
}

impl Serialize for NetworkEnchantmentData {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut len = 9usize;
        if self.primary_items.is_some() {
            len += 1;
        }
        if self.exclusive_set.is_some() {
            len += 1;
        }
        if self.effects.is_some() {
            len += 1;
        }
        let mut m = serializer.serialize_map(Some(len))?;
        m.serialize_entry("description", &EffectsField(&self.description))?;
        m.serialize_entry("min_cost", &self.min_cost)?;
        m.serialize_entry("max_cost", &self.max_cost)?;
        m.serialize_entry("anvil_cost", &self.anvil_cost)?;
        m.serialize_entry("slots", &self.slots)?;
        m.serialize_entry("supported_items", &self.supported_items)?;
        if let Some(ref pi) = self.primary_items {
            m.serialize_entry("primary_items", pi)?;
        }
        m.serialize_entry("weight", &self.weight)?;
        m.serialize_entry("max_level", &self.max_level)?;
        if let Some(ref es) = self.exclusive_set {
            m.serialize_entry("exclusive_set", es)?;
        }
        if let Some(ref effects) = self.effects {
            m.serialize_entry("effects", &EffectsField(effects))?;
        }
        m.end()
    }
}

impl From<&EnchantmentData> for NetworkEnchantmentData {
    fn from(data: &EnchantmentData) -> Self {
        NetworkEnchantmentData {
            description: data.description.clone(),
            min_cost: data.min_cost.clone(),
            max_cost: data.max_cost.clone(),
            anvil_cost: data.anvil_cost,
            slots: data.slots.clone(),
            supported_items: format!("#{}", data.supported_items.as_str()),
            primary_items: data.primary_items.as_ref().map(|k| format!("#{}", k.as_str())),
            weight: data.weight,
            max_level: data.max_level,
            exclusive_set: data.exclusive_set.as_ref().map(|k| format!("#{}", k.as_str())),
            effects: data.effects.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnchantmentCost {
    pub base: u32,
    pub per_level_above_first: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn assets_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("assets")
    }

    #[test]
    fn deserialize_and_resolve_sharpness() {
        let bytes = std::fs::read(
            assets_dir().join("minecraft/enchantment/sharpness.json"),
        )
        .unwrap();
        let proto: ProtoEnchantmentData = serde_json::from_slice(&bytes).unwrap();

        assert_eq!(proto.supported_items, "#minecraft:enchantable/sharp_weapon");
        assert_eq!(
            proto.exclusive_set.as_deref(),
            Some("#minecraft:exclusive_set/damage")
        );
        assert_eq!(
            proto.primary_items.as_deref(),
            Some("#minecraft:enchantable/melee_weapon")
        );

        let data = proto.resolve().unwrap();
        assert_eq!(
            data.supported_items.as_str(),
            "minecraft:enchantable/sharp_weapon"
        );
        assert_eq!(
            data.exclusive_set.as_ref().map(|k| k.as_str()),
            Some("minecraft:exclusive_set/damage")
        );
        assert_eq!(
            data.primary_items.as_ref().map(|k| k.as_str()),
            Some("minecraft:enchantable/melee_weapon")
        );
    }

    #[test]
    fn deserialize_and_resolve_all_enchantments() {
        let dir = assets_dir().join("minecraft/enchantment");
        let mut count = 0;
        let mut failures = Vec::new();

        for entry in std::fs::read_dir(&dir).expect("enchantment dir must exist") {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let bytes = std::fs::read(&path).unwrap();
            match serde_json::from_slice::<ProtoEnchantmentData>(&bytes) {
                Ok(proto) => match proto.resolve() {
                    Ok(_) => count += 1,
                    Err(e) => failures.push((path.display().to_string(), e.to_string())),
                },
                Err(e) => failures.push((path.display().to_string(), e.to_string())),
            }
        }

        if !failures.is_empty() {
            for (path, err) in &failures {
                eprintln!("FAIL {path}: {err}");
            }
            panic!(
                "{} of {} enchantments failed",
                failures.len(),
                count + failures.len()
            );
        }

        assert!(count > 0, "no enchantment files found");
        eprintln!("successfully deserialized and resolved {count} enchantments");
    }

    #[test]
    fn enchantment_without_exclusive_set() {
        let bytes = std::fs::read(
            assets_dir().join("minecraft/enchantment/mending.json"),
        )
        .unwrap();
        let proto: ProtoEnchantmentData = serde_json::from_slice(&bytes).unwrap();
        let data = proto.resolve().unwrap();
        assert!(data.exclusive_set.is_none());
    }
}
