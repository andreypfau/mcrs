pub mod beta_surface;
pub mod climate;
pub mod source;

use std::collections::BTreeMap;
use std::sync::Arc;

use bevy_asset::io::Reader;
use bevy_asset::{Asset, AssetLoader, Handle, LoadContext, UntypedAssetId, VisitAssetDependencies};
use bevy_reflect::TypePath;
use serde::de::{self, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};

use crate::ResourceLocation;
use crate::attribute::EnvironmentAttributeMap;
use crate::value::IntValueProvider;

pub const NATURAL_MOB_SPAWNS: &str = "minecraft:gameplay/natural_mob_spawns";

#[derive(Debug, Clone, Serialize, Deserialize, TypePath)]
pub struct Biome {
    pub temperature: f32,
    pub downfall: f32,
    pub has_precipitation: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature_modifier: Option<String>,
    pub effects: BiomeEffects,
    #[serde(default)]
    pub attributes: EnvironmentAttributeMap,
    #[serde(default, deserialize_with = "one_or_many")]
    pub carvers: Vec<ResourceLocation<Arc<str>>>,
    #[serde(default)]
    pub features: Vec<Vec<ResourceLocation<Arc<str>>>>,
}

impl Biome {
    pub fn load(ctx: &mut LoadContext<'_>, loc: &ResourceLocation<Arc<str>>) -> Handle<Biome> {
        ctx.load(format!("{}/worldgen/biome/{}.json", loc.namespace(), loc.path()))
    }

    pub fn natural_mob_spawns(&self) -> serde_json::Result<Option<MobSpawnSettings>> {
        self.attributes.argument(NATURAL_MOB_SPAWNS)
    }
}

/// Biome data subset for NETWORK_CODEC — omits server-only generation settings.
///
/// Sent to clients during Configuration; excludes carvers, features, and the
/// attributes the client is not allowed to see.
#[derive(Debug, Clone, Serialize)]
pub struct NetworkBiome {
    pub temperature: f32,
    pub downfall: f32,
    pub has_precipitation: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature_modifier: Option<String>,
    pub attributes: EnvironmentAttributeMap,
    pub effects: BiomeEffects,
}

impl From<&Biome> for NetworkBiome {
    fn from(biome: &Biome) -> Self {
        NetworkBiome {
            temperature: biome.temperature,
            downfall: biome.downfall,
            has_precipitation: biome.has_precipitation,
            temperature_modifier: biome.temperature_modifier.clone(),
            attributes: biome.attributes.filter_syncable(),
            effects: biome.effects.clone(),
        }
    }
}

impl Asset for Biome {}

impl VisitAssetDependencies for Biome {
    fn visit_dependencies(&self, _visit: &mut impl FnMut(UntypedAssetId)) {}
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BiomeEffects {
    #[serde(default)]
    pub water_color: Option<String>,
    #[serde(default)]
    pub foliage_color: Option<String>,
    #[serde(default)]
    pub grass_color: Option<String>,
    #[serde(default)]
    pub grass_color_modifier: Option<String>,
    #[serde(default)]
    pub dry_foliage_color: Option<String>,
}

/// The `minecraft:gameplay/natural_mob_spawns` attribute argument.
///
/// A category absent from `spawns_by_category` is undefined and falls through
/// to the layer below; a category present but empty suppresses it, which is how
/// `deep_dark` and `the_void` silence the dimension's spawns.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MobSpawnSettings {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub spawn_costs: BTreeMap<ResourceLocation<Arc<str>>, SpawnCost>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub spawns_by_category: BTreeMap<MobCategory, Vec<SpawnerData>>,
}

impl MobSpawnSettings {
    pub fn is_empty(&self) -> bool {
        self.spawn_costs.is_empty() && self.spawns_by_category.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MobCategory {
    Ambient,
    Axolotls,
    Creature,
    Misc,
    Monster,
    UndergroundWaterCreature,
    WaterAmbient,
    WaterCreature,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpawnerData {
    #[serde(rename = "type")]
    pub entity_type: ResourceLocation<Arc<str>>,
    pub count: IntValueProvider,
    pub weight: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpawnCost {
    pub charge: f64,
    pub energy_budget: f64,
}

// ---------------------------------------------------------------------------
// Serde helper: accept either a single value or an array
// ---------------------------------------------------------------------------

fn one_or_many<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    struct OneOrManyVisitor<T>(std::marker::PhantomData<T>);

    impl<'de, T: Deserialize<'de>> Visitor<'de> for OneOrManyVisitor<T> {
        type Value = Vec<T>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a single value or an array")
        }

        fn visit_seq<A>(self, seq: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            Vec::deserialize(de::value::SeqAccessDeserializer::new(seq))
        }

        fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
            let item = T::deserialize(de::value::StrDeserializer::new(v))?;
            Ok(vec![item])
        }

        fn visit_string<E: de::Error>(self, v: String) -> Result<Self::Value, E> {
            let item = T::deserialize(de::value::StringDeserializer::new(v))?;
            Ok(vec![item])
        }

        fn visit_map<M>(self, map: M) -> Result<Self::Value, M::Error>
        where
            M: de::MapAccess<'de>,
        {
            let item = T::deserialize(de::value::MapAccessDeserializer::new(map))?;
            Ok(vec![item])
        }
    }

    deserializer.deserialize_any(OneOrManyVisitor(std::marker::PhantomData))
}

// ---------------------------------------------------------------------------
// Asset loader
// ---------------------------------------------------------------------------

#[derive(Default, TypePath)]
pub struct BiomeLoader;

#[derive(Debug, thiserror::Error)]
pub enum BiomeLoaderError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),
}

impl AssetLoader for BiomeLoader {
    type Asset = Biome;
    type Settings = ();
    type Error = BiomeLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Biome, BiomeLoaderError> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let biome: Biome = serde_json::from_slice(&bytes)?;
        Ok(biome)
    }

    fn extensions(&self) -> &[&str] {
        &[]
    }
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
    fn deserialize_all_biomes() {
        let biome_dir = assets_dir().join("minecraft/worldgen/biome");
        let mut count = 0;
        let mut failures = Vec::new();

        for entry in std::fs::read_dir(&biome_dir).expect("biome dir must exist") {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let bytes = std::fs::read(&path).unwrap();
            match serde_json::from_slice::<Biome>(&bytes) {
                Ok(biome) => {
                    let raw: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
                    let attributes =
                        raw.get("attributes").cloned().unwrap_or_else(|| serde_json::json!({}));
                    assert_eq!(
                        serde_json::to_value(&biome.attributes).unwrap(),
                        attributes,
                        "{} attributes must round-trip unchanged",
                        path.display()
                    );
                    count += 1;
                }
                Err(e) => failures.push((path.display().to_string(), e.to_string())),
            }
        }

        if !failures.is_empty() {
            for (path, err) in &failures {
                eprintln!("FAIL {path}: {err}");
            }
            panic!("{} of {} biomes failed to deserialize", failures.len(), count + failures.len());
        }

        assert!(count > 0, "no biome files found");
        eprintln!("successfully deserialized {count} biomes");
    }

    #[test]
    fn network_biome_omits_server_fields() {
        let bytes = std::fs::read(
            assets_dir().join("minecraft/worldgen/biome/plains.json"),
        )
        .unwrap();
        let biome: Biome = serde_json::from_slice(&bytes).unwrap();
        let network = NetworkBiome::from(&biome);

        let json = serde_json::to_value(&network).unwrap();
        assert!(json.get("temperature").is_some());
        assert!(json.get("downfall").is_some());
        assert!(json.get("has_precipitation").is_some());
        assert!(json.get("effects").is_some());
        assert!(json.get("carvers").is_none());
        assert!(json.get("features").is_none());

        let attributes = json.get("attributes").expect("attributes are synced");
        assert_eq!(attributes.get("minecraft:visual/sky_color").unwrap(), "#78a7ff");
        assert!(attributes.get(NATURAL_MOB_SPAWNS).is_none(), "spawns are server-only");

        let nbt = mcrs_nbt::to_nbt_compound(&network).expect("network biome must encode to NBT");
        let Some(mcrs_nbt::tag::NbtTag::Compound(attributes)) = nbt.get("attributes") else {
            panic!("attributes must reach the client as a compound");
        };
        assert_eq!(
            attributes.get("minecraft:visual/sky_color"),
            Some(&mcrs_nbt::tag::NbtTag::String("#78a7ff".to_string()))
        );

        assert!((network.temperature - biome.temperature).abs() < f32::EPSILON);
        assert!((network.downfall - biome.downfall).abs() < f32::EPSILON);
        assert_eq!(network.has_precipitation, biome.has_precipitation);
    }

    #[test]
    fn deserialize_plains_biome() {
        let bytes = std::fs::read(
            assets_dir().join("minecraft/worldgen/biome/plains.json"),
        )
        .unwrap();
        let biome: Biome = serde_json::from_slice(&bytes).unwrap();

        assert!((biome.temperature - 0.8).abs() < f32::EPSILON);
        assert!((biome.downfall - 0.4).abs() < f32::EPSILON);
        assert!(biome.has_precipitation);
        assert_eq!(biome.carvers.len(), 3);
        assert_eq!(biome.carvers[0].as_str(), "minecraft:cave");
        let spawns = biome.natural_mob_spawns().unwrap().expect("plains has spawns");
        assert!(!spawns.spawns_by_category[&MobCategory::Creature].is_empty());
        assert_eq!(
            biome.attributes.get(NATURAL_MOB_SPAWNS).unwrap().modifier,
            crate::attribute::Operation::Overlay
        );
        assert_eq!(
            biome.attributes.get("minecraft:visual/sky_color").unwrap().argument,
            serde_json::json!("#78a7ff")
        );
    }
}
